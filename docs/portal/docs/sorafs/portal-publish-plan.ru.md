---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 576e2aa47d5ebf5c11c313549e140bc832d2f666d77fae462b82d714b29e4254
source_last_modified: "2025-11-15T08:41:26.442940+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: portal-publish-plan
title: План публикации портала docs → SoraFS
sidebar_label: План публикации портала
description: Пошаговый чеклист для публикации docs портала, OpenAPI и SBOM через SoraFS.
---

:::note Канонический источник
Отражает `docs/source/sorafs/portal_publish_plan.md`. Обновляйте обе копии при изменении workflow.
:::

Пункт roadmap DOCS-7 требует, чтобы каждый артефакт docs (build портала, OpenAPI spec,
SBOM) проходил через pipeline manifests SoraFS и отдавался через `docs.sora` с заголовками
`Sora-Proof`. Этот чеклист связывает существующие helpers так, чтобы Docs/DevRel, Storage
и Ops могли запускать релиз без поиска по нескольким runbooks.

## 1. Build и упаковка payloads

Запустите helper для упаковки (есть опции skip для dry-run):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` использует `docs/portal/build`, если CI уже создал его.
- Добавьте `--skip-sbom`, когда `syft` недоступен (например, air-gapped репетиция).
- Скрипт запускает тесты портала, выпускает пары CAR + manifest для `portal`,
  `openapi`, `portal-sbom` и `openapi-sbom`, проверяет каждый CAR при `--proof`,
  и сохраняет Sigstore bundles при `--sign`.
- Структура вывода:

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

Сохраняйте всю папку (или создайте symlink через `artifacts/devportal/sorafs/latest`), чтобы
ревьюеры по governance могли трассировать build артефакты.

## 2. Pin manifests и aliases

Используйте `sorafs_cli manifest submit`, чтобы отправить manifests в Torii и привязать aliases.
Задайте `${SUBMITTED_EPOCH}` как последнюю эпоху консенсуса (через
`curl -s "${TORII_URL}/v2/status" | jq '.sumeragi.epoch'` или ваш dashboard).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v2/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- Повторите для `openapi.manifest.to` и SBOM manifests (уберите флаги alias для SBOM bundles,
  если governance не назначила namespace).
- Альтернатива: `iroha app sorafs pin register` работает с digest из submit summary, если
  бинарь уже установлен.
- Проверьте состояние registry командой
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Дашборды для мониторинга: `sorafs_pin_registry.json` (метрики `torii_sorafs_replication_*`).

## 3. Gateway headers и proofs

Сгенерируйте блок HTTP headers + binding metadata:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- Шаблон включает заголовки `Sora-Name`, `Sora-CID`, `Sora-Proof`,
  `Sora-Proof-Status` плюс стандартные CSP/HSTS/Permissions-Policy.
- Используйте `--rollback-manifest-json`, чтобы отрисовать набор rollback headers.

Перед открытием трафика выполните:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Probe требует свежести подписи GAR, политики alias и отпечатков TLS.
- Self-cert harness загружает manifest через `sorafs_fetch` и сохраняет CAR replay логи;
  храните outputs для аудиторских доказательств.

## 4. DNS и guardrails телеметрии

1. Обновите DNS skeleton, чтобы governance могла подтвердить binding:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Мониторьте во время rollout:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` и pin registry board.

3. Проведите smoke для alert rules (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) и
   сохраните логи/скриншоты для архива релиза.

## 5. Bundle доказательств

Включите следующее в тикет релиза или пакет governance:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  Sigstore bundles, submit summaries).
- Outputs gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleton + шаблоны headers (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Скриншоты дашбордов + подтверждения алертов.
- Обновление `status.md` с указанием manifest digest и времени alias binding.

Следование этому чеклисту обеспечивает DOCS-7: payloads portal/OpenAPI/SBOM
упакованы детерминированно, закреплены aliases, защищены headers `Sora-Proof`
и мониторятся end-to-end через существующий стек observability.
