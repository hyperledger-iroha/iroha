---
lang: hy
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android թողարկման ստուգաթերթ (AND6)

Այս ստուգաթերթը ներառում է **AND6 — CI & Compliance Hardening** դարպասները
`roadmap.md` (§Առաջնահերթություն 5): Այն հավասարեցնում է Android SDK թողարկումները Rust-ի հետ
թողարկեք RFC-ի ակնկալիքները՝ ուղղագրելով CI-ի աշխատանքները, համապատասխանության արտեֆակտները,
սարք-լաբորատոր ապացույցներ և ծագման փաթեթներ, որոնք պետք է կցվեն մինչև GA,
LTS-ը կամ թեժ ուղղման գնացքը շարժվում է առաջ:

Օգտագործեք այս փաստաթուղթը հետևյալի հետ միասին.

- `docs/source/android_support_playbook.md` — թողարկման օրացույց, SLA-ներ և
  էսկալացիայի ծառ.
- `docs/source/android_runbook.md` — ամենօրյա գործառնական մատյաններ:
- `docs/source/compliance/android/and6_compliance_checklist.md` — կարգավորիչ
  արտեֆակտ գույքագրում.
- `docs/source/release_dual_track_runbook.md` — երկակի ուղու թողարկման կառավարում:

## 1. Բեմական դարպասները մի հայացքով

| Բեմական | Պահանջվող դարպասներ | Ապացույցներ |
|-------|----------------|----------|
| **T−7 օր (նախնական սառեցում)** | Գիշերային `ci/run_android_tests.sh` կանաչ 14 օր; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` և `ci/check_android_docs_i18n.sh` անցում; lint/dependency scans հերթագրված: | Buildkite-ի վահանակներ, հարմարանքների տարբերությունների հաշվետվություն, սքրինշոթի լուսանկարների օրինակ: |
| **T−3 օր (RC առաջխաղացում)** | Սարքի լաբորատորիայի ամրագրումը հաստատված է. StrongBox ատեստավորման CI գործարկում (`scripts/android_strongbox_attestation_ci.sh`); Ռոբոէլեկտրական/գործիքավորված հավաքակազմ, որն իրականացվում է պլանավորված սարքավորումների վրա; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` մաքուր. | Սարքի մատրիցա CSV, ատեստավորման փաթեթի մանիֆեստ, Gradle հաշվետվությունները արխիվացված են `artifacts/android/lint/<version>/` տակ: |
| **T−1 օր (գնալ/չգնալ)** | Հեռաչափության խմբագրման կարգավիճակի փաթեթը թարմացվել է (`scripts/telemetry/check_redaction_status.py --write-cache`); համապատասխանության արտեֆակտները թարմացվել են ըստ `and6_compliance_checklist.md`-ի; ծագման փորձն ավարտված է (`scripts/android_sbom_provenance.sh --dry-run`): | `docs/source/compliance/android/evidence_log.csv`, հեռաչափության կարգավիճակ JSON, ծագման չոր գործարկման մատյան: |
| **T0 (GA/LTS կտրող)** | `scripts/publish_android_sdk.sh --dry-run` ավարտված; ծագումը + SBOM ստորագրված; թողարկման ստուգաթերթը արտահանվել և կցվել է go/no-go րոպեներին. `ci/sdk_sorafs_orchestrator.sh` ծխի աշխատանք կանաչ. | Թողարկեք RFC կցորդները, Sigstore փաթեթը, ընդունման արտեֆակտները `artifacts/android/`-ի ներքո: |
| **T+1 օր (կտրումից հետո)** | Թեժ շտկման պատրաստությունը ստուգված է (`scripts/publish_android_sdk.sh --validate-bundle`); Վերանայված վահանակի տարբերությունները (`ci/check_android_dashboard_parity.sh`); ապացույցների փաթեթ՝ վերբեռնված `status.md`-ում: | Վահանակի տարբերությունների արտահանում, հղում դեպի `status.md` մուտքի, արխիվացված թողարկման փաթեթ: |

## 2. CI & Quality Gate Matrix| Դարպաս | Հրաման(ներ) / Սցենար | Ծանոթագրություններ |
|------|--------------------|-------|
| Միավոր + ինտեգրացիոն թեստեր | `ci/run_android_tests.sh` (փաթաթում է `ci/run_android_tests.sh`) | Արտանետում է `artifacts/android/tests/test-summary.json` + թեստային մատյան: Ներառում է Norito կոդեկ, հերթ, StrongBox հետադարձ և Torii հաճախորդի ամրացման թեստեր: Պահանջվում է գիշերը և նախքան պիտակավորումը: |
| Հարմարավետության հավասարություն | `ci/check_android_fixtures.sh` (փաթաթում է `scripts/check_android_fixtures.py`) | Ապահովում է, որ վերականգնված Norito հարմարանքները համապատասխանում են Rust կանոնական հավաքածուին. կցել JSON տարբերությունը, երբ դարպասը ձախողվում է: |
| Նմուշ հավելվածներ | `ci/check_android_samples.sh` | Կառուցում է `examples/android/{operator-console,retail-wallet}` և հաստատում տեղայնացված սքրինշոթները `scripts/android_sample_localization.py`-ի միջոցով: |
| Փաստաթղթեր/I18N | `ci/check_android_docs_i18n.sh` | Պահակներ README + տեղայնացված արագ մեկնարկներ: Կրկին գործարկեք այն բանից հետո, երբ փաստաթղթի խմբագրումները վայրէջք կատարեն թողարկման ճյուղում: |
| Վահանակի հավասարություն | `ci/check_android_dashboard_parity.sh` | Հաստատում է, որ CI/արտահանված չափումները համապատասխանում են Rust-ի գործընկերներին. պահանջվում է T+1 ստուգման ժամանակ: |
| SDK ընդունման ծուխ | `ci/sdk_sorafs_orchestrator.sh` | Իրականացնում է բազմաղբյուր Sorafs նվագախմբի կապերը ընթացիկ SDK-ի հետ: Պահանջվում է նախքան բեմականացված արտեֆակտները վերբեռնելը: |
| Ատեստավորման ստուգում | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Միավորում է StrongBox/TEE ատեստավորման փաթեթները `artifacts/android/attestation/**`-ի ներքո; կցել ամփոփագիրը GA փաթեթներին: |
| Սարքի լաբորատորիայի բնիկի վավերացում | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Վավերացնում է գործիքակազմի փաթեթները՝ նախքան փաթեթները թողարկելու ապացույցներ կցելը. CI-ն աշխատում է `fixtures/android/device_lab/slot-sample`-ի նմուշի բնիկով (հեռաչափություն/հաստատում/հերթ/տեղեկամատյաններ + `sha256sum.txt`): |

> **Խորհուրդ.** ավելացրեք այս աշխատանքները `android-release` Buildkite խողովակաշարում, որպեսզի
> սառեցրեք շաբաթները ավտոմատ կերպով վերագործարկեք յուրաքանչյուր դարպաս՝ արձակման ճյուղի ծայրով:

Համախմբված `.github/workflows/android-and6.yml` աշխատանքը աշխատում է,
թեստային հավաքակազմ, ատեստավորում-ամփոփում և սարքի լաբորատոր անցք ստուգումներ յուրաքանչյուր PR/push-ում
դիպչելով Android աղբյուրներին, վերբեռնելով ապացույցներ `artifacts/android/{lint,tests,attestation,device_lab}/`-ի ներքո:

## 3. Lint & Dependency Scans

Գործարկեք `scripts/android_lint_checks.sh --version <semver>`-ը ռեպո արմատից: Այն
սցենարը կատարում է.

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Հաշվետվությունները և կախվածության պահպանության արդյունքները արխիվացված են տակ
  `artifacts/android/lint/<label>/` և `latest/` սիմվոլիկ թողարկման համար
  խողովակաշարեր.
- Անհաջող հայտնաբերումները պահանջում են կամ վերականգնում կամ մուտքագրում թողարկման մեջ
  RFC-ն փաստում է ընդունված ռիսկը (հաստատված է Release Engineering + Program-ի կողմից
  կապար):
- `dependencyGuardBaseline`-ը վերականգնում է կախվածության կողպեքը; կցել տարբերությունը
  դեպի go/no-go փաթեթը:

## 4. Սարքի լաբորատորիա և StrongBox ծածկույթ

1. Պահպանեք Pixel + Galaxy սարքերը՝ օգտագործելով հզորության հետագծիչը, որը նշված է
   `docs/source/compliance/android/device_lab_contingency.md`. Արգելափակում է թողարկումները
   ` ատեստավորման հաշվետվությունը թարմացնելու համար:
3. Գործարկեք գործիքավորման մատրիցը (փաստագրեք փաթեթի/ABI ցուցակը սարքում
   հետագծող): Նկարագրեք անհաջողությունները միջադեպերի մատյանում, նույնիսկ եթե կրկնվող փորձերը հաջողությամբ պսակվեն:
4. Ներկայացրեք տոմս, եթե պահանջվում է վերադարձ Firebase Test Lab-ին; կապել տոմսը
   ստորև բերված ստուգաթերթում:

## 5. Համապատասխանության և հեռաչափության արտեֆակտներ- Հետևեք `docs/source/compliance/android/and6_compliance_checklist.md`-ին ԵՄ-ի համար
  և JP-ի ներկայացումները: Թարմացրեք `docs/source/compliance/android/evidence_log.csv`
  հեշերով + Buildkite աշխատանքի URL-ներով:
- Թարմացրեք հեռաչափության խմբագրման ապացույցները միջոցով
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`:
  Ստացված JSON-ը պահեք տակ
  `artifacts/android/telemetry/<version>/status.json`.
- Գրանցեք սխեմայի տարբերությունը
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  ապացուցել Rust արտահանողների հետ հավասարությունը:

## 6. Ծագում, SBOM և հրատարակում

1. Չորացնել հրապարակման խողովակաշարը.

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Ստեղծեք SBOM + Sigstore ծագումը.

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Կցեք `artifacts/android/provenance/<semver>/manifest.json` եւ ստորագրված
   `checksums.sha256` RFC-ի թողարկման համար:
4. Երբ գովազդում եք իրական Maven պահոց, կրկնեք
   `scripts/publish_android_sdk.sh` առանց `--dry-run`, գրավեք վահանակը
   գրանցվեք և վերբեռնեք ստացված արտեֆակտները `artifacts/android/maven/<semver>`:

## 7. Ներկայացման փաթեթի ձևանմուշ

GA/LTS/hotfix-ի յուրաքանչյուր թողարկում պետք է ներառի.

1. **Լրացված ստուգաթերթ** — պատճենեք այս ֆայլի աղյուսակը, նշեք յուրաքանչյուր տարր և հղումը
   օժանդակ արտեֆակտներին (Buildkite գործարկում, տեղեկամատյաններ, փաստաթղթերի տարբերություններ):
2. **Սարքի լաբորատոր ապացույց** — ատեստավորման հաշվետվության ամփոփագիր, ամրագրումների մատյան և
   ցանկացած անկանխատեսելի ակտիվացում:
3. **Telemetry փաթեթ** — խմբագրման կարգավիճակ JSON, սխեմայի տարբերություն, հղում դեպի
   `docs/source/sdk/android/telemetry_redaction.md` թարմացումներ (եթե այդպիսիք կան):
4. **Համապատասխանության արտեֆակտներ** — համապատասխանության թղթապանակում ավելացված/թարմացված գրառումներ
   գումարած թարմացված ապացույցների մատյան CSV:
5. **Ծագման փաթեթ** — SBOM, Sigstore ստորագրություն և `checksums.sha256`:
6. **Թողարկման ամփոփագիր** — մեկ էջանոց ակնարկ կցված է `status.md` ամփոփմանը
   վերը նշվածը (ամսաթիվը, տարբերակը, ցանկացած բաց թողնված դարպասների ընդգծում):

Փաթեթը պահեք `artifacts/android/releases/<version>/` տակ և հղում կատարեք դրան
`status.md`-ում և RFC-ի թողարկումը:

- `scripts/run_release_pipeline.py --publish-android-sdk ...` ավտոմատ կերպով
  պատճենում է վերջին lint արխիվը (`artifacts/android/lint/latest`) և
  համապատասխանության ապացույց մուտք գործեք `artifacts/android/releases/<version>/`, որպեսզի
  ներկայացման փաթեթը միշտ ունի կանոնական գտնվելու վայրը:

---

**Հիշեցում.** թարմացրեք այս ստուգաթերթը, երբ նոր CI աշխատատեղեր, համապատասխանության արտեֆակտներ,
կամ ավելացվել են հեռաչափության պահանջները: Ճանապարհային քարտեզի AND6 կետը բաց է մնում մինչև
ստուգաթերթը և հարակից ավտոմատացումը կայուն են երկու անընդմեջ թողարկման համար
գնացքներ.