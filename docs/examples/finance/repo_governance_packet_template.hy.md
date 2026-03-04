---
lang: hy
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo-ի կառավարման փաթեթի ձևանմուշ (ճանապարհային քարտեզ F1)

Օգտագործեք այս ձևանմուշը՝ ճանապարհային քարտեզի կետով պահանջվող արտեֆակտների փաթեթը պատրաստելիս
F1 (ռեպո կյանքի ցիկլի փաստաթղթեր և գործիքներ): Նպատակն է հանձնել գրախոսներին ա
մեկ Markdown ֆայլ, որը թվարկում է բոլոր մուտքագրման, հեշի և ապացույցների փաթեթը
Կառավարման խորհուրդը կարող է վերարտադրել առաջարկի մեջ նշված բայթերը:

> Պատճենեք ձևանմուշը ձեր սեփական ապացույցների գրացուցակում (օրինակ
> `artifacts/finance/repo/2026-03-15/packet.md`), փոխարինիր տեղապահները և
> կատարել/վերբեռնել այն ստորև նշված հաշված արտեֆակտների կողքին:

## 1. Մետատվյալներ

| Դաշտային | Արժեք |
|-------|-------|
| Համաձայնագրի/փոփոխության նույնացուցիչ | `<repo-yyMMdd-XX>` |
| Պատրաստված է / ամսաթիվը | `<desk lead> – 2026-03-15T10:00Z` |
| Վերանայվել է | `<dual-control reviewer(s)>` |
| Փոխել տեսակը | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Պահառու(ներ) | `<custodian id(s)>` |
| Կապված առաջարկ / հանրաքվե | `<governance ticket id or GAR link>` |
| Ապացույցների տեղեկատու | «`artifacts/finance/repo/<slug>/`» |

## 2. Հրահանգների օգտակար բեռներ

Ձայնագրեք Norito փուլային հրահանգները, որոնց միջոցով գրասեղանները ստորագրել են
`iroha app repo ... --output`. Յուրաքանչյուր գրառում պետք է ներառի թողարկվածի հեշը
ֆայլը և գործողության կարճ նկարագրությունը, որը կներկայացվի քվեարկությունից հետո
անցնում է.

| Գործողություն | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|--------|------|---------|-------|
| Նախաձեռնել | `instructions/initiate.json` | `<sha256>` | Պարունակում է կանխիկ/գրավի ոտքեր, որոնք հաստատված են գրասեղանի + կոնտրագենտի կողմից: |
| Մարժան կանչ | `instructions/margin_call.json` | `<sha256>` | Լուսանկարում է կադանսը + մասնակցի id-ն, որն ակտիվացրել է զանգը: |
| Լիցքաթափվել | `instructions/unwind.json` | `<sha256>` | Պայմանները բավարարելուց հետո հակառակ ոտքի ապացույց: |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Պահառուի երախտագիտություն (միայն եռակողմ)

Լրացրեք այս բաժինը, երբ ռեպո օգտագործում է `--custodian`: Կառավարման փաթեթ
պետք է ներառի յուրաքանչյուր պահառուի ստորագրված հաստատում, գումարած դրա հեշը
ֆայլը նշված է `docs/source/finance/repo_ops.md`-ի §2.8-ում:

| Պահառու | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Ստորագրված SLA, որը ծածկում է պահառության պատուհանը, երթուղային հաշիվը և փորված կոնտակտը: |

> Պահպանեք հաստատումը մյուս ապացույցների կողքին (`artifacts/finance/repo/<slug>/`)
> այնպես որ, `scripts/repo_evidence_manifest.py` ֆայլը գրանցում է նույն ծառի վրա, ինչ
> փուլային հրահանգներ և կազմաձևման հատվածներ: Տես
> `docs/examples/finance/repo_custodian_ack_template.md` պատրաստի լիցքավորման համար
> Կաղապար, որը համապատասխանում է կառավարման ապացույցների պայմանագրին:

## 3. Կազմաձևման հատված

Տեղադրեք `[settlement.repo]` TOML բլոկը, որը վայրէջք կկատարի կլաստերի վրա (ներառյալ
`collateral_substitution_matrix`): Պահպանեք հեշը հատվածի կողքին այսպես
աուդիտորները կարող են հաստատել գործարկման ժամանակի քաղաքականությունը, որն ակտիվ էր ռեպո ամրագրման ժամանակ
հաստատվել է։

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Հաստատումից հետո կազմաձևման պատկերներ

Հանրաքվեի կամ կառավարման քվեարկության ավարտից հետո և `[settlement.repo]`
Փոփոխությունն իրականացվել է, նկարահանեք `/v1/configuration` նկարներ յուրաքանչյուր հասակակից
աուդիտորները կարող են ապացուցել, որ հաստատված քաղաքականությունը գործում է ամբողջ կլաստերում (տես
`docs/source/finance/repo_ops.md` §2.9 ապացույցների աշխատանքային հոսքի համար):

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Գործընկեր / աղբյուր | Ֆայլ | SHA-256 | Բլոկի բարձրությունը | Ծանոթագրություններ |
|-----------------------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Պատկերը նկարահանվել է կազմաձևման թողարկումից անմիջապես հետո: |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Հաստատում է, որ `[settlement.repo]`-ը համապատասխանում է բեմադրված TOML-ին: |

Գրանցեք ամփոփումները `hashes.txt`-ում (կամ համարժեք) համապատասխան ID-ների կողքին
ամփոփում), որպեսզի վերանայողները կարողանան հետևել, թե որ հանգույցներն են ընդունել փոփոխությունը: Պատկերները
ապրում է `config/peers/` տակ TOML հատվածի կողքին և կվերցվի
ավտոմատ կերպով `scripts/repo_evidence_manifest.py`-ի միջոցով:

## 4. Դետերմինիստական թեստի արտեֆակտներ

Կցեք վերջին արդյունքները հետևյալից.

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Գրանցեք ֆայլի ուղիները + հեշերը ձեր CI-ի կողմից արտադրված տեղեկամատյանների փաթեթների կամ JUnit XML-ի համար
համակարգ.

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Կյանքի ցիկլի ապացուցման մատյան | `tests/repo_lifecycle.log` | `<sha256>` | Նկարված է `--nocapture` ելքով: |
| Ինտեգրման թեստային մատյան | `tests/repo_integration.log` | `<sha256>` | Ներառում է փոխարինում + մարժա կադենսի ծածկույթ: |

## 5. Lifecycle Proof Snapshot

Յուրաքանչյուր փաթեթ պետք է ներառի դետերմինիստական կյանքի ցիկլի նկարը, որից արտահանվում է
`repo_deterministic_lifecycle_proof_matches_fixture`. Գործարկեք ամրագոտիը
արտահանման կոճակները միացված են, որպեսզի վերանայողները կարողանան տարբերել JSON շրջանակը և վերլուծել դրանցից
սարքը, որը հետևվում է `crates/iroha_core/tests/fixtures/`-ում (տես
`docs/source/finance/repo_ops.md` §2.7):

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Կամ օգտագործեք ամրացված օգնականը` հարմարանքները վերականգնելու և դրանք ձեր մեջ պատճենելու համար
ապացույցների փաթեթ մեկ քայլով.

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Կանոնական կյանքի ցիկլի շրջանակ, որն արտանետվում է ապացուցման ամրագոտու կողմից: |
| Digest ֆայլ | `repo_proof_digest.txt` | `<sha256>` | Մեծատառ վեցանկյուն դիջեստ՝ արտացոլված `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`-ից; կցել նույնիսկ երբ անփոփոխ. |

## 6. Ապացույցների մանիֆեստ

Ստեղծեք մանիֆեստ ամբողջ ապացույցների գրացուցակի համար, որպեսզի աուդիտորները կարողանան ստուգել
հեշեր առանց արխիվը բացելու: Օգնականը արտացոլում է նկարագրված աշխատանքային ընթացքը
`docs/source/finance/repo_ops.md`-ում §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Ապացույցների դրսևորում | `manifest.json` | `<sha256>` | Ներառեք ստուգման գումարը կառավարման տոմսի / հանրաքվեի նշումներում: |

## 7. Հեռուստաչափություն և իրադարձության նկար

Արտահանեք համապատասխան `AccountEvent::Repo(*)` գրառումները և ցանկացած վահանակ կամ CSV
արտահանումները, որոնք նշված են `docs/source/finance/repo_ops.md`-ում: Ձայնագրեք ֆայլերը +
հեշեր այստեղ, որպեսզի գրախոսները կարողանան անմիջապես անցնել ապացույցներին:

| Արտահանում | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|--------|------|---------|-------|
| Repo միջոցառումներ JSON | `evidence/repo_events.ndjson` | `<sha256>` | Raw Torii իրադարձությունների հոսքը զտված է գրասեղանի հաշիվներին: |
| Հեռաչափություն CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Արտահանվել է Grafana-ից՝ օգտագործելով Repo Margin վահանակը: |

## 8. Հաստատումներ և ստորագրություններ

- **Երկակի կառավարման ստորագրողներ.** `<names + timestamps>`
- **GAR / րոպե ամփոփում. ** `<sha256>` ստորագրված GAR PDF-ի կամ րոպեների վերբեռնման:
- **Պահպանման վայրը՝** `governance://finance/repo/<slug>/packet/`

## 9. Ստուգաթերթ

Նշեք յուրաքանչյուր տարր, երբ ավարտված է:

- [ ] Հրահանգների օգտակար բեռները բեմադրված, հեշավորված և կցված:
- [ ] Կազմաձևման հատվածի հեշը գրանցվեց:
- [ ] Դետերմինիստական ​​թեստային տեղեկամատյանները գրավված + հաշված:
- [ ] Կյանքի ցիկլի նկար + ամփոփումն արտահանվել է:
- [ ] Ստեղծվել է ապացույցների մանիֆեստ և գրանցված հեշ:
- [ ] Իրադարձությունների/հեռաչափության արտահանումները գրավված են + հեշացված:
- [ ] Երկկողմանի հսկողության հաստատումները արխիվացված են:
- [ ] GAR/րոպե վերբեռնված; վերևում արձանագրված ամփոփում:

Այս ձևանմուշը յուրաքանչյուր փաթեթի կողքին պահելը պահպանում է կառավարման DAG-ը
որոշիչ և աուդիտորներին տրամադրում է շարժական մանիֆեստ ռեպո կյանքի ցիկլի համար
որոշումներ։