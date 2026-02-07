---
lang: kk
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI Арматура үлгілері

Бұл каталог үлгіден жасалған детерминирленген артефактілерді бумалайды
`fixtures/sorafs_manifest/ci_sample/` астында пайдалы жүктеме. Топтама көрсетеді
CI жұмыс үрдістері орындайтын SoraFS орау және қол қою құбыры.

## Артефактілерді түгендеу

| Файл | Сипаттама |
|------|-------------|
| `payload.txt` | Арматура сценарийлері пайдаланатын бастапқы пайдалы жүктеме (қарапайым мәтін үлгісі). |
| `payload.car` | `sorafs_cli car pack` шығарған CAR мұрағаты. |
| `car_summary.json` | Жиынтық дайджесттер мен метадеректерді түсіру үшін `car pack` арқылы жасалған. |
| `chunk_plan.json` | Бөлшек ауқымдары мен провайдердің күтулерін сипаттайтын JSON жоспарын алу. |
| `manifest.to` | `sorafs_cli manifest build` жасаған Norito манифесті. |
| `manifest.json` | Түзетуге арналған адам оқи алатын манифестті көрсету. |
| `proof.json` | `sorafs_cli proof verify` шығарған PoR қорытындысы. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` арқылы жасалған кілтсіз қолтаңба жинағы. |
| `manifest.sig` | Манифестке сәйкес бөлінген Ed25519 қолы. |
| `manifest.sign.summary.json` | Қол қою кезінде шығарылған CLI жиыны (хэштер, бума метадеректері). |
| `manifest.verify.summary.json` | `manifest verify-signature` ішінен CLI қорытындысы. |

Шығарылым жазбаларында және құжаттамада сілтеме жасалған барлық дайджесттер қайдан алынды
бұл файлдар. `ci/check_sorafs_cli_release.sh` жұмыс процесі бірдей қалпына келтіреді
артефактілер және оларды қабылданған нұсқалардан ажыратады.

## Арматураны қалпына келтіру

Арматура жинағын қалпына келтіру үшін репозиторий түбірінен төмендегі пәрмендерді орындаңыз.
Олар `sorafs-cli-fixture` жұмыс процесі пайдаланатын қадамдарды көрсетеді:

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Кез келген қадам әртүрлі хэштерді шығарса, арматураларды жаңарту алдында зерттеңіз.
CI жұмыс үрдістері регрессияларды анықтау үшін детерминирленген нәтижеге сүйенеді.

## Болашақ қамту

Қосымша chunker профильдері мен дәлелдеу пішімдері жол картасынан шыққан сайын,
олардың канондық құрылғылары осы каталогқа қосылады (мысалы,
`sorafs.sf2@1.0.0` (`fixtures/sorafs_manifest/ci_sample_sf2/` қараңыз) немесе PDP
ағынды дәлелдер). Әрбір жаңа профиль бір құрылымды ұстанады — пайдалы жүк, CAR,
жоспар, манифест, дәлелдер және қолтаңба артефактілері — осылайша төменгі ағынды автоматтандыру мүмкін
теңшелетін сценарийсіз diff шығарылымдары.