---
lang: ba
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

DOCS‐I18NT0000000004X юл картаһы һәр йәмәғәт алдан ҡарауҙы талап итә, шул уҡ атлау өсөн .
тикшерелгән сумма тикшерелгән өйөм, тип рецензенттар урындағы күнекмәләр. Был runbook ҡулланыу .
Һуңынан рецензент onboarding (һәм саҡырыу раҫлау билет) тулы ҡуйырға
бета алдан ҡарау хост онлайн.

## Алдан шарттар

- Рецензент onboarding тулҡыны раҫланған һәм алдан ҡарау трекеры теркәлде.
- Һуңғы порталь төҙөү `docs/portal/build/` һәм чемпион
  тикшерелгән (`build/checksums.sha256`).
- I18NT00000000000Х алдан ҡарау ышаныс ҡағыҙҙары (I18NT00000000003X URL, власть, шәхси асҡыс, тапшырылған
  эпоха) йәки тирә-яҡ мөхит үҙгәртеүселәрендә һаҡлана йәки JSON конфигурациялау, мәҫәлән,
  [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json).
- DNS үҙгәртеү билеты теләкле хост-исем менән асылды (`docs-preview.sora.link`,
  `docs.iroha.tech`, һ.б.) плюс шылтыратыуҙа контакттар.

## 1-се аҙым – төҙөү һәм раҫлау өйөм

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Тикшеренеүҙең сценарийы дауам итеүҙән баш тарта, ҡасан тикшерелгән сумма манифест юҡ йәки
үҙгәртелә, һәр алдан ҡарау артефакт аудит һаҡлау.

## 2-се аҙым – I18NT000000001X артефакттары пакеты

Статик сайтты детерминистик CAR/манифест парына әйләндерегеҙ. `ARTIFACT_DIR`
ғәҙәттәгесә I18NI000000019X.

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

Беркетелгән генерацияланған I18NI000000020X, I18NI000000021X, дескриптор, һәм чемпионат
алдан ҡарау тулҡын билетына күренә.

## 3-сө аҙым – алдан ҡарау псевдонимын баҫтырып сығарыу

Ҡабаттан йүгерә булавка ярҙамсыһы **һыҙ ** I18NI000000022X бер тапҡыр һеҙ әҙер фашларға
хужа. Йәки йәки JSON конфигурация йәки асыҡ CLI флагтары менән тәьмин итеү:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Команда I18NI000000023X, 1990 й.
I18NI000000024X, һәм I18NI000000025X, улар
саҡырыу дәлилдәре менән ебәрергә тейеш.

## 4-се аҙым – DNS өҙөү планын генерациялау

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
``` X

Һөҙөмтәлә JSON менән бүлешергә Ops шулай DNS коммутатор һылтанмалар теүәл .
асыҡ һеңдерелгән. Ҡасан ҡабаттан ҡулланғанда, элекке дескриптор булараҡ, кире ҡайтарыу сығанағы,
ҡушымта `--previous-dns-plan path/to/previous.json`.

## 5-се аҙым – зондлау таратыусы хост .

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Зонд раҫлай хеҙмәтләндерелгән релиз тег, CSP башлыҡтары, һәм ҡултамға метамағлүмәттәре.
Ҡабатлау командаһы ике төбәктән (йәки беркетергә ҡойроҡ сығарыу), шулай итеп, аудиторҙар күрә ала
тип, сит кэш йылы.

## Дәлилдәр өйөмө

Алдан ҡарау тулҡын билетында түбәндәге артефакттарҙы индерегеҙ һәм уларҙы 1990 йылда мөрәжәғәт итегеҙ.
саҡырыу электрон почтаһы:

| Артефакт | Маҡсат |
|---------|----------|
| `build/checksums.sha256` | Ҡалып иҫбатлай өйөм тура килә CI төҙөү. |
| `artifacts/sorafs/portal.tar.gz` + I18NI000000029X X | Canonical SoraFS файҙалы йөк + манифест. |
| I18NI000000030X, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Күрһәтте, асыҡ тапшырыу + псевдоним бәйләү уңышлы булды. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS метамағлүмәттәр (билет, тәҙрә, контакттар), маршруттарҙы пропагандалау (`Sora-Route-Binding`) резюме, I18NI000000035X күрһәткесе (план JSON + баш ҡалыптары), кэш таҙартыу мәғлүмәте, һәм Ops өсөн кире инструкциялар. |
| `artifacts/sorafs/preview-descriptor.json` | Ҡул ҡуйылған дескриптор бәйләү архив + тикшерелгән суммаһы бергә. |
| I18NI000000037X сығыш | Тере алып барыусы раҫлай реклама көтөлгән релиз тег. |

Бер тапҡыр алып барыусы йәшәй, эйәреп [предвидение саҡырыу плейбук](./public-preview-invite.md)
һылтанманы таратыу, журнал саҡыра, һәм телеметрия күҙәтеү.