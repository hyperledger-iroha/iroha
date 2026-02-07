---
lang: ba
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

# I18NT000000001X CI өлгөһөн фикстуралар

Был каталог пакеттар детерминистик артефакттар генерацияланған өлгөһө .
файҙалы йөк I18NI000000005X буйынса. Блетл күрһәтә
ос-ос SoraFS упаковка һәм ҡул ҡуйыу торбаһы, тип CI эш ағымы күнекмәләр.

## Артефакт инвентаризацияһы

| Файл | Тасуирлама |
|------|-------------|
| `payload.txt` | Сығанаҡ файҙалы йөк ҡулланылған ҡоролма скрипттары (ябай текст өлгөһө). |
| I18NI0000007X | CAR архивы `sorafs_cli car pack` тарафынан сығарылған. |
| `car_summary.json` | Йәмғеһе генерацияланған I18NI000000010X эләктереп, өлөшләтә дигесттар һәм метамағлүмәттәр. |
| I18NI0000011X | Фетч-план JSON һүрәтләү өлөшө диапазондары һәм провайдер өмөттәре. |
| I18NI0000012X | I18NNT000000X манифесы `sorafs_cli manifest build` етештергән. |
| I18NI0000014X | Кеше уҡый торған манифест рендеринг өсөн отладка. |
| `proof.json` | PoR резюмеһы сығарылған I18NI000000016X. |
| `manifest.bundle.json` | 18NI000000018X тарафынан генерацияланған асҡысһыҙ ҡултамға өйөмө. |
| `manifest.sig` | Айырым Ed25519 ҡултамға тура килә манифест. |
| `manifest.sign.summary.json` | CLI резюмеһы ҡул ҡуйыу ваҡытында сығарылған (хэштар, өйөм метамағлүмәттәре). |
| `manifest.verify.summary.json` | CLI резюмеһы I18NI000000022X. |

Бөтә дайекстарҙа ла 2012 йылдан алып документтар һәм документацияла һылтанма яһала.
был файлдар. `ci/check_sorafs_cli_release.sh` эш ағымы шул уҡ регенерациялана
артефакттар һәм уларҙы йөкмәтелгән версияларға ҡаршы айыра.

## Фикстура регенерацияһы

Аҫтағы командаларҙы һаҡлау тамырынан эшләтеп, ҡоролма йыйылмаһын тергеҙеү өсөн.
Улар I18NI000000024X эш ағымы ҡулланған аҙымдарҙы көҙгөләй:

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

Әгәр ниндәй ҙә булһа аҙым төрлө хеш етештерә, тикшерергә алдынан яңыртыу ҡоролмалары.
CI эш ағымы детерминистик сығышҡа таяна, регрессияларҙы асыҡлау өсөн.

## Киләсәктә ҡаплау

Өҫтәмә chunker профилдәре һәм иҫбатлау форматтары булараҡ, юл картаһын тамамлай,
улар канон ҡоролмалары был каталог аҫтында өҫтәләсәк (мәҫәлән,
I18NI000000025X (ҡара: `fixtures/sorafs_manifest/ci_sample_sf2/` X) йәки ПДП
потоковый дәлилдәр). Һәр яңы профиль бер үк структура буйынса буласаҡ — түләү, CAR,
планы, асыҡ, дәлилдәр, һәм ҡултамға артефакттары — шулай автоматлаштырыу нижестоящий ала
дифф ҡулланыусылар сценарийҙарыһыҙ сығарыла.