---
lang: kk
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

# Android шығарылымын тексеру тізімі (AND6)

Бұл бақылау тізімі **ЖӘНЕ6 — CI және Compliance Hardening** қақпаларын қамтиды
`roadmap.md` (§Басымдылық 5). Ол Android SDK шығарылымдарын Rust нұсқасымен теңестіреді
CI тапсырмаларын, сәйкестік артефактілерін жазу арқылы RFC күтулерін босатыңыз,
GA алдында тіркелуі керек құрылғы-зертхана дәлелдері және шығу тегі,
LTS немесе түзету пойызы алға жылжиды.

Бұл құжатты мыналармен бірге пайдаланыңыз:

- `docs/source/android_support_playbook.md` — шығарылым күнтізбесі, SLA және
  эскалация ағашы.
- `docs/source/android_runbook.md` — күнделікті операциялық жұмыс кітаптары.
- `docs/source/compliance/android/and6_compliance_checklist.md` — реттегіш
  артефактілер тізімдемесі.
- `docs/source/release_dual_track_runbook.md` — екі жолды шығаруды басқару.

## 1. Сахна қақпалары бір қарағанда

| Кезең | Қажетті қақпалар | Дәлелдер |
|-------|----------------|----------|
| **T−7 күн (алдын ала мұздату)** | 14 күн бойы түнгі `ci/run_android_tests.sh` жасыл; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` және `ci/check_android_docs_i18n.sh` өту; линт/тәуелділік сканерлері кезекке қойылды. | Buildkite бақылау тақталары, құрылғылардың айырмашылығы туралы есеп, скриншоттың суретінің үлгісі. |
| **T−3 күн (RC жарнамасы)** | Құрылғының зертханалық резерві расталды; StrongBox attestation CI іске қосу (`scripts/android_strongbox_attestation_ci.sh`); Жоспарланған аппараттық құралдарда орындалатын робоэлектрлік/аспаптық жинақтар; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` таза. | CSV құрылғы матрицасы, аттестаттау бумасы манифесті, Gradle есептер `artifacts/android/lint/<version>/` астында мұрағатталған. |
| **T−1 күн (бару/жоқ)** | Телеметриялық редакция күйінің жинағы жаңартылды (`scripts/telemetry/check_redaction_status.py --write-cache`); `and6_compliance_checklist.md` сәйкес жаңартылған сәйкестік артефактілері; шығу репетициясы аяқталды (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, телеметрия күйі JSON, құрғақ іске қосу журналы. |
| **T0 (GA/LTS кесу)** | `scripts/publish_android_sdk.sh --dry-run` аяқталды; шығу тегі + SBOM қол қойылған; экспортталған және бару/шығуға тыйым салынған минуттарға тіркелген босату тексеру парағы; `ci/sdk_sorafs_orchestrator.sh` түтін жұмысы жасыл. | RFC тіркемелерін, Sigstore бумасын, `artifacts/android/` астында қабылдау артефактілерін шығарыңыз. |
| **T+1 күн (кесілгеннен кейінгі)** | Түзетуге дайындығы тексерілді (`scripts/publish_android_sdk.sh --validate-bundle`); бақылау тақтасының айырмашылықтары қаралды (`ci/check_android_dashboard_parity.sh`); `status.md` ішіне жүктеп салынған дәлелдер пакеті. | Бақылау тақтасының айырмашылығы экспорты, `status.md` жазбасына сілтеме, мұрағатталған шығарылым пакеті. |

## 2. CI және сапа қақпасының матрицасы| Қақпа | Команда(лар) / Сценарий | Ескертпелер |
|------|--------------------|-------|
| Бірлік + интеграциялық сынақтар | `ci/run_android_tests.sh` (`ci/run_android_tests.sh` орау) | `artifacts/android/tests/test-summary.json` + сынақ журналын шығарады. Norito кодек, кезек, StrongBox резерві және Torii клиенттік жабдық сынақтарын қамтиды. Түнде және белгілеу алдында қажет. |
| Фикстура паритеті | `ci/check_android_fixtures.sh` (`scripts/check_android_fixtures.py` орады) | Қалпына келтірілген Norito құрылғыларының Rust канондық жиынтығына сәйкестігін қамтамасыз етеді; қақпа сәтсіз болғанда JSON дифференциалын тіркеңіз. |
| Үлгі қолданбалар | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` құрастырады және `scripts/android_sample_localization.py` арқылы локализацияланған скриншоттарды тексереді. |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | README + жергілікті жылдам іске қосуларды қорғайды. Құжат өңдеулері шығарылым тармағына түскеннен кейін қайта іске қосыңыз. |
| Бақылау тақтасының паритеті | `ci/check_android_dashboard_parity.sh` | CI/экспортталған көрсеткіштердің Rust аналогтарымен сәйкестігін растайды; T+1 тексеру кезінде қажет. |
| SDK қабылдау түтіні | `ci/sdk_sorafs_orchestrator.sh` | Ағымдағы SDK-мен көп көзді Sorafs оркестрінің байланыстарын орындайды. Сахналанған артефактілерді жүктеп салу алдында қажет. |
| Аттестацияны тексеру | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` астында StrongBox/TEE аттестаттау жинақтарын біріктіреді; қорытындыны GA пакеттеріне тіркеңіз. |
| Құрылғы-зертхана ұясының валидациясы | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Пакеттерді шығару үшін дәлелдемелерді тіркемес бұрын аспаптық жинақтарды растайды; CI `fixtures/android/device_lab/slot-sample` үлгі ұяшығымен жұмыс істейді (телеметрия/аттестация/кезекте/журналдар + `sha256sum.txt`). |

> **Кеңес:** бұл тапсырмаларды `android-release` Buildkite құбырына қосыңыз, сонда
> мұздату апталары әр қақпаны босату тармағының ұшымен автоматты түрде қайта іске қосыңыз.

Біріктірілген `.github/workflows/android-and6.yml` тапсырмасы түкті,
Әрбір PR/push бойынша сынақ жинағы, аттестаттау-резюме және құрылғы-зертхана ұяшықтарын тексеру
Android көздеріне қол тигізу, `artifacts/android/{lint,tests,attestation,device_lab}/` астында дәлелдерді жүктеп салу.

## 3. Линт және тәуелділік сканерлері

`scripts/android_lint_checks.sh --version <semver>` репо түбірінен іске қосыңыз. The
сценарий орындалады:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Есептер мен тәуелділікті қорғау шығыстары астында мұрағатталады
  `artifacts/android/lint/<label>/` және шығаруға арналған `latest/` символдық сілтемесі
  құбырлар.
- Сәтсіз түкті анықтаулар түзетуді немесе шығарылымдағы жазбаны қажет етеді
  Қабылданған тәуекелді құжаттайтын RFC (Release Engineering + Бағдарламасымен бекітілген
  қорғасын).
- `dependencyGuardBaseline` тәуелділік құлпын қалпына келтіреді; дифференциалды тіркеңіз
  бару/жоқ пакетіне.

## 4. Device Lab & StrongBox қамтуы

1. Сілтемедегі сыйымдылықты бақылау құралын пайдаланып Pixel + Galaxy құрылғыларын резервтеңіз
   `docs/source/compliance/android/device_lab_contingency.md`. Шығарылымдарды блоктайды
   егер ` аттестаттау есебін жаңарту үшін.
3. Аспаптар матрицасын іске қосыңыз (құрылғыдағы жинақ/ABI тізімін құжаттаңыз
   трекер). Қайталау сәтті болса да, оқиға журналындағы сәтсіздіктерді түсіріңіз.
4. Firebase сынақ зертханасына қайта оралу қажет болса, билетті толтырыңыз; билетті байланыстырыңыз
   төмендегі бақылау парағында.

## 5. Сәйкестік және телеметрия артефактілері- ЕО үшін `docs/source/compliance/android/and6_compliance_checklist.md` орындаңыз
  және JP жіберулері. `docs/source/compliance/android/evidence_log.csv` жаңарту
  хэштермен + Buildkite тапсырмасының URL мекенжайларымен.
- Телеметриялық өңдеу дәлелдерін жаңарту арқылы
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Алынған JSON файлын астында сақтаңыз
  `artifacts/android/telemetry/<version>/status.json`.
- Схема айырмашылығының шығысын жазыңыз
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust экспорттаушыларымен теңдікті дәлелдеу.

## 6. Шығу, SBOM және Publishing

1. Жариялау құбырын құрғатыңыз:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore шығу тегі:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` тіркеп, қол қойылған
   `checksums.sha256` RFC шығарылымына.
4. Нағыз Maven репозиторийіне жылжыту кезінде қайта іске қосыңыз
   `scripts/publish_android_sdk.sh` `--dry-run` жоқ, консольді түсіріңіз
   журналға жазып, алынған артефактілерді `artifacts/android/maven/<semver>` жүйесіне жүктеңіз.

## 7. Жіберу пакетінің үлгісі

Әрбір GA/LTS/түзету шығарылымы мыналарды қамтуы керек:

1. **Аяқталған бақылау тізімі** — осы файлдың кестесін көшіріп, әрбір элементті белгілеңіз және сілтеме қойыңыз
   қолдау артефактілеріне (Buildkite іске қосу, журналдар, құжат айырмашылықтары).
2. **Құрылғы зертханасының дәлелі** — аттестаттау есебінің қысқаша мазмұны, брондау журналы және
   кез келген күтпеген белсендірулер.
3. **Телеметриялық пакет** — өңдеу күйі JSON, схема айырмашылығы, сілтеме
   `docs/source/sdk/android/telemetry_redaction.md` жаңартулары (бар болса).
4. **Сәйкестік артефактілері** — сәйкестік қалтасына қосылған/жаңартылған жазбалар
   плюс жаңартылған дәлелдер журналы CSV.
5. **Провенанс жинағы** — SBOM, Sigstore қолтаңбасы және `checksums.sha256`.
6. **Шығарылымның қысқаша мазмұны** — бір беттік шолу `status.md` түйіндемесіне қоса беріледі
   жоғарыда көрсетілген (күні, нұсқасы, кез келген бас тартылған қақпаларды бөлектеу).

Пакетті `artifacts/android/releases/<version>/` астында сақтаңыз және оған сілтеме жасаңыз
`status.md` және RFC шығарылымында.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` автоматты түрде
  соңғы линт мұрағатын (`artifacts/android/lint/latest`) және көшіреді
  сәйкестік дәлелдері журналына `artifacts/android/releases/<version>/`, осылайша
  жіберу пакеті әрқашан канондық орынға ие болады.

---

**Ескерту:** бұл бақылау тізімін жаңа CI тапсырмалары, сәйкестік артефактілері,
немесе телеметрия талаптары қосылады. Жол картасының AND6 тармағы мына уақытқа дейін ашық қалады
бақылау тізімі және онымен байланысты автоматтандыру екі қатарынан шығарылым үшін тұрақты болып табылады
пойыздар.