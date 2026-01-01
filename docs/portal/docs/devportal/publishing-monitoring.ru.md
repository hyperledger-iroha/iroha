---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: Публикация и мониторинг SoraFS
sidebar_label: Публикация и мониторинг
description: Фиксация end-to-end мониторинга релизов портала SoraFS, чтобы DOCS-3c имел детерминированные probes, телеметрию и bundles доказательств.
---

Элемент дорожной карты **DOCS-3c** требует большего, чем чеклист упаковки: после каждого
публикуемого релиза SoraFS мы должны постоянно доказывать, что портал разработчика,
прокси Try it и gateway bindings остаются здоровыми. Эта страница описывает мониторинговую
поверхность, которая сопровождает [deployment guide](./deploy-guide.md), чтобы CI и on-call инженеры
могли выполнять те же проверки, что и Ops для соблюдения SLO.

## Краткий обзор pipeline

1. **Build и подпись** - следуйте [deployment guide](./deploy-guide.md), чтобы запустить
   `npm run build`, `scripts/preview_wave_preflight.sh` и шаги отправки Sigstore + manifest.
   Скрипт preflight пишет `preflight-summary.json`, чтобы каждый preview нес metadata build/link/probe.
2. **Pin и верификация** - `sorafs_cli manifest submit`, `verify-sorafs-binding.mjs`
   и план DNS cutover дают детерминированные artefacts для governance.
3. **Архивация доказательств** - сохраняйте CAR summary, Sigstore bundle, alias proof,
   probe output и снимки дашборда `docs_portal.json` под `artifacts/sorafs/<tag>/`.

## Каналы мониторинга

### 1. Publishing monitors (`scripts/monitor-publishing.mjs`)

Новая команда `npm run monitor:publishing` объединяет probe портала, probe прокси Try it
и verifier bindings в один CI-дружественный check. Передайте JSON config
(хранится в CI secrets или `configs/docs_monitor.json`) и запустите:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Добавьте `--prom-out ../../artifacts/docs_monitor/monitor.prom` (и опционально
`--prom-job docs-preview`) для вывода Prometheus text-format метрик, пригодных
для Pushgateway или прямых scrapes в staging/production. Метрики зеркалируют JSON
summary, чтобы SLO dashboards и alert rules могли отслеживать здоровье портала,
Try it, bindings и DNS без разбора evidence bundle.

Пример config с обязательными knobs и несколькими bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/wonderland@wonderland/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/manifest",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiaff84aef0aaaf6a7c246c8ca1889e62d69c8d9b20d94933cb7b09902f3",
      "manifest": "8b8f3d2a4a7e92abdb17e5fafd4f9d67c6c7a8547ff985bb0d71f87209c1444d",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "openapi",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/openapi",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeidevopenapi",
      "manifest": "dad4b9fd48e35297c7fd71cd15b52c4ff0bb62dd8a1da4c5c2c1536ae2732b55",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "portal-sbom",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/portal-sbom",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiportalssbom",
      "manifest": "e2b2790f9f4c1ecbc8f1bdb9f8ba3fd65fd687e9e5e4de3c3d67c3d3192b79c8",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    }
  ],
  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

Монитор пишет JSON summary (S3/SoraFS friendly) и завершается с non-zero кодом,
если любой probe проваливается, поэтому он подходит для Cron jobs, Buildkite steps
или Alertmanager webhooks. Передача `--evidence-dir` сохраняет `summary.json`, `portal.json`,
`tryit.json` и `binding.json` рядом с manifest `checksums.sha256`, чтобы governance reviewers
могли diff-ить результаты без повторного запуска probes.

> **TLS guardrail:** `monitorPortal` отклоняет base URLs `http://`, если только
> не задан `allowInsecureHttp: true` в config. Держите probes production/staging на HTTPS;
> опция существует только для локальных previews.

Каждая запись binding проверяет `Sora-Name`, `Sora-Proof` и `Sora-Content-CID`
(headers и payload) плюс guard `expectHost`, чтобы DNS promotion
(`docs.sora` vs. `docs-preview.sora.link`) не отклонялся от alias в pin registry.
Checks падают сразу, если gateway перестает stapling заголовки
`Sora-Content-CID`/`Sora-Proof`, если в proof появляется неверный base64, или если
advertised manifest/CID расходится с pinned payloads
(сайт, OpenAPI и SBOM).

Опциональный блок `dns` подключает rollout SoraDNS из DOCS-7 к тому же monitor.
Каждая запись разрешает пару hostname/record-type (например CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) и подтверждает
совпадение с `expectedRecords` или `expectedIncludes`. Вторая запись в примере
фиксирует canonical hashed hostname, полученный `cargo xtask soradns-hosts --name docs-preview.sora.link`;
monitor теперь доказывает, что и friendly alias, и canonical hash (`igjssx53...gw.sora.id`)
резолвятся к pinned pretty host. Это делает evidence DNS promotion автоматическим:
monitor провалится, если любой host дрейфует, даже если HTTP bindings продолжают
stapling корректный manifest.

### 2. Guard для manifest версий OpenAPI

Требование DOCS-2b "signed OpenAPI manifest" теперь имеет автоматический guard:
`ci/check_openapi_spec.sh` вызывает `npm run check:openapi-versions`, который запускает
`scripts/verify-openapi-versions.mjs` для проверки
`docs/portal/static/openapi/versions.json` на соответствие реальным Torii specs и manifests.
Guard проверяет:

- Каждая версия в `versions.json` имеет соответствующую директорию под
  `static/openapi/versions/`.
- Поля `bytes` и `sha256` совпадают с on-disk файлом spec.
- Alias `latest` отражает `current` entry (digest/size/signature metadata),
  чтобы дефолтный download не дрейфовал.
- Подписанные entries ссылаются на manifest, чей `artifact.path` указывает обратно
  на тот же spec, и чьи значения подписи/публичного ключа в hex совпадают с manifest.

Запускайте guard локально при зеркалировании новой spec:

```bash
cd docs/portal
npm run check:openapi-versions
```

Сообщения об ошибках включают подсказку по устаревшему файлу (`npm run sync-openapi -- --latest`),
чтобы контрибьюторы портала знали, как обновить snapshots. Наличие guard в CI предотвращает
релизы портала, где signed manifest и опубликованный digest расходятся.

### 2. Dashboards и alerts

- **`dashboards/grafana/docs_portal.json`** - основной board для DOCS-3c. Панели
  отслеживают `torii_sorafs_gateway_refusals_total`, промахи SLA репликации, ошибки
  прокси Try it и latency probes (overlay `docs.preview.integrity`). Экспортируйте
  board после каждого релиза и прикладывайте к ops ticket.
- **Try it proxy alerts** - правило Alertmanager `TryItProxyErrors` срабатывает на
  устойчивые падения `probe_success{job="tryit-proxy"}` или всплески
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` гарантирует, что alias bindings продолжают
  рекламировать pinned manifest digest; эскалации ссылаются на CLI transcript
  `verify-sorafs-binding.mjs`, захваченный во время публикации.

### 3. Evidence trail

Каждый мониторинг-запуск должен добавлять:

- Evidence bundle `monitor-publishing` (`summary.json`, файлы по разделам и `checksums.sha256`).
- Grafana screenshots для board `docs_portal` за окно релиза.
- Transcripts изменения/rollback прокси Try it (логи `npm run manage:tryit-proxy`).
- Вывод проверки alias из `scripts/verify-sorafs-binding.mjs`.

Складывайте это под `artifacts/sorafs/<tag>/monitoring/` и линкуйте в release issue,
чтобы аудит-трейл сохранялся после истечения CI логов.

## Операционный чеклист

1. Пройдите deployment guide до Step 7.
2. Запустите `npm run monitor:publishing` с production config; архивируйте JSON вывод.
3. Сделайте скриншоты Grafana панелей (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) и приложите к release ticket.
4. Настройте регулярные мониторинги (рекомендация: каждые 15 минут) на production URLs
   с тем же config для выполнения SLO gate DOCS-3c.
5. Во время инцидентов перезапускайте monitor с `--json-out`, чтобы фиксировать
   доказательства до/после и прикладывать к postmortem.

Следование этому циклу закрывает DOCS-3c: build flow портала, publishing pipeline
и monitoring stack теперь живут в одном playbook с воспроизводимыми командами,
примером configs и telemetry hooks.
