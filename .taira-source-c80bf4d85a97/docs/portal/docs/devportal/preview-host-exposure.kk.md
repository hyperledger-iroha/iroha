---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82939c8aa73add3f3817490ab1a24bef5388c2fc5a00d19d76e4be6a3fa9c559
source_last_modified: "2025-12-29T18:16:35.111267+00:00"
translation_last_reviewed: 2026-02-07
id: preview-host-exposure
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
---

DOCS‑SORA жол картасы әрбір жалпыға ортақ алдын ала қарауды бірдей жолмен жүруді талап етеді
тексерушілер жергілікті орындайтын бақылау сомасы тексерілген топтама. Бұл runbook пайдаланыңыз
шолушы кіргеннен кейін (және шақыруды мақұлдау билетін) қою аяқталды
бета-алдын ала қарау хосты.

## Алғышарттар

- Шолушіні қосу толқыны мақұлданып, алдын ала қарау трекеріне кірді.
- `docs/portal/build/` және бақылау сомасы астында ең соңғы портал құрастыру
  тексерілген (`build/checksums.sha256`).
- SoraFS алдын ала қарау тіркелгі деректері (Torii URL, өкілеттік, жеке кілт, жіберілген
  epoch) ортаның айнымалы мәндерінде немесе JSON конфигурациясында сақталады, мысалы
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Қажетті хост атымен ашылған DNS өзгерту билеті (`docs-preview.sora.link`,
  `docs.iroha.tech`, т.б.) плюс қоңыраудағы контактілер.

## 1-қадам – Буманы құрастырыңыз және тексеріңіз

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Тексеру скрипті бақылау сомасы манифесті жоқ болғанда немесе жалғастырудан бас тартады
бұрмаланып, әрбір алдын ала қарау артефакті тексеріледі.

## 2-қадам – SoraFS артефактілерін орау

Статикалық торапты детерминирленген CAR/манифест жұбына түрлендіру. `ARTIFACT_DIR`
әдепкі бойынша `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

Жасалған `portal.car`, `portal.manifest.*`, дескриптор мен бақылау сомасын тіркеңіз
алдын ала қарау толқын билетіне манифест.

## 3-қадам – алдын ала қарау бүркеншік атын жариялау

Іске қосуға дайын болғаннан кейін пин көмекшісін **сыз** `--skip-submit` қайта іске қосыңыз.
хост. JSON конфигурациясын немесе айқын CLI жалауларын жеткізіңіз:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Пәрмен `portal.pin.report.json` деп жазады,
`portal.manifest.submit.summary.json` және `portal.submit.response.json`, олар
шақыру дәлелдер жинағымен бірге жеткізілуі керек.

## 4-қадам – DNS кесу жоспарын жасаңыз

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

Алынған JSON файлын Ops көмегімен бөлісіңіз, осылайша DNS қосқышы дәл сілтеме жасайды
манифест дайджест. Бұрынғы дескрипторды кері қайтару көзі ретінде қайта пайдаланған кезде,
`--previous-dns-plan path/to/previous.json` қосыңыз.

## 5-қадам – Орналастырылған хостты тексеру

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Тексеру қызмет көрсетілетін шығарылым тегін, CSP тақырыптарын және қолтаңба метадеректерін растайды.
Аудиторлар көре алатындай пәрменді екі аймақтан қайталаңыз (немесе бұйра шығуды тіркеңіз).
шеткі кэш жылы.

## Дәлелдер жинағы

Келесі артефактілерді алдын ала қарау толқыны билетіне қосыңыз және оларға сілтеме жасаңыз
шақыру электрондық поштасы:

| Артефакт | Мақсаты |
|----------|---------|
| `build/checksums.sha256` | Буманың CI құрылымына сәйкес келетінін дәлелдейді. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канондық SoraFS пайдалы жүктеме + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Манифестті жіберуді көрсетеді + бүркеншік атын байланыстыру сәтті аяқталды. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS метадеректері (билет, терезе, контактілер), маршрутты жылжыту (`Sora-Route-Binding`) қысқаша мазмұны, `route_plan` көрсеткіші (жоспар JSON + тақырып үлгілері), кэшті тазарту ақпараты және Ops үшін кері қайтару нұсқаулары. |
| `artifacts/sorafs/preview-descriptor.json` | Мұрағатты + бақылау сомасын біріктіретін қол қойылған дескриптор. |
| `probe` шығыс | Тікелей хост күтілетін шығарылым тегін жарнамалайтынын растайды. |

Хост белсенді болғаннан кейін, [алдын ала қарау шақыру ойнату кітабын] орындаңыз (./public-preview-invite.md)
сілтемені тарату, шақыруларды тіркеу және телеметрияны бақылау.