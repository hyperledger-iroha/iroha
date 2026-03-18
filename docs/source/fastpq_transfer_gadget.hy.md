---
lang: hy
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ փոխանցման հարմարանքների ձևավորում

# Ընդհանուր ակնարկ

Ներկայիս FASTPQ պլանավորողը գրանցում է `TransferAsset` հրահանգում ներառված յուրաքանչյուր պարզունակ գործողություն, ինչը նշանակում է, որ յուրաքանչյուր փոխանցում վճարում է հաշվեկշռի թվաբանության, հեշ փուլերի և SMT թարմացումների համար առանձին: Մեկ փոխանցման համար տողերը նվազեցնելու համար մենք ներկայացնում ենք հատուկ հարմարանք, որը ստուգում է միայն թվաբանական/պարտավորությունների նվազագույն ստուգումները, մինչ հյուրընկալողը շարունակում է կատարել կանոնական վիճակի անցումը:

- **Շրջանակ**. առանձին փոխանցումներ և փոքր խմբաքանակներ, որոնք արտանետվում են գոյություն ունեցող Kotodama/IVM `TransferAsset` համակարգային մակերևույթի միջոցով:
- **Նպատակ**. կտրեք FFT/LDE սյունակի հետքը մեծ ծավալի փոխանցումների համար՝ փոխանակելով որոնման աղյուսակները և փլուզելով յուրաքանչյուր փոխանցման թվաբանությունը կոմպակտ սահմանափակումների բլոկի մեջ:

# Ճարտարապետություն

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Տառադարձման ձևաչափ

Հյուրընկալողը թողարկում է `TransferTranscript` մեկ syscall կանչի համար՝

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash`-ը կապում է տառադարձումը գործարքի մուտքի կետի հեշին` կրկնությունից պաշտպանվելու համար:
- `authority_digest`-ը հյուրընկալողի հեշն է տեսակավորված ստորագրողների/քվորումի տվյալների նկատմամբ. գաջեթը ստուգում է հավասարությունը, բայց չի կրկնում ստորագրության ստուգումը: Կոնկրետ հոսթ Norito-կոդավորում է `AccountId`-ը (որն արդեն ներկառուցում է կանոնական multisig կարգավորիչը) և հեշավորում է `b"iroha:fastpq:v1:authority|" || encoded_account`-ը Blake2b-256-ի հետ՝ պահպանելով ստացված I180NI00000-ը:
- `poseidon_preimage_digest` = Poseidon (հաշիվ_-ից || հաշիվ_ից || ակտիվ || գումար || խմբաքանակի_հաշ); ապահովում է, որ գաջեթը վերահաշվարկի նույն բովանդակությունը, ինչ հյուրընկալողը: Նախապատկերի բայթերը կառուցված են որպես `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash`՝ օգտագործելով մերկ Norito կոդավորումը՝ նախքան դրանք ընդհանուր Poseidon2 օգնականի միջով անցնելը: Այս ամփոփումը առկա է մեկ դելտա տառադարձումների համար և բաց թողնված բազմադելտա խմբաքանակների համար:

Բոլոր դաշտերը սերիականացված են Norito-ի միջոցով, ուստի գոյություն ունեցող դետերմինիզմի երաշխիքները պահպանվում են:
Թե՛ `from_path`, և թե՛ `to_path` արտանետվում են որպես Norito բշտիկներ՝ օգտագործելով
`TransferMerkleProofV1` սխեման՝ `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`:
Ապագա տարբերակները կարող են ընդլայնել սխեման, մինչդեռ պրովերը պարտադրում է տարբերակի պիտակը
վերծանումից առաջ: `TransitionBatch` մետատվյալները զետեղում են Norito կոդավորված տառադարձությունը
վեկտոր `transfer_transcripts` ստեղնի տակ, որպեսզի պրովերը կարողանա վերծանել վկային
առանց խմբից դուրս հարցումներ կատարելու: Հանրային մուտքեր (`dsid`, `slot`, արմատներ,
`perm_root`, `tx_set_hash`) տեղափոխվում են `FastpqTransitionBatch.public_inputs`,
թողնելով մետատվյալները մուտքի հեշ/ձայնագրությունների հաշվառման հաշվառման համար: Մինչև հյուրընկալող սանտեխնիկան
հողերը, պրովերը սինթետիկ կերպով ապացույցներ է ստանում բանալին/բալանսի զույգերից և տողերից
միշտ ներառել դետերմինիստական SMT ուղի, նույնիսկ երբ տառադարձումը բաց է թողնում ընտրովի դաշտերը:

## Գործիքների դասավորություն

1. **Հաշվեկշռի թվաբանական բլոկ**
   - Մուտքեր՝ `from_balance_before`, `amount`, `to_balance_before`:
   - Ստուգումներ.
     - `from_balance_before >= amount` (համօգտագործվող RNS տարրալուծմամբ միջակայքի հարմարանք):
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Փաթեթավորված է հատուկ դարպասի մեջ, այնպես որ բոլոր երեք հավասարումները սպառում են մեկ տող խումբ:2. **Պոսեյդոնի պարտավորությունների բլոկ**
   - Վերահաշվարկում է `poseidon_preimage_digest`-ը՝ օգտագործելով Poseidon-ի ընդհանուր որոնման աղյուսակը, որն արդեն օգտագործվում է այլ հարմարանքներում: Հետքում Պոսեյդոնի պտույտներ չկան յուրաքանչյուր փոխանցման համար:

3. **Merkle Path Block**
   - Ընդլայնում է գոյություն ունեցող Kaigi SMT գաջեթը «զույգացված թարմացում» ռեժիմով: Երկու տերևներ (ուղարկող, ստացող) կիսում են նույն սյունակը եղբայրական հեշերի համար՝ նվազեցնելով կրկնվող տողերը:

4. **Authority Digest Check **
   - Պարզ հավասարության սահմանափակում հյուրընկալողի կողմից տրամադրված ամփոփագրի և վկայի արժեքի միջև: Ստորագրությունները մնում են իրենց հատուկ գործիքում:

5. **Խմբաքանակային հանգույց **
   - Ծրագրերը զանգահարում են `transfer_v1_batch_begin()` `transfer_asset` կառուցողներից առաջ և `transfer_v1_batch_end()`-ից հետո: Մինչ շրջանակն ակտիվ է, հյուրընկալողը բուֆերացնում է յուրաքանչյուր փոխանցում և վերարտադրում դրանք որպես մեկ `TransferAssetBatch`՝ կրկին օգտագործելով Poseidon/SMT համատեքստը մեկ խմբաքանակի համար: Յուրաքանչյուր լրացուցիչ դելտա ավելացնում է միայն թվաբանական և երկու տերևային ստուգումներ: Տառադարձման ապակոդավորիչն այժմ ընդունում է բազմադելտա խմբաքանակներ և դրանք դնում է որպես `TransferGadgetInput::deltas`, որպեսզի պլանավորողը կարողանա ծալել վկաներին՝ առանց Norito վերընթերցելու: Պայմանագրերը, որոնք արդեն ունեն Norito օգտակար բեռնվածություն (օրինակ՝ CLI/SDK) կարող են ամբողջությամբ բաց թողնել շրջանակը՝ զանգահարելով `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, որը հոսթին հանձնում է ամբողջությամբ կոդավորված խմբաքանակ մեկ syscall-ում:

# Հյուրընկալողի և պրովերի փոփոխություններ| Շերտ | Փոփոխություններ |
|-------|---------|
| `ivm::syscalls` | Ավելացրեք `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`), որպեսզի ծրագրերը կարողանան փակագծել բազմաթիվ `transfer_v1` համակարգային զանգեր՝ առանց միջանկյալ ISI-ներ արձակելու, գումարած I01: (`0x2B`) նախապես կոդավորված խմբաքանակների համար: |
| `ivm::host` & թեստեր | Հիմնական/Լռակյաց հոստերները վերաբերվում են `transfer_v1`-ին որպես խմբաքանակի հավելված, քանի դեռ շրջանակն ակտիվ է, `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` մակերևույթը, իսկ WSV-ի ծաղրական հաղորդիչը բուֆերացնում է մուտքերը նախքան կատարելը, որպեսզի ռեգրեսիայի թեստերը կարողանան հաստատել դետերմինիստական հավասարակշռություն: թարմացումներ։【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Արտադրեք `TransferTranscript` վիճակի անցումից հետո, ստեղծեք `FastpqTransitionBatch` գրառումներ բացահայտ `public_inputs`-ով `StateBlock::capture_exec_witness`-ի ընթացքում և գործարկեք FASTPQ պրովերի գիծը, որպեսզի և՛ `StateBlock::capture_exec_witness`-ը կարողանան ստանալ և՛ Torii, և՛ Torii-ի հետադարձ կապը: `TransitionBatch` մուտքեր: `TransferAssetBatch`-ը խմբավորում է հաջորդական փոխանցումները մեկ տառադարձման մեջ՝ բաց թողնելով poseidon digest-ը բազմադելտա խմբաքանակների համար, որպեսզի գաջեթը կարողանա դետերմինիստիկ կերպով կրկնել գրառումների միջև: |
| `fastpq_prover` | `gadgets::transfer`-ն այժմ վավերացնում է բազմադելտա տառադարձումները (հավասարակշռված թվաբանություն + Poseidon digest) և մակերեսային կառուցվածքային վկաներ (ներառյալ տեղապահով զուգակցված SMT բլբեր) պլանավորողի համար (`crates/fastpq_prover/src/gadgets/transfer.rs`): `trace::build_trace`-ը վերծանում է այդ տառադարձումները խմբաքանակի մետատվյալներից, մերժում է փոխանցման խմբաքանակները, որոնցում բացակայում է `transfer_transcripts` օգտակար բեռը, վավերացված վկաները կցում է `Trace::transfer_witnesses`-ին, և `Trace::transfer_witnesses`-ին, և `TracePolynomialData::transfer_plan()`-ը պահում է aligned planesum planesum agg-ը: (`crates/fastpq_prover/src/trace.rs`): Տողերի քանակի ռեգրեսիոն զրահը այժմ առաքվում է `fastpq_row_bench`-ի միջոցով (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)՝ ընդգրկելով մինչև 65536 լիցքավորված տողերի սցենարներ, մինչդեռ զուգակցված SMT լարերը մնում են TF-3 խմբաքանակի օգնականի ետևում (մինչև որ տեղապահները չպահպանեն ցամաքային գծերը): |
| Kotodama | Իջեցնում է `transfer_batch((from,to,asset,amount), …)` օգնականը դեպի `transfer_v1_batch_begin`, հաջորդական `transfer_asset` զանգեր և `transfer_v1_batch_end`: Յուրաքանչյուր բազմակի արգումենտ պետք է հետևի `(AccountId, AccountId, AssetDefinitionId, int)` ձևին; միայնակ փոխանցումները պահպանում են առկա շինարարին. |

Օրինակ Kotodama օգտագործման.

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch`-ը կատարում է նույն թույլտվությունը և թվաբանական ստուգումները, ինչ առանձին `Transfer::asset_numeric` զանգերը, բայց գրանցում է բոլոր դելտաները մեկ `TransferTranscript`-ի ներսում: Բազմադելտա տառադարձումները վերացնում են պոսեյդոնի մարսողությունը, մինչև մեկ դելտայի պարտավորությունները իջնեն հաջորդիվ: Kotodama կառուցողն այժմ ավտոմատ կերպով թողարկում է սկզբի/վերջի համակարգի զանգերը, այնպես որ պայմանագրերը կարող են տեղակայել խմբաքանակային փոխանցումներ՝ առանց Norito բեռների կոդավորման ձեռքով:

