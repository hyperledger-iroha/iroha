<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 576e2aa47d5ebf5c11c313549e140bc832d2f666d77fae462b82d714b29e4254
source_last_modified: "2025-11-15T08:41:26.442940+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: portal-publish-plan
title: Plan de publicacion del portal de docs → SoraFS
sidebar_label: Plan de publicacion del portal
description: Checklist paso a paso para publicar el portal de docs, OpenAPI y bundles SBOM via SoraFS.
---

:::note Fuente canónica
Refleja `docs/source/sorafs/portal_publish_plan.md`. Actualiza ambas copias cuando el flujo cambie.
:::

El item de roadmap DOCS-7 requiere que cada artefacto de docs (build del portal, spec OpenAPI,
SBOMs) fluya por el pipeline de manifests de SoraFS y se sirva via `docs.sora` con headers
`Sora-Proof`. Esta checklist une los helpers existentes para que Docs/DevRel, Storage y Ops
puedan ejecutar el release sin buscar en multiples runbooks.

## 1. Build y empaquetado de payloads

Ejecuta el helper de empaquetado (hay opciones de skip para dry-runs):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` reutiliza `docs/portal/build` si CI ya lo produjo.
- Agrega `--skip-sbom` cuando `syft` no este disponible (p. ej., ensayo air-gapped).
- El script corre los tests del portal, emite pares CAR + manifest para `portal`,
  `openapi`, `portal-sbom` y `openapi-sbom`, verifica cada CAR cuando se usa
  `--proof`, y deja bundles de Sigstore cuando se usa `--sign`.
- Estructura de salida:

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

Conserva el folder completo (o un symlink via `artifacts/devportal/sorafs/latest`) para que
los revisores de gobernanza puedan rastrear los artefactos de build.

## 2. Pin de manifests y aliases

Usa `sorafs_cli manifest submit` para empujar manifests a Torii y enlazar aliases.
Configura `${SUBMITTED_EPOCH}` con la epoca mas reciente de consenso (desde
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` o tu dashboard).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="ih58..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

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

- Repite para `openapi.manifest.to` y los manifests de SBOM (omite flags de alias para bundles SBOM
  salvo que gobernanza asigne un namespace).
- Alternativa: `iroha app sorafs pin register` funciona con el digest del resumen de submit si el
  binario ya esta instalado.
- Verifica el estado del registry con
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Dashboards a seguir: `sorafs_pin_registry.json` (metricas `torii_sorafs_replication_*`).

## 3. Headers y proofs de gateway

Genera el bloque de headers HTTP + metadata de binding:

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

- La plantilla incluye headers `Sora-Name`, `Sora-CID`, `Sora-Proof`,
  `Sora-Proof-Status` mas la CSP/HSTS/Permissions-Policy por defecto.
- Usa `--rollback-manifest-json` para renderizar un set de headers de rollback.

Antes de exponer trafico, ejecuta:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- El probe exige frescura de firma GAR, politica de alias y huellas de TLS.
- El harness self-cert descarga el manifest con `sorafs_fetch` y guarda logs de replay del CAR;
  conserva los outputs para evidencia de auditoria.

## 4. Guardrails de DNS y telemetria

1. Refresca el esqueleto DNS para que gobernanza pueda demostrar el binding:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Monitorea durante el rollout:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` y la board del pin registry.

3. Haz smoke de las reglas de alerta (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) y
   captura logs/screenshots para el archivo de release.

## 5. Bundle de evidencias

Incluye lo siguiente en el ticket de release o paquete de gobernanza:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  bundles de Sigstore, submit summaries).
- Outputs de gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- Esqueleto DNS + plantillas de headers (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Capturas de dashboard + acknowledgements de alertas.
- Actualizacion de `status.md` referenciando el digest del manifest y el tiempo de
  binding del alias.

Seguir esta checklist entrega DOCS-7: los payloads del portal/OpenAPI/SBOM se
empaquetan de forma determinista, se fijan con aliases, se protegen con headers
`Sora-Proof` y se monitorean end-to-end mediante la pila de observabilidad existente.
