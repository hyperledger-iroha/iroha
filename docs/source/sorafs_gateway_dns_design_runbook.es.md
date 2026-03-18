---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_design_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1f8de6c5ff283479cd76dc60afbdc4bc142596f3e48c52472d2a943ec805b0d
source_last_modified: "2025-11-22T10:14:00.146215+00:00"
translation_last_reviewed: "2026-01-30"
---

# 1. Propósito y alcance

Este runbook convierte el requisito de “owner runbooks + ensayo de automatización”
de `roadmap.md` (entrada Decentralized DNS & Gateway bajo **Near-Term Execution**)
en pasos accionables. Conecta los entregables SF‑4 (DNS determinista) y SF‑5
(hardening de gateway) al describir cómo Networking, Ops, QA y Docs ensayan la
pila de automatización antes del kickoff del 2025‑03‑03.

- **Qué cubre:** derivación determinista de hosts, releases del directorio SoraDNS,
  probes de automatización TLS/GAR, ejecución del harness de conformidad, captura
  de telemetría y empaquetado de evidencia.
- **Qué no cubre:** respuesta a incidentes en producción (ver
  `docs/source/sorafs_gateway_operator_playbook.md`) ni cambios posteriores al
  kickoff en el roadmap (se rastrean en `roadmap.md` y `status.md`).

# 2. Matriz de owners y roles

| Workstream | Responsabilidades | Primario / Backup | Artefactos requeridos |
|------------|------------------|------------------|------------------------|
| Networking TL (stack DNS) | Mantener plan determinista de hosts, ejecutar releases del directorio RAD, dueño de inputs de telemetría del resolver. | networking@sora | `artifacts/soradns_directory/<timestamp>/`, diff de `docs/source/soradns/deterministic_hosts.md`, metadata de release RAD. |
| Ops Automation Lead (gateway) | Ejecutar drills de automatización TLS/ECH/GAR, operar `sorafs-gateway-probe`, gestionar hooks de PagerDuty. | ops@sorafs / sre@sora | `artifacts/sorafs_gateway_probe/<timestamp>/`, JSON de probe, `ops/drill-log.md` actualizado. |
| QA Guild & Tooling WG | Correr `ci/check_sorafs_gateway_conformance.sh`, curar fixtures y archivar bundles Norito de auto-cert. | qa.guild@sorafs / tooling@sora | `artifacts/sorafs_gateway_conformance/<timestamp>/`, `artifacts/sorafs_gateway_attest/<timestamp>/`. |
| Docs / DevRel | Capturar actas, actualizar archivos `docs/source/sorafs_gateway_dns_design_*`, publicar resumen de evidencia en el portal. | docs@sora / devrel@sora | Pre-read actualizado, changelog del runbook, apéndice de telemetría, registro de acciones de la agenda. |

# 3. Inputs y prerrequisitos

- Especificación de host determinista (`docs/source/soradns/deterministic_hosts.md`) y
  el scaffolding de atestación del resolver (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefactos de gateway: perfil, deployment handbook, automatización TLS, guía de
  modo directo y workflow de self-cert (ver docs `docs/source/sorafs_gateway_*`).
- Tooling: helpers `cargo xtask` (`soradns-directory-release`,
  `sorafs-gateway-probe`), `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, y helpers de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secretos y llaves: key de release GAR, credenciales DNS/TLS ACME, routing key
  de PagerDuty, token de auth Torii para fetches del resolver.

# 4. Checklist pre-flight

1. **Confirmar asistentes y agenda:** actualizar
   `docs/source/sorafs_gateway_dns_design_attendance.md` y compartir
   `docs/source/sorafs_gateway_dns_design_agenda.md`.
2. **Preparar directorios de artefactos:** crear
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/` para guardar outputs del ensayo.
3. **Refrescar fixtures:** obtener los últimos manifiestos GAR, pruebas RAD y
  fixtures de conformidad de gateway (`git submodule update --init --recursive` si aplica).
4. **Sanidad de secretos:** verificar la key Ed25519 de release, archivo de cuenta ACME,
  y token PagerDuty con checksums que coincidan con el vault.
5. **Targets de telemetría:** asegurar que el Pushgateway de staging y el board
  GAR en Grafana (`dashboards/grafana/sorafs_fetch_observability.json`) estén
  accesibles antes del drill.

# 5. Pasos del ensayo de automatización

Cada paso DEBE ejecutarse y archivarse antes del kickoff. Guardar outputs bajo
el directorio del run ID creado en el paso 2.

## 5.1 Mapa determinista de hosts y release del directorio RAD

1. Ejecutar el script de derivación determinista (receta Rust o JS) contra el
   set de manifiestos propuesto para asegurar que no hay drift frente a
   `docs/source/soradns/deterministic_hosts.md`.
2. Cuando ya existan sobres GAR, verificar que los patrones de host coinciden
   con el output derivado antes de los ensayos:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --verify-host-patterns artifacts/soradns_gar/docs.sora.json
```

3. Generar un bundle de directorio del resolver:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

4. Registrar el ID del directorio, SHA-256 y rutas de output en
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` más las notas de
   la reunión de kickoff.

## 5.2 Captura de telemetría DNS

1. Tail de logs de transparencia del resolver por al menos diez minutos:

```bash
./scripts/telemetry/run_soradns_transparency_tail.sh \
  --log /var/log/soradns/transparency.jsonl \
  --metrics artifacts/sorafs_gateway_dns_design_metrics_20250302.prom \
  --output artifacts/sorafs_gateway_dns_design_transparency_20250302.jsonl \
  --push-url https://pushgateway.stg.sora.net/metrics/job/soradns
```

2. Anexar un resumen (hashes, conteos de pruebas, ventana de frescura) a
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.

## 5.3 Probe de TLS / GAR de gateway

1. Ejecutar el drill de automatización TLS según
   `docs/source/sorafs_gateway_tls_automation.md`.
2. Ejecutar la probe con flags de drill para ejercitar payloads PagerDuty/webhook:

```bash
cargo xtask sorafs-gateway-probe \
  --target https://gateway.stg.sora \
  --report-json artifacts/sorafs_gateway_probe/20250302/report.json \
  --summary-json artifacts/sorafs_gateway_probe/20250302/summary.json \
  --drill-name dns-gateway-kickoff \
  --pagerduty-routing-key "${PD_KEY}" \
  --pagerduty-detail playbook=docs/source/sorafs_gateway_tls_automation.md \
  --ops-notes artifacts/sorafs_gateway_dns/20250302/ops-notes.md
```

3. Si el drill usa CI, invocar `ci/check_sorafs_gateway_probe.sh` para mantener
   dashboards/tests actuales. Copiar el resultado en `ops/drill-log` a las
   actas del kickoff.

## 5.4 Harness de conformidad y self-cert

1. Correr la suite de conformidad:

```bash
CI=1 ci/check_sorafs_gateway_conformance.sh \
  --out artifacts/sorafs_gateway_conformance/20250302
```

2. Producir un bundle de atestación Norito para el gateway de ensayo:

```bash
./scripts/sorafs_gateway_self_cert.sh \
  --config docs/examples/sorafs_gateway_self_cert.conf \
  --out artifacts/sorafs_gateway_attest/20250302
```

3. Adjuntar `sorafs_gateway_report.json` y sobres `.to` al bundle de evidencia;
   enlazarlos desde `docs/source/sorafs_gateway_dns_design_attendance.md` una vez
   añadidas las notas de revisión.

## 5.5 Ensamblaje del bundle de evidencia

Crear `artifacts/sorafs_gateway_dns/20250302/runbook_bundle/` con:

- JSON del directorio RAD, record, metadata y copias `.norito` del resolver.
- Archivos de transparencia `.jsonl` y `.prom` con manifiestos SHA-256.
- JSON de probe + screenshots, logs de CI, payloads PagerDuty.
- Output de conformidad (`report.json`, logs) y sobre de self-cert.
- Agenda firmada de la reunión, snapshot de asistentes y registro de acciones.

Comprimir el directorio (`tar -czf sorafs_gateway_dns_runbook_20250302.tgz ...`) y
subirlo junto a las actualizaciones del pre-read.

### Tracker de hashes (llenar durante el upload)

| Artefacto | SHA-256 | Ruta de upload |
|----------|---------|----------------|
| `sorafs_gateway_dns_runbook_20250302.tgz` | `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0` | `s3://sora-governance/sorafs/gateway_dns/20250302/` |
| `gateway_dns_minutes_20250302.pdf` | `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18` | `s3://sora-governance/sorafs/gateway_dns/20250302/` |
| Capturas adicionales / evidencia | n/a (no capturado para este ensayo) | — |

## 5.6 Verificación del bundle de alertas

- Cargar el pack de alertas DNS/gateway (`dashboards/alerts/soradns_gateway_rules.yml`) en el
  Alertmanager de staging y ejecutar `promtool check rules dashboards/alerts/soradns_gateway_rules.yml`
  como parte del ensayo. El pack monitorea edad/TTL de pruebas del resolver, drift de CID, lag de
  sync, fallas de refresh de caché de alias y renovación/expiración TLS del gateway para que los
  operadores tengan guardrails durante promociones.
- Ejecutar los tests de alertas respaldados por fixtures con
  `promtool test rules dashboards/alerts/tests/soradns_gateway_rules.test.yml` (o
  `scripts/check_prometheus_rules.sh dashboards/alerts/tests/soradns_gateway_rules.test.yml`) y
  guardar el stdout en el bundle de evidencia; esto prueba que los thresholds de
  stale-proof/TTL/drift/TLS están bien cableados antes del workshop.
- Capturar el output del drill de PromQL (o stdout de promtool) en el bundle de
  evidencia antes de pasar a la facilitación.

# 6. Facilitación de sesión y hand-off de evidencia

Esta sección mantiene el taller en curso una vez completados los dry-runs. Los
moderadores deben seguir la mini-línea de tiempo, registrar decisiones en la
plantilla de actas y empujar artefactos a governance inmediatamente después de
la llamada.

## 6.1 Timeline del moderador

| Offset | Owner | Acción |
|--------|-------|--------|
| T‑24 h | Program Management | Publicar recordatorio en `#nexus-steering`, adjuntar la última agenda/attendance y enlazar el bundle de artefactos. |
| T‑2 h | Networking TL | Re-ejecutar el script de telemetría GAR y anexar deltas en `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`. |
| T‑15 m | Ops Automation | Verificar health del host de probe y actualizar el run ID en `artifacts/sorafs_gateway_dns/current`. |
| Kickoff | Moderador | Compartir el enlace al runbook + plantilla de actas, llamar criterios de éxito y asignar un scribe en vivo. |
| +15 m | Docs/DevRel | Capturar action items + punteros de evidencia en el doc de actas, resaltando bloqueos que requieran escalamiento de governance. |
| EoS | Todos los owners | Confirmar uploads de artefactos, notar TODOs pendientes y crear tickets de seguimiento. |

## 6.2 Plantilla de actas

Copiar este esqueleto en `docs/source/sorafs_gateway_dns_design_minutes.md`
para cada sesión (un archivo por fecha):

```markdown
# Gateway & DNS Kickoff — YYYY-MM-DD

- **Moderador:** <name>
- **Scribe:** <name>
- **Asistentes:** <list>
- **Puntos de agenda:** <bullets>
- **Decisiones:** <numbered list>
- **Acciones:** <owner → due date>
- **Bundle de evidencia:** `artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/`
- **Riesgos/preguntas abiertas:** <bullets>
```

Una vez exportado a PDF para governance, guardar el archivo renderizado junto
al bundle y enlazarlo desde `docs/source/sorafs_gateway_dns_design_attendance.md`.

## 6.3 Checklist de upload de evidencia

1. Subir el tarball de § 5.5 más el PDF de actas al bucket de governance
   (`s3://sora-governance/sorafs/gateway_dns/20250302/`).
2. Registrar los hashes SHA-256 en las actas y en el issue tracker del steering.
3. Avisar al alias de reviewer de governance con el link de upload, ACKs de Networking
   y Storage leads, y el screenshot de PagerDuty.

# 7. Acciones post-run

1. Actualizar `docs/source/sorafs_gateway_dns_design_pre_read.md` y la agenda
   con riesgos/assumptions nuevos descubiertos durante el ensayo.
2. Añadir una fila a `ops/drill-log.md` resumiendo el drill, versiones de tooling
  y resultado de PagerDuty.
3. Registrar un bullet en `status.md` bajo “Latest Updates” referenciando los
   artefactos ensayados para que equipos downstream sepan que el material del
   kickoff está listo.
4. Si hubo fallos de conformidad, abrir issues de GitHub con label `sf-5a` y
   listarlos en `docs/source/sorafs_gateway_dns_design_attendance.md`.