## Տողերի քանակի ռեգրեսիոն ամրացում

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) սինթեզում է FASTPQ անցումային խմբաքանակները կարգավորելի ընտրիչի թվերով և հաղորդում է ստացված `row_usage` ամփոփագիրը (`total_rows`, յուրաքանչյուր ընտրիչի երկարությունը ₂ հաշվում է երկայնքով: 65536 շարքով առաստաղի համար նշեք հենանիշները՝

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```Արտանետվող JSON-ը արտացոլում է FASTPQ խմբաքանակի արտեֆակտները, որոնք `iroha_cli audit witness`-ն այժմ արտանետում է լռելյայն (անցեք `--no-fastpq-batches`՝ դրանք ճնշելու համար), այնպես որ `scripts/fastpq/check_row_usage.py`-ը և CI դարպասը կարող են տարբերել սինթետիկ պլանների նկարահանումները նախորդ վավերացման ժամանակ:

# Տարածման պլան

1. **TF-1 (Transcript Plumbing)**. ✅ `StateTransaction::record_transfer_transcripts`-ն այժմ թողարկում է Norito տառադարձումներ յուրաքանչյուր `TransferAsset`/խմբաքանակի համար, `sumeragi::witness::record_fastpq_transcript`-ը դրանք պահում է գլոբալ վկայի ներսում81000X, իսկ Kotodama `fastpq_batches` բացահայտ `public_inputs`-ով օպերատորների և պրովերի գծի համար (օգտագործեք `--no-fastpq-batches`, եթե ձեզ ավելի բարակ է հարկավոր ելք).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness .rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (Գաջեթի իրականացում)**. ✅ `gadgets::transfer`-ն այժմ վավերացնում է բազմադելտա տառադարձումները (հավասարակշռված թվաբանություն + Poseidon digest), սինթեզում է զուգակցված SMT ապացույցները, երբ հոսթորդները դրանք բաց են թողնում, բացահայտում կառուցվածքային վկաներին Kotodama-ի և Kotodama-ի միջոցով: այդ վկաներին կապում է `Trace::transfer_witnesses` մեջ, իսկ ապացույցներից լրացնում է SMT սյունակները: `fastpq_row_bench`-ը գրավում է 65536 տողով ռեգրեսիոն ամրագոտին, որպեսզի պլանավորողները հետևեն տողերի օգտագործմանը՝ առանց Norito վերարտադրելու օգտակար բեռներ.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Խմբաքանակի օգնական)**. Միացրեք խմբաքանակի syscall + Kotodama ստեղծողը, ներառյալ հոսթի մակարդակի հաջորդական հավելվածը և հարմարանքների հանգույցը:
4. **TF-4 (Telemetry & Docs)**. Թարմացրեք `fastpq_plan.md`, `fastpq_migration_guide.md` և վահանակի սխեմաները՝ փոխանցման տողերի մակերեսի բաշխման համար այլ հարմարանքների նկատմամբ:

#Բաց Հարցեր

- **Դոմենի սահմանները**. ներկայիս FFT պլանավորողը խուճապի է մատնվում 2¹4 տողից ավելի հետքերի համար: TF-2-ը պետք է կա՛մ բարձրացնի տիրույթի չափը, կա՛մ փաստաթղթավորի նվազեցված հենանիշային թիրախ:
- **Բազմ ակտիվների խմբաքանակ**. սկզբնական հարմարանքը ենթադրում է նույն ակտիվի ID-ն մեկ դելտայի համար: Եթե ​​մեզ անհրաժեշտ են տարասեռ խմբաքանակներ, մենք պետք է ապահովենք, որ Պոսեյդոնի վկան ամեն անգամ ներառի ակտիվը, որպեսզի կանխենք խաչաձև ակտիվների կրկնությունը:
- **Authority digest reuse**. երկարաժամկետ մենք կարող ենք նորից օգտագործել նույն ամփոփումը այլ թույլատրված գործողությունների համար՝ խուսափելու համար ստորագրողների ցուցակների վերահաշվարկից յուրաքանչյուր syscall-ի համար:


Այս փաստաթուղթը հետևում է դիզայնի որոշումներին. պահեք այն համաժամանակյա ճանապարհային քարտեզի գրառումների հետ, երբ նշմարվում են կարևոր կետեր: