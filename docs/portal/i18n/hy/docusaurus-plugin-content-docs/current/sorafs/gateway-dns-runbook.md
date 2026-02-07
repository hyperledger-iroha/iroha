---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Gateway & DNS Kickoff Runbook

Այս պորտալի պատճենը արտացոլում է կանոնական մատյանը
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md):
Այն գրավում է ապակենտրոնացված DNS-ի և Gateway-ի գործառնական պահակակետերը
աշխատանքային հոսք, որպեսզի ցանցային, օպերատիվ և փաստաթղթերի առաջատարները կարողանան կրկնել դրանք
ավտոմատացման կույտ 2025-03 թվականների մեկնարկից առաջ:

## Շրջանակ և առաքում

- Կապեք DNS (SF‑4) և gateway (SF‑5) նշաձողերը՝ փորձելով դետերմինիստական
  հյուրընկալող ածանցում, լուծիչի գրացուցակի թողարկումներ, TLS/GAR ավտոմատացում և ապացույցներ
  գրավել.
- Պահպանեք մեկնարկային մուտքերը (օրակարգ, հրավիրում, հաճախումների հետագծում, GAR հեռաչափություն
  snapshot) սինխրոնիզացված սեփականատերերի վերջին հանձնարարությունների հետ:
- Ստեղծեք աուդիտի ենթակա արտեֆակտների փաթեթ կառավարման վերանայողների համար. լուծիչ
  գրացուցակի թողարկման նշումներ, դարպասների հետաքննության տեղեկամատյաններ, համապատասխանության զրահի ելք և
  Docs/DevRel ամփոփագիրը:

## Դերեր և պարտականություններ

| Աշխատանքային հոսք | Պարտականություններ | Պահանջվող արտեֆակտներ |
|------------------------------------------------------|
| Ցանցային TL (DNS կույտ) | Պահպանեք դետերմինիստական ​​հյուրընկալող պլանը, գործարկեք RAD գրացուցակի թողարկումները, հրապարակեք լուծիչի հեռաչափության մուտքերը: | `artifacts/soradns_directory/<ts>/`, տարբերվում է `docs/source/soradns/deterministic_hosts.md`-ի, RAD մետատվյալների համար: |
| Օպերացիոն ավտոմատացման կապար (դարպաս) | Կատարեք TLS/ECH/GAR ավտոմատացման վարժություններ, գործարկեք `sorafs-gateway-probe`, թարմացրեք PagerDuty կեռիկները: | `artifacts/sorafs_gateway_probe/<ts>/`, զոնդ JSON, `ops/drill-log.md` գրառումներ: |
| QA Guild & Tooling WG | Գործարկել `ci/check_sorafs_gateway_conformance.sh`, մշակել հարմարանքները, արխիվացնել Norito ինքնահաստատման փաթեթները: | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Փաստաթղթեր / DevRel | Ձեռք բերեք րոպեները, թարմացրեք դիզայնի նախապես կարդացված + հավելվածները և հրապարակեք ապացույցների ամփոփագիրը այս պորտալում: | Թարմացված `docs/source/sorafs_gateway_dns_design_*.md` ֆայլեր և տեղադրման նշումներ: |

## Ներածումներ և նախադրյալներ

- Հոսթի դետերմինիստական սպեցիֆիկացիա (`docs/source/soradns/deterministic_hosts.md`) և
  լուծիչի ատեստավորման փայտամած (`docs/source/soradns/resolver_attestation_directory.md`):
- Gateway artefacts. օպերատորի ձեռնարկ, TLS/ECH ավտոմատացման օգնականներ,
  Ուղղակի ռեժիմի ուղղորդում և աշխատանքի ինքնահաստատում `docs/source/sorafs_gateway_*`-ի ներքո:
- Գործիքավորում՝ `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` և CI օգնականներ
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`):
- Գաղտնիքներ՝ GAR թողարկման բանալի, DNS/TLS ACME հավատարմագրեր, PagerDuty երթուղային բանալի,
  Torii վավերացման նշան լուծիչի բեռնման համար:

## Թռիչքից առաջ ստուգաթերթ

1. Հաստատեք ներկաներին և օրակարգը՝ թարմացնելով
   `docs/source/sorafs_gateway_dns_design_attendance.md` և շրջանառվում է
   ընթացիկ օրակարգ (`docs/source/sorafs_gateway_dns_design_agenda.md`):
2. Բեմական արտեֆակտ արմատներ, ինչպիսիք են
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` և
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Թարմացրեք հարմարանքները (GAR մանիֆեստներ, RAD ապացույցներ, դարպասների համապատասխանության փաթեթներ) և
   համոզվեք, որ `git submodule` վիճակը համապատասխանում է վերջին փորձի պիտակին:
4. Ստուգեք գաղտնիքները (Ed25519 թողարկման բանալի, ACME հաշվի ֆայլ, PagerDuty նշան)
   ներկայացնել և համընկնել պահոցների ստուգման գումարները:
5. Ծխի փորձարկման հեռաչափության թիրախներ (Pushgateway վերջնակետ, GAR Grafana տախտակ) առաջ
   դեպի փորված.

## Ավտոմատացման փորձի քայլեր

### Որոշիչ հյուրընկալող քարտեզ և RAD գրացուցակի թողարկում

1. Գործարկեք դետերմինիստական հյուրընկալողի դերիվացիայի օգնականը առաջարկվող մանիֆեստի դեմ
   սահմանել և հաստատել, որ որևէ շեղում չկա
   `docs/source/soradns/deterministic_hosts.md`.
2. Ստեղծեք լուծիչի գրացուցակի փաթեթ.

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Գրանցեք տպագիր գրացուցակի ID-ն, SHA-256-ը և ելքային ուղիները ներսում
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` և մեկնարկը
   րոպե.

### DNS հեռաչափության գրավում

- Պոչերի լուծիչի թափանցիկության տեղեկամատյանները ≥10 րոպեի ընթացքում օգտագործելով
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Արտահանեք Pushgateway-ի չափումները և արխիվացրեք NDJSON-ի նկարները վազքի հետ մեկտեղ
  ID տեղեկատու.

### Դարպասների ավտոմատացման վարժանքներ

1. Կատարեք TLS/ECH զոնդը՝

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Գործարկեք համապատասխանության ամրագոտին (`ci/check_sorafs_gateway_conformance.sh`) և
   ինքնավստահ օգնականը (`scripts/sorafs_gateway_self_cert.sh`) թարմացնելու համար
   Norito ատեստավորման փաթեթ:
3. Լուսանկարեք PagerDuty/Webhook-ի իրադարձությունները՝ ապացուցելու ավտոմատացման ուղու ավարտը
   վերջ.

### Ապացույցների փաթեթավորում

- Թարմացրեք `ops/drill-log.md`-ը ժամանակի դրոշմանիշներով, մասնակիցների և հետաքննության հեշերով:
- Պահպանեք արտեֆակտները գործարկվող ID դիրեկտորիաների տակ և հրապարակեք գործադիր ամփոփագիրը
  Docs/DevRel հանդիպման արձանագրության մեջ:
- Միացրեք ապացույցների փաթեթը կառավարման տոմսում նախքան մեկնարկային ստուգումը:

## Նիստի հեշտացում և ապացույցների հանձնում

- ** Մոդերատորի ժամանակացույցը:**  
  - T‑24h — Ծրագրի կառավարումը տեղադրում է հիշեցում + օրակարգ/հաճախումների նկար `#nexus-steering`-ում:  
  - T‑2h — Ցանցային TL-ը թարմացնում է GAR հեռաչափության պատկերը և գրանցում դելտաները `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`-ում:  
  - T‑15m — Ops Automation-ը ստուգում է զոնդի պատրաստությունը և գրում է ակտիվ գործարկման ID-ն `artifacts/sorafs_gateway_dns/current`-ում:  
  - Զանգի ընթացքում — Մոդերատորը կիսում է այս գրքույկը և նշանակում է կենդանի գրագիր. Docs/DevRel գործողությունների տարրերը ներկառուցված են:
- **Րոպե ձևանմուշ.** Պատճենել կմախքը
  `docs/source/sorafs_gateway_dns_design_minutes.md` (նաև արտացոլված է պորտալում
  փաթեթ) և կատարեք մեկ լրացված օրինակ յուրաքանչյուր նստաշրջանի համար: Ներառեք մասնակիցների ցուցակը,
  որոշումներ, գործողությունների տարրեր, ապացույցների հեշեր և չմարված ռիսկեր:
- **Ապացույցների վերբեռնում. ** Կպցրեք `runbook_bundle/` գրացուցակը փորձից,
  կցել կազմված արձանագրությունները PDF, արձանագրել SHA-256 հեշերը արձանագրություններում + օրակարգում,
  այնուհետև ping-ով ուղարկեք կառավարման վերանայող կեղծանունը, երբ հողը վերբեռնվի
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Ապացույցի նկար (2025 թվականի մարտի մեկնարկ)

Վերջին փորձը/կենդանի արտեֆակտները, որոնք նշված են ճանապարհային քարտեզում և կառավարման մեջ
րոպե ապրել `s3://sora-governance/sorafs/gateway_dns/` դույլի տակ: Հեշ
ներքևում արտացոլում են կանոնական մանիֆեստը (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`):

- **Չոր վազք — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Փաթեթ թարբոլ՝ `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Րոպեներ PDF՝ `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Կենդանի արհեստանոց — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Սպասող վերբեռնում. `gateway_dns_minutes_20250303.pdf` — Docs/DevRel-ը կավելացնի SHA-256-ը, երբ ստացված PDF-ը հայտնվի փաթեթում:)_

## Հարակից նյութ

- [Դարպասի գործառնությունների գրքույկ] (./operations-playbook.md)
- [SoraFS դիտելիության պլան] (./observability-plan.md)
- [Ապակենտրոնացված DNS և Gateway որոնիչ] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)