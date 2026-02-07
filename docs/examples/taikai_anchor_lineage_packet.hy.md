---
lang: hy
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage փաթեթի ձևանմուշ (SN13-C)

Ճանապարհային քարտեզի կետը **SN13-C — Manifests & SoraNS anchors** պահանջում է յուրաքանչյուր այլանուն
ռոտացիա՝ դետերմինիստական ապացույցների փաթեթ ուղարկելու համար: Պատճենեք այս ձևանմուշը ձեր մեջ
արտեֆակտ գրացուցակի տեղադրում (օրինակ
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) և փոխարինել
տեղապահները նախքան փաթեթը կառավարման հանձնելը:

## 1. Մետատվյալներ

| Դաշտային | Արժեք |
|-------|-------|
| Միջոցառման ID | `<taikai.event.launch-2026-07-10>` |
| Հեռարձակում / կատարում | `<main-stage>` |
| Այլանուն անվանատարածք / անունը | `<sora / docs>` |
| Ապացույցների տեղեկատու | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Օպերատորի հետ կապ | `<name + email>` |
| GAR / RPT տոմս | `<governance ticket or GAR digest>` |

## Փաթեթի օգնական (ըստ ցանկության)

Պատճենեք կծիկի արտեֆակտները և թողարկեք JSON (ըստ ցանկության ստորագրված) ամփոփագիր
լրացնելով մնացած բաժինները.

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Օգնականը քաշում է `taikai-anchor-request-*`, `taikai-trm-state-*`,
`taikai-lineage-*`, ծրարներ և պահապաններ Taikai կծիկի գրացուցակից դուրս
(`config.da_ingest.manifest_store_dir/taikai`), ուստի ապացույցների թղթապանակն արդեն
պարունակում է ստորև նշված ճշգրիտ ֆայլերը:

## 2. Տոհմային մատյան և հուշում

Կցեք և՛ սկավառակի տոհմային մատյանը, և՛ JSON Torii-ի ակնարկը, որը գրել է դրա համար
պատուհան. Սրանք ուղղակիորեն գալիս են
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` և
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Տոհմային մատյան | `taikai-trm-state-docs.json` | `<sha256>` | Ապացուցում է նախորդ մանիֆեստի ամփոփումը/պատուհանը: |
| Տոհմային ակնարկ | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Նկարված է SoraNS խարիսխում վերբեռնելուց առաջ: |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Խարիսխի օգտակար բեռի գրավում

Գրանցեք POST-ի ծանրաբեռնվածությունը, որը Torii-ը տրամադրել է խարիսխ ծառայությանը: Օգտակար բեռը
ներառում է `envelope_base64`, `ssm_base64`, `trm_base64` և inline
`lineage_hint` օբյեկտ; աուդիտները հիմնվում են այս գրավման վրա՝ ապացուցելու այն ակնարկը, որը եղել է
ուղարկվել է SoraNS-ին։ Torii-ն այժմ գրում է այս JSON-ը ավտոմատ կերպով որպես
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool գրացուցակի ներսում (`config.da_ingest.manifest_store_dir/taikai/`), այսպես
օպերատորները կարող են այն ուղղակիորեն պատճենել՝ HTTP տեղեկամատյանները քերելու փոխարեն:

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Խարիսխ ՓՈՍՏ | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Հում հարցումը պատճենված է `taikai-anchor-request-*.json`-ից (Taikai spool): |

## 4. Դիջեստի բացահայտ ճանաչում

| Դաշտային | Արժեք |
|-------|-------|
| Նոր մանիֆեստի ամփոփում | `<hex digest>` |
| Նախորդ մանիֆեստը մարսել (ակնարկից) | `<hex digest>` |
| Պատուհանի սկիզբ / ավարտ | `<start seq> / <end seq>` |
| Ընդունման ժամանակացույց | `<ISO8601>` |

Հղեք վերևում գրանցված մատյան/ակնարկ հեշերին, որպեսզի վերանայողները կարողանան ստուգել այն
պատուհան, որը փոխարինվել է:

## 5. Չափումներ / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` լուսանկար՝ `<Prometheus query + export path>`
- `/status taikai_alias_rotations` աղբանոց (ըստ այլանունի)՝ `<file path + hash>`

Տրամադրեք Prometheus/Grafana արտահանումը կամ `curl` ելքը, որը ցույց է տալիս հաշվիչը
ավելացում և `/status` զանգված այս անունի համար:

## 6. Մանիֆեստ ապացույցների գրացուցակի համար

Ստեղծեք ապացույցների գրացուցակի դետերմինիստական մանիֆեստ (սպոլ ֆայլեր,
ծանրաբեռնվածության գրավում, չափումների նկարներ), որպեսզի կառավարումը կարողանա ստուգել յուրաքանչյուր հեշ առանց
բացել արխիվը:

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Արտեֆակտ | Ֆայլ | SHA-256 | Ծանոթագրություններ |
|----------|------|---------|-------|
| Ապացույցների դրսևորում | `manifest.json` | `<sha256>` | Կցեք սա կառավարման փաթեթին / GAR: |

## 7. ստուգաթերթ

- [ ] Lineage մատյանը պատճենված + հաշված:
- [ ] Lineage ակնարկը պատճենված է + հեշացված:
- [ ] Anchor POST-ի օգտակար բեռը գրավված և հեշացված է:
- [ ] Մանիֆեստի ամփոփման աղյուսակը լրացված է:
- [ ] Չափման ակնարկներ արտահանված (`taikai_trm_alias_rotations_total`, `/status`):
- [ ] Մանիֆեստը ստեղծվել է `scripts/repo_evidence_manifest.py`-ով:
- [ ] Փաթեթը վերբեռնված է կառավարման համակարգ՝ հեշերով + կոնտակտային տվյալներով:

Այս ձևանմուշի պահպանումը յուրաքանչյուր այլ կեղծանունի ռոտացիայի համար պահպանում է SoraNS-ի կառավարումը
փաթեթը վերարտադրելի է և կապում է տոհմային ակնարկները ուղղակիորեն GAR/RPT ապացույցներին: