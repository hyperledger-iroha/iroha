---
lang: es
direction: ltr
source: docs/source/sorafs/portal_publish_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 84a23df97e771e110dc304ed09155e85d110deca8ba557518055c3669e6b9c27
source_last_modified: "2026-01-22T15:57:55.445047+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Portal de Docs → Plan de publicación en SoraFS (DOCS-7)
summary: Lista de verificación para empaquetar el portal de desarrolladores, OpenAPI y los paquetes SBOM como manifiestos SoraFS, fijarlos con alias y desplegar el gateway de docs + enlaces DNS.
---

# 1. Propósito y alcance

El ítem de roadmap **DOCS-7 – “Publicar vía gateway SoraFS”** exige que cada
artefacto público de documentación (build del portal, especificación OpenAPI,
SBOMs) pase por el registro de pins de SoraFS y se sirva detrás de `docs.sora`
con headers que transporten pruebas. Esta nota vincula las herramientas que ya
existen en el repositorio en un flujo reproducible para que Ops, Docs/DevRel y
Storage puedan ensayar el cutover antes del hito del T2 2026.

El plan asume:

- Están disponibles los helpers de empaquetado bajo `ci/` y
  `docs/portal/scripts/`.
- La gobernanza ha emitido el bundle de prueba de alias `docs:portal`
  (Norito binario).
- Las entradas SoraDNS para `docs.sora` ya apuntan al pool anycast del gateway.

Consulte `docs/portal/docs/devportal/deploy-guide.md` para el tutorial largo;
este archivo se centra en el runbook multi-equipo vinculado a DOCS-7.

# 2. Construir y empaquetar los payloads de la versión

1. **Lanzar el script de empaquetado**

   ```bash
   ./ci/package_docs_portal_sorafs.sh \
     --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
     --sign \
     --sigstore-provider=github-actions \
     --sigstore-audience=sorafs-devportal \
     --proof
   ```

   - Ejecuta `npm ci && npm run build` dentro de `docs/portal/`, replica
     `npm run sync-openapi` y `npm run test:*` para mantener frescos los
     snippets OpenAPI/Norito.
   - Llama a `syft` dos veces para emitir SBOMs CycloneDX para el directorio del
     portal compilado y `static/openapi/torii.json`.
   - Usa `sorafs_cli car pack` + `sorafs_cli manifest build` para el portal,
     OpenAPI, SBOM del portal y SBOM de OpenAPI (perfil de chunker por defecto
     `sorafs.sf1@1.0.0`, política de pin mínimo 5 réplicas, almacenamiento warm,
     retención de 14 épocas).
   - Cuando se suministra `--proof`, `sorafs_cli proof verify` añade un bundle
     `${label}.proof.json` para cada par CAR/manifiesto.
   - Cuando se suministra `--sign`, `sorafs_cli manifest sign` emite bundles
     Sigstore (`.manifest.bundle.json` / `.manifest.sig`), usando por defecto el
     token apuntado por `${SIGSTORE_ID_TOKEN}`.

2. **Inspeccionar el resumen de salida**

   - El script escribe un `package_summary.json` con una entrada por artefacto:

     ```json
     {
       "generated_at": "2026-02-19T13:00:12Z",
       "output_dir": "/abs/path/artifacts/devportal/sorafs/20260219T130012Z",
       "artifacts": [
         {
           "name": "portal",
           "car": "artifacts/devportal/sorafs/.../portal.car",
           "plan": "artifacts/devportal/sorafs/.../portal.plan.json",
           "car_summary": "artifacts/devportal/sorafs/.../portal.car.json",
           "manifest": "artifacts/devportal/sorafs/.../portal.manifest.to",
           "manifest_json": "artifacts/devportal/sorafs/.../portal.manifest.json",
           "proof": "artifacts/devportal/sorafs/.../portal.proof.json",
           "bundle": "artifacts/devportal/sorafs/.../portal.manifest.bundle.json",
           "signature": "artifacts/devportal/sorafs/.../portal.manifest.sig"
         }
       ]
     }
     ```

   - Archive el directorio completo (o haga un symlink vía
     `artifacts/devportal/sorafs/latest`) para que los revisores de gobernanza
     puedan rastrear los digests de payload hasta los logs de build y bundles de
     Sigstore.

# 3. Enviar y fijar manifiestos con alias

1. **Enviar manifiestos vía `sorafs_cli` (preferido)**

   ```bash
   OUT="artifacts/devportal/sorafs/20260219T130012Z"
   TORII_URL="https://torii.stg.sora.net/"
   AUTHORITY="ih58..."
   KEY_FILE="secrets/docs-admin.key"
   ALIAS_PROOF="secrets/docs.alias.proof"

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

   - Pasar `--chunk-plan` permite que la CLI derive el digest de chunk SHA3-256 y
     rechace discrepancias (sin malabares manuales de digest).
   - Repita para `openapi.manifest.to`, `portal.sbom.manifest.to` y
     `openapi.sbom.manifest.to`. Los SBOM normalmente **no** reciben alias, así
     que omita los flags de alias salvo que la gobernanza solicite un namespace
     dedicado (p. ej., `docs:portal-sbom`).
   - Ajuste `${SUBMITTED_EPOCH}` al epoch de consenso actual (obtenible con
     `curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` o su dashboard
     de ops) para que el registro rastree cuándo el manifiesto entró al ledger.

2. **Alternativa: `iroha app sorafs pin register`**

   - Cuando el host de despliegue tenga la CLI compilada (`cargo install --path .`),
     el mismo manifiesto puede fijarse vía:

     ```bash
     iroha app sorafs pin register \
     --manifest "${OUT}/portal.manifest.to" \
     --chunk-digest "$(jq -r '.chunk_digest_sha3_hex' "${OUT}/portal.manifest.submit.json")" \
     --submitted-epoch "${SUBMITTED_EPOCH}" \
     --alias-namespace docs \
     --alias-name portal \
     --alias-proof "${ALIAS_PROOF}"
     ```

   - Este wrapper golpea el mismo endpoint `/v1/sorafs/pin/register` pero
     depende de un digest precomputado.

3. **Verificar el estado del registro**

   - `iroha app sorafs pin list --alias docs:portal --format json | jq` debería
     mostrar el nuevo digest del manifiesto, la política de pin y las órdenes de
     replicación.
   - Dashboards: revise `dashboards/grafana/sorafs_pin_registry.json` para los
     conteos de alias y `torii_sorafs_replication_backlog_total` (debería
     permanecer cerca de cero).

# 4. Headers del gateway y `Sora-Proof`

1. **Generar el enlace de ruta + plantilla de headers**

   ```bash
   OUT="artifacts/devportal/sorafs/20260219T130012Z"
   iroha app sorafs gateway route-plan \
     --manifest-json "${OUT}/portal.manifest.json" \
     --hostname docs.sora \
     --alias docs:portal \
     --route-label docs-portal-20260219 \
     --proof-status ok \
     --headers-out "${OUT}/portal.gateway.headers.txt" \
     --out "${OUT}/portal.gateway.plan.json"
   ```

   - La plantilla contiene los headers `Sora-Name`, `Sora-CID`, `Sora-Proof` y
     `Sora-Proof-Status` (además de CSP/HSTS/Permissions-Policy). Adjunte el
     archivo `*.headers.txt` al bundle de evidencia de la versión para que un
     tercero pueda recrear el binding.

2. **Actualizar la configuración del gateway / CDN**

   - Entregue el bloque de headers generado a la automatización del Gateway
     según `docs/source/sorafs_gateway_tls_automation.md`. Para cutovers
     manuales, los headers pueden pegarse en la configuración de CDN o
     templarse vía Terraform.
   - Si ya existe un manifiesto de rollback, pase `--rollback-manifest-json`
     para que el comando escriba los headers primarios y de rollback.

3. **Sondear y auto-certificar antes de abrir el tráfico**

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --report-json artifacts/sorafs_gateway_probe/docs.json

   scripts/sorafs_gateway_self_cert.sh \
     --manifest "${OUT}/portal.manifest.json" \
     --headers "${OUT}/portal.gateway.headers.txt" \
     --output artifacts/sorafs_gateway_self_cert/docs
   ```

   - El probe aplica frescura de firmas GAR, política de pruebas de alias y
     huellas TLS (`dashboards/grafana/sorafs_gateway_observability.json` mostrará
     picos de `torii_sorafs_gateway_refusals_total` si algo deriva).
   - El harness de self-cert replica la ruta de fetch en producción con
     `sorafs_fetch`, guardando logs de replay de CAR bajo
     `artifacts/sorafs_gateway_self_cert/`.

# 5. Guardarraíles de DNS y monitoreo

1. **Evidencia del enlace DNS**

   - Ejecute `scripts/sns_zonefile_skeleton.py --manifest "${OUT}/portal.manifest.json"` para
     refrescar la estrofa TXT de `docs.sora` (incluye los hashes más recientes de
     `Sora-Proof` y `Sora-Proof-Status`). Adjunte la salida JSON a
     `artifacts/sorafs/portal.dns-cutover.json` junto con el plan existente.

2. **Telemetría**

   - Durante los primeros 30 minutos post-promoción observe:
     - `torii_sorafs_alias_cache_refresh_total` (salud de la política de alias)
     - `torii_sorafs_gateway_refusals_total{profile="docs"}` (política de gateway)
     - `torii_sorafs_fetch_duration_ms` / `torii_sorafs_fetch_failures_total`
       (telemetría de fetch end-to-end en el fetcher de staging)
   - Dashboards a fijar: `sorafs_gateway_observability.json`,
     `sorafs_fetch_observability.json` y el tablero genérico del registro de pins.

3. **Caos y alertas**

   - Dispare `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario dns-cutover`
     para que la evidencia de PagerDuty + Alertmanager quede archivada en
     `artifacts/sorafs_gateway_probe/<stamp>/`.
   - Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh` para ejercitar el
     paquete de alertas de fetch (`dashboards/alerts/sorafs_fetch_rules.yml`).

# 6. Evidencia y reportes

- Archive lo siguiente en el bundle de la versión (Git annex o SoraFS):
  - `artifacts/devportal/sorafs/<stamp>/` (CARs, manifiestos, SBOMs, pruebas,
    bundles Sigstore, resumen de empaquetado, resúmenes de envío de manifiestos).
  - `artifacts/sorafs_gateway_probe/<stamp>/` y
    `artifacts/sorafs_gateway_self_cert/<stamp>/`.
  - Esqueleto DNS (`portal.dns-cutover.json`) y plantillas de headers del gateway.
  - Capturas de dashboard que demuestren métricas saludables y reinicios de alertas.
- Actualice `status.md` (“Docs/DevRel” + “Storage”) con los digests de los
  manifiestos, timestamp de binding del alias y enlaces a los probes.
- Refleje este plan en el portal (`docs/portal/docs/sorafs/portal-publish-plan.md`)
  al actualizar el sitio Docusaurus para que los docs públicos compartan los
  mismos pasos.

Seguir estos pasos satisface el entregable DOCS-7: cada artefacto ahora viaja
por el pipeline canónico Norito/SoraFS, los manifiestos están aliased y
observables, y tanto DNS como telemetría capturan la evidencia firmada requerida
para el sign-off de producción.
