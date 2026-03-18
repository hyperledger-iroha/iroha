---
lang: kk
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI және SDK — Шығарылым жазбалары (v0.1.0)

## Ерекшеліктер
- `sorafs_cli` енді бүкіл орау құбырын орап алады (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) сондықтан CI жүгірушілері
  тапсырыс көмекшілердің орнына бір екілік. Жаңа кілтсіз қол қою ағыны әдепкі бойынша
  `SIGSTORE_ID_TOKEN`, GitHub әрекеттері OIDC провайдерлерін түсінеді және детерминистикалық шығарады
  қолтаңба бумасының жанында жиынтық JSON.
- Көп дереккөзді алу *көрсеткіш тақтасы* `sorafs_car` бөлігі ретінде жіберіледі: ол қалыпқа келеді
  провайдердің телеметриясы, мүмкіндік жазаларын қолданады, JSON/Norito есептерін сақтайды және
  ортақ тізбе дескрипторы арқылы оркестр симуляторын (`sorafs_fetch`) береді.
  `fixtures/sorafs_manifest/ci_sample/` астындағы арматуралар детерминацияны көрсетеді
  CI/CD айырмашылығы күтілетін кірістер мен шығыстар.
- Шығарылымды автоматтандыру `ci/check_sorafs_cli_release.sh` және кодталады
  `scripts/release_sorafs_cli.sh`. Әрбір шығарылым енді манифест бумасын мұрағаттайды,
  қолтаңба, `manifest.sign/verify` қорытындылары және таблоның суреті осылайша басқару
  шолушылар құбырды қайта іске қоспай-ақ артефактілерді бақылай алады.

## Жаңарту қадамдары
1. Жұмыс кеңістігіңіздегі тураланған жәшіктерді жаңартыңыз:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. fmt/clippy/сынақ қамтуын растау үшін босату қақпасын жергілікті (немесе CI бойынша) қайта іске қосыңыз:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Таңдалған конфигурациямен қол қойылған артефактілер мен қорытындыларды қалпына келтіріңіз:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Жаңартылған бумаларды/дәлелдемелерді `fixtures/sorafs_manifest/ci_sample/` ішіне көшіріңіз
   шығарылым жаңартулары канондық арматура.

## Тексеру
- Шығару қақпасы: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` қақпа сәтті болғаннан кейін бірден).
- `ci/check_sorafs_cli_release.sh` шығысы: мұрағатталған
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (шығару жинағына тіркелген).
- Манифест жинағы дайджест: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Дәлелдеу жиынтық дайджесті: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Манифест дайджесті (төменгі ағындағы аттестацияны салыстыру үшін):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json` бастап).

## Операторларға арналған ескертпелер
- Torii шлюзі енді `X-Sora-Chunk-Range` мүмкіндік тақырыбын қамтамасыз етеді. Жаңарту
  рұқсат тізімдері, осылайша жаңа ағын таңбалауыш аумақтарын ұсынатын клиенттер қабылданады; ескі белгілер
  диапазонсыз шағым тоқтатылады.
- `scripts/sorafs_gateway_self_cert.sh` манифестті тексеруді біріктіреді. Жүгіру кезінде
  өзін-өзі растайтын белдік, орауыш мүмкін болатындай жаңадан жасалған манифест бумасын жеткізіңіз
  қолтаңба дрейфінде тез сәтсіздікке ұшырайды.
- Телеметриялық бақылау тақталары жаңа көрсеткіштер тақтасының экспортын (`scoreboard.json`) қабылдауы керек.
  жеткізушінің жарамдылығын, салмақ тағайындауларын және бас тарту себептерін салыстырыңыз.
- Әр шығарылыммен төрт канондық қорытындыны мұрағаттау:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Басқару билеттері кезінде осы нақты файлдарға сілтеме жасайды
  бекіту.

## Алғыс
- Storage Team — CLI консолидациясы, блок-жоспар рендерері және көрсеткіштер тақтасы
  телеметриялық сантехника.
- Құралдар ЖТ — шығару құбыры (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) және детерминирленген арматура жинағы.
- Gateway Operations — мүмкіндіктерді бекіту, ағынды-токен саясатын шолу және жаңартылған
  өзін-өзі сертификаттау оқулықтары.