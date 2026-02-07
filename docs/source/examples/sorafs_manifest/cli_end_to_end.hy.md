---
lang: hy
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS Մանիֆեստ CLI-ի ավարտից մինչև վերջ օրինակ

Այս օրինակը անցնում է SoraFS-ում փաստաթղթերի կառուցման հրապարակման միջոցով՝ օգտագործելով
`sorafs_manifest_stub` CLI դետերմինիստական կտրատող հարմարանքների հետ միասին
նկարագրված է SoraFS Architecture RFC-ում: Հոսքը ընդգրկում է մանիֆեստի սերունդը,
ակնկալիքների ստուգում, առբերման պլանի վավերացում և առբերման ապացույցի փորձ, այսպես
թիմերը կարող են ներդնել նույն քայլերը CI-ում:

## Նախադրյալներ

- Աշխատանքային տարածքը կլոնավորված է և պատրաստ է գործիքների շղթայով (`cargo`, `rustc`):
- `fixtures/sorafs_chunker`-ի հարմարանքները հասանելի են, որպեսզի ակնկալիքների արժեքները լինեն
  ստացված (արտադրական գործարկումների համար, հանեք արժեքները միգրացիոն մատյանից
  կապված արտեֆակտի հետ):
- Նմուշի բեռնվածության գրացուցակը հրապարակելու համար (այս օրինակն օգտագործում է `docs/book`):

## Քայլ 1 — Ստեղծեք մանիֆեստ, մեքենա, ստորագրություններ և բեռնման պլան

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

Հրաման.

- Հոսում է օգտակար բեռը `ChunkProfile::DEFAULT`-ի միջոցով:
- Թողարկում է CARv2 արխիվ, գումարած բեկորային պլան:
- Ստեղծում է `ManifestV1` գրառում, ստուգում է մանիֆեստի ստորագրությունները (եթե նախատեսված է) և
  գրում է ծրարը.
- Ակտիվացնում է ակնկալիքների դրոշները, որպեսզի գործարկումը ձախողվի, եթե բայթերը շարժվեն:

## Քայլ 2 — Ստուգեք արդյունքները chunk store + PoR փորձով

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Սա կրկնում է մեքենան դետերմինիստական կտոր պահեստի միջոցով, բխում է
Առբերելիության ապացույցի նմուշառման ծառ և թողարկում է մանիֆեստի հաշվետվություն, որը հարմար է
կառավարման վերանայում։

## Քայլ 3 — Մոդելավորել բազմաբնույթ մատակարարների որոնումը

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI միջավայրերի համար տրամադրեք առանձին բեռնատար ուղիներ յուրաքանչյուր մատակարարի համար (օրինակ՝ տեղադրված
հարմարանքներ) իրականացնելու տիրույթի պլանավորում և խափանումների մշակում:

## Քայլ 4 — Գրանցեք մատյանում մուտքը

Մուտքագրեք հրապարակումը `docs/source/sorafs/migration_ledger.md`-ում՝ ֆիքսելով.

- Դրսևորեք CID, CAR ամփոփում և խորհրդի ստորագրության հեշ:
- Կարգավիճակ (`Draft`, `Staging`, `Pinned`):
- Հղումներ դեպի CI վազք կամ կառավարման տոմսեր:

## Քայլ 5 — Ամրացրեք կառավարման գործիքակազմի միջոցով (երբ ռեեստրն ակտիվ է)

Երբ Pin Registry-ը տեղակայվի (Milestone M2 միգրացիոն ճանապարհային քարտեզում),
ներկայացնել մանիֆեստը CLI-ի միջոցով.

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

Առաջարկի նույնացուցիչը և հետագա հաստատման գործարքի հեշերը պետք է լինեն
ընդգրկված է միգրացիոն մատյանում աուդիտի համար:

## Մաքրում

`target/sorafs/` տակ գտնվող արտեֆակտները կարող են արխիվացվել կամ վերբեռնվել բեմականացման հանգույցներում:
Մանիֆեստը, ստորագրությունները, CAR-ը և առբերման պլանը միասին պահեք այնքան ներքև
օպերատորները և SDK թիմերը կարող են վճռականորեն հաստատել տեղակայումը: