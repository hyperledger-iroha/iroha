---
lang: es
direction: ltr
source: docs/source/sorafs/provider_admission_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cbbd4f055d7ac8fead2dd5d0f1df5842f333c102e1608f4224e2ab37b77d4e55
source_last_modified: "2025-11-22T07:08:04.607687+00:00"
translation_last_reviewed: "2026-01-30"
---

# Politica de admision e identidad de providers SoraFS (borrador SF-2b)

Esta nota captura los entregables accionables de **SF-2b**: definir y hacer
cumplir el workflow de admision, los requisitos de identidad y los payloads de
atestacion para providers de storage SoraFS. Expande el proceso de alto nivel
del RFC de Arquitectura SoraFS y desglosa el trabajo restante en tareas de
ingenieria trazables.

## Objetivos de la politica

- Asegurar que solo operadores verificados puedan publicar registros
  `ProviderAdvertV1` que la red acepte.
- Asociar cada clave de advertisement a un documento de identidad aprobado por
governance, endpoints atestados y un minimo de stake.
- Proveer tooling de verificacion determinista para que Torii, gateways y
  `sorafs-node` apliquen los mismos checks.
- Soportar renovacion y revocacion de emergencia sin romper determinismo ni
  ergonomia del tooling.

## Requisitos de identidad y stake

| Requisito | Descripcion | Entregable |
|-----------|-------------|------------|
| Proveniencia de clave de advert | Los providers deben registrar un par de llaves Ed25519 que firme cada advert. El bundle de admision guarda la llave publica junto a una firma de governance. | Extender el schema `ProviderAdmissionProposalV1` con `advert_key` (32 bytes) y referenciarlo desde el registry (`sorafs_manifest::provider_admission`). |
| Puntero de stake | La admision requiere un `StakePointer` no-cero apuntando a un pool de staking activo. | Agregar validacion en `sorafs_manifest::provider_advert::StakePointer::validate()` y exponer errores en CLI/tests. |
| Tags de jurisdiccion | Los providers declaran jurisdiccion + contacto legal. | Extender el schema de propuesta con `jurisdiction_code` (ISO 3166-1 alpha-2) y `contact_uri` opcional. |
| Atestacion de endpoints | Cada endpoint anunciado debe estar respaldado por un reporte de certificado mTLS o QUIC. | Definir payload Norito `EndpointAttestationV1` y almacenarlo por endpoint dentro del bundle de admision. |

## Workflow de admision

1. **Creacion de propuesta**
   - CLI: agregar `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     para producir `ProviderAdmissionProposalV1` + bundle de atestacion.
   - Validacion: asegurar campos requeridos, stake > 0, handle de chunker canonico en `profile_id`.
2. **Endoso de governance**
   - El council firma `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando tooling de sobres existente (`sorafs_manifest::governance` module).
   - El sobre se persiste en `governance/providers/<provider_id>/admission.json`.
3. **Ingestion del registry**
   - Implementar un verificador compartido (`sorafs_manifest::provider_admission::validate_envelope`) que Torii/gateways/CLI reusan.
   - Actualizar el path de admision Torii para rechazar adverts cuyo digest o expiry difiera del sobre.
4. **Renovacion y revocacion**
   - Agregar `ProviderAdmissionRenewalV1` con updates opcionales de endpoints/stake.
   - Exponer un path CLI `--revoke` que registre la razon de revocacion y emita un evento de governance.

## Tareas de implementacion

| Area | Tarea | Owner(s) | Status |
|------|------|----------|--------|
| Schema | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) bajo `crates/sorafs_manifest/src/provider_admission.rs`. Implementado en `sorafs_manifest::provider_admission` con helpers de validacion.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ Completed |
| CLI tooling | Extender `sorafs_manifest_stub` con subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

El flujo de CLI ahora acepta bundles de certificados intermedios (`--endpoint-attestation-intermediate`), emite bytes canonicos de propuesta/sobre y valida firmas del council durante `sign`/`verify`. Operadores pueden proveer advert bodies directos o reusar adverts firmados, y los archivos de firmas pueden suministrarse combinando `--council-signature-public-key` con `--council-signature-file` para facilitar automatizacion.

### Referencia CLI

Ejecutar cada comando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …`.

- `proposal`
  - Flags requeridas: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y al menos un `--endpoint=<kind:host>`.
  - La atestacion por endpoint espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificado via
    `--endpoint-attestation-leaf=<path>` (mas `--endpoint-attestation-intermediate=<path>`
    opcional por cada elemento de cadena) y cualquier ALPN negociado
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC pueden suministrar
    reportes de transporte con `--endpoint-attestation-report[-hex]=…`.
  - Output: bytes Norito canonicos de propuesta (`--proposal-out`) y un resumen JSON
    (default stdout o `--json-out`).
- `sign`
  - Inputs: una propuesta (`--proposal`), un advert firmado (`--advert`), advert body
    opcional (`--advert-body`), retention epoch y al menos una firma del council. Las
    firmas pueden proveerse inline (`--council-signature=<signer_hex:signature_hex>`) o
    via archivos combinando `--council-signature-public-key` con
    `--council-signature-file=<path>`.
  - Produce un sobre validado (`--envelope-out`) y un reporte JSON que indica
    bindings de digest, conteo de firmantes y rutas de input.
- `verify`
  - Valida un sobre existente (`--envelope`), opcionalmente chequeando la propuesta,
    advert o advert body. El reporte JSON resalta digests, estado de verificacion de
    firmas y que artefactos opcionales coincidieron.
- `renewal`
  - Enlaza un sobre aprobado nuevo al digest previamente ratificado. Requiere
    `--previous-envelope=<path>` y el sucesor `--envelope=<path>` (ambos payloads Norito).
    El CLI verifica que aliases/perfiles/capacidades y advert keys permanezcan
    sin cambios mientras permite updates de stake, endpoints y metadata. Emite
    bytes canonicos `ProviderAdmissionRenewalV1` (`--renewal-out`) mas un resumen JSON.
- `revoke`
  - Emite un bundle de emergencia `ProviderAdmissionRevocationV1` para un provider
    cuyo sobre debe retirarse. Requiere `--envelope=<path>`, `--reason=<text>`,
    al menos una `--council-signature`, y opcional `--revoked-at`/`--notes`. El CLI
    firma y valida el digest de revocacion, escribe el payload Norito via
    `--revocation-out` e imprime un reporte JSON con digest y conteo de firmas.
| Verificacion | Implementar verificador compartido usado por Torii, gateways y `sorafs-node`. Proveer unit + CLI integration tests.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ Completed |
| Integracion Torii | Hilar el verificador en la ingestion de adverts Torii, rechazar adverts fuera de politica, emitir telemetria. | Networking TL | ✅ Completed | Torii ahora carga sobres de governance (`torii.sorafs.admission_envelopes_dir`), verifica digest/firma durante ingestion y expone telemetria de admision.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renewal | Agregar schema de renovacion/revocacion + helpers CLI, publicar guia de lifecycle en docs (ver runbook abajo y comandos CLI en `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ Completed |
| Telemetria | Definir dashboards y alertas de `provider_admission` (renewal faltante, expiracion de sobres). | Observability | 🟠 In progress | Counter `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertas pendientes.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Runbook de renovacion y revocacion

#### Renovacion programada (updates de stake/topologia)
1. Construir el par propuesta/advert sucesor con `provider-admission proposal` y `provider-admission sign`, incrementando `--retention-epoch` y actualizando stake/endpoints segun se requiera.
2. Ejecutar
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   El comando valida campos de capacidad/perfil sin cambios via
   `AdmissionRecord::apply_renewal`, emite `ProviderAdmissionRenewalV1` e imprime
   digests para el log de governance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Reemplazar el sobre previo en `torii.sorafs.admission_envelopes_dir`, commitear
   la renovacion Norito/JSON en el repo de governance y anexar el hash de renovacion
   + retention epoch en `docs/source/sorafs/migration_ledger.md`.
4. Notificar a operadores que el nuevo sobre esta live y monitorear
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar
   ingestion.
5. Regenerar y commitear los fixtures canonicos via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) valida que los outputs Norito se mantienen estables.

#### Revocacion de emergencia
1. Identificar el sobre comprometido y emitir la revocacion:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   El CLI firma `ProviderAdmissionRevocationV1`, verifica el set de firmas via
   `verify_revocation_signatures` y reporta el digest de revocacion.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Remover el sobre de `torii.sorafs.admission_envelopes_dir`, distribuir la
   revocacion Norito/JSON a caches de admision y registrar el hash de razon en
   las minutas de governance.
3. Vigilar `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`
   para confirmar que caches descartan el advert revocado; mantener los artefactos
   de revocacion en retrospectives de incidentes.

## Testing y telemetria

- Agregar fixtures golden para propuestas de admision y sobres bajo
  `fixtures/sorafs_manifest/provider_admission/`.
- Extender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propuestas y verificar sobres.
- Fixtures generados incluyen `metadata.json` con digests canonicos; tests downstream
  verifican `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Proveer integration tests:
  - Torii rechaza adverts con sobres de admision ausentes o expirados.
  - CLI round-trips de propuesta -> sobre -> verificacion.
  - Renovacion de governance rota atestacion de endpoints sin cambiar provider ID.
- Requisitos de telemetria:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` en Torii. ✅ `torii_sorafs_admission_total{result,reason}` ahora expone outcomes aceptados/rechazados.
  - Agregar warnings de expiry a dashboards de observabilidad (renovacion due dentro de 7 dias).

## Proximos pasos

1. ✅ Se finalizaron los cambios de schema Norito y se aterrizaron helpers de validacion en
   `sorafs_manifest::provider_admission`. Sin feature flags.
2. ✅ Workflows CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) estan documentados y
   ejercitados via integration tests; mantener scripts de governance en sync con el runbook.
3. ✅ Torii admission/discovery ingieren los sobres y exponen contadores de telemetria para
   aceptacion/rechazo.
4. Enfocar observabilidad: terminar dashboards/alertas de admision para que renovaciones
   dentro de siete dias levanten warnings (`torii_sorafs_admission_total`, expiry gauges).
