---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de inicio de Gateway y DNS de SoraFS

Esta copia del portal refleja el runbook canónico en
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Recoge las salvaguardas operativas del workstream de DNS y Gateway descentralizados
para que los leads de networking, ops y documentación puedan ensayar la pila de
automatización antes del kickoff 2025-03.

## Alcance y entregables

- Vincular los hitos de DNS (SF-4) y gateway (SF-5) ensayando la derivación
  determinista de hosts, liberaciones del directorio de resolutores, automatización TLS/GAR
  y captura de evidencias.
- Mantener los insumos del kickoff (agenda, invitación, tracker de asistencia, snapshot)
  de telemetría GAR) sincronizados con las últimas asignaciones de propietarios.
- Producir un paquete de artefactos auditables para revisores de gobernanza: notas de
  liberación del directorio de resolutores, registros de sondas del gateway, salida del arnés
  de conformidad y el resumen de Docs/DevRel.

## Roles y responsabilidades| Flujo de trabajo | Responsabilidades | Artefactos requeridos |
|------------|-------------------|-----------------------|
| Redes TL (pila DNS) | Mantener el plan determinista de hosts, ejecutar liberaciones del directorio RAD, publicar entradas de telemetría de resolutores. | `artifacts/soradns_directory/<ts>/`, diferencias de `docs/source/soradns/deterministic_hosts.md`, metadatos RAD. |
| Líder de automatización de operaciones (puerta de enlace) | Ejecutar taladros de automatización TLS/ECH/GAR, correr `sorafs-gateway-probe`, actualizar ganchos de PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de sonda, entradas en `ops/drill-log.md`. |
| Grupo de trabajo sobre herramientas y gremio de control de calidad | Ejecutar `ci/check_sorafs_gateway_conformance.sh`, curar accesorios, archivar paquetes de autocertificación Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Registrar minutas, actualizar el pre-read de diseño + apéndices y publicar el resumen de evidencias en este portal. | Archivos actualizados `docs/source/sorafs_gateway_dns_design_*.md` y notas de implementación. |

## Entradas y requisitos previos- Especificación de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) y
  el andamiaje de atestación de resolutores (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefactos del gateway: manual del operador, ayudantes de automatización TLS/ECH,
  guía de modo directo y flujo de autocertificación bajo `docs/source/sorafs_gateway_*`.
- Útiles: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, y ayudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secretos: clave de liberación GAR, credenciales ACME DNS/TLS, clave de enrutamiento de PagerDuty,
  token de autenticación de Torii para obtener resolutores.

## Lista de verificación previa al vuelo

1. Confirma asistentes y agenda actualizando
   `docs/source/sorafs_gateway_dns_design_attendance.md` y circulando la agenda
   vigente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepara raíces de artefactos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Refresca accesorios (manifiestos GAR, pruebas RAD, paquetes de conformidad del gateway) y
   asegura que el estado de `git submodule` coincide con la última etiqueta de ensayo.
4. Verifica secretos (clave de liberación Ed25519, archivo de cuenta ACME, token de PagerDuty)
   y que coinciden con checksums de vault.
5. Haz smoke-test de los objetivos de telemetría (endpoint de Pushgateway, tablero GAR Grafana)
   antes del taladro.

## Pasos de ensayo de automatizacion

### Mapa determinista de hosts y liberación del directorio RAD1. Ejecuta el helper de derivación determinista de hosts contra el set de manifests
   propuesto y confirma que no haya deriva respecto de
   `docs/source/soradns/deterministic_hosts.md`.
2. Genera un paquete de directorio de resolutores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registre el ID del directorio, el SHA-256 y las rutas de salida impresas dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` y en las minutas del saque inicial.

### Captura de telemetría DNS

- Haz tail de los logs de transparencia de resolutores durante ≥10 minutos usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporta métricas de Pushgateway y archiva los snapshots NDJSON junto al
  ID de ejecución del directorio.

### Taladros de automatización del gateway

1. Ejecuta la sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Ejecuta el arnés de conformidad (`ci/check_sorafs_gateway_conformance.sh`) y
   el helper de self-cert (`scripts/sorafs_gateway_self_cert.sh`) para refrescar
   el paquete de atestaciones Norito.
3. Captura eventos de PagerDuty/Webhook para demostrar que la automatización
   funciona de extremo a extremo.

### Empaquetado de evidencias

- Actualiza `ops/drill-log.md` con marcas de tiempo, participantes y hashes de sondas.
- Guarda los artefactos bajo los directorios de run ID y publica un resumen ejecutivo.
  en las minutas de Docs/DevRel.
- Enlaza el paquete de evidencias en el ticket de gobernanza antes de la revisión
  del saque inicial.

## Facilitación de sesión y entrega de evidencias- **Línea de tiempo del moderador:**
  - T-24 h — Program Management publica el recordatorio + snapshot de agenda/asistencia en `#nexus-steering`.
  - T-2 h — Networking TL refresca el snapshot de telemetria GAR y registra los deltas en `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica la preparación de sondas y escribe el run ID activo en `artifacts/sorafs_gateway_dns/current`.
  - Durante la llamada — El moderador comparte este runbook y asigna un escriba en vivo; Docs/DevRel capturan elementos de acción en línea.
- **Plantilla de minutas:** Copia el esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (también espejado en el paquete
  del portal) y comitea una instancia completada por sesión. Incluye lista de
  asistentes, decisiones, elementos de acción, hashes de evidencia y riesgos pendientes.
- **Carga de evidencias:** Comprime el directorio `runbook_bundle/` del ensayo,
  adjunta el PDF de minutas renderizado, registra hashes SHA-256 en las minutas +
  agenda y luego notifica al alias de reviewers de gobernanza cuando las cargas
  aterricen en `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Instantánea de evidencias (kickoff de marzo 2025)

Los últimos artefactos de ensayo/producción referenciados en el roadmap y las minutas
viven en el cubo `s3://sora-governance/sorafs/gateway_dns/`. Los hashes abajo
reflejando el manifiesto canónico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Ejecución en seco — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball del paquete: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF de minutas: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Taller en vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Carga pendiente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara el SHA-256 cuando el PDF renderizado llegue al paquete.)_

## Material relacionado

- [Guía de operaciones del gateway](./operations-playbook.md)
- [Plan de observabilidad de SoraFS](./observability-plan.md)
- [Rastreador de DNS descentralizado y puerta de enlace](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)