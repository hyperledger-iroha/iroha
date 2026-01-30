---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admisión e identidad de proveedores SoraFS (Borrador SF-2b)

Esta nota captura los entregables accionables para **SF-2b**: definir y
aplicar el flujo de admisión, los requisitos de identidad y los payloads de
atestado para proveedores de almacenamiento SoraFS. Amplía el proceso de alto
nivel descrito en el RFC de Arquitectura de SoraFS y divide el trabajo restante
en tareas de ingeniería trazables.

## Objetivos de la política

- Garantizar que solo operadores verificados puedan publicar registros
  `ProviderAdvertV1` que la red aceptará.
- Vincular cada clave de anuncio a un documento de identidad aprobado por la
  gobernanza, endpoints atestados y una contribución mínima de stake.
- Proveer tooling de verificación determinista para que Torii, los gateways y
  `sorafs-node` apliquen los mismos controles.
- Soportar renovación y revocación de emergencia sin romper el determinismo ni
  la ergonomía del tooling.

## Requisitos de identidad y stake

| Requisito | Descripción | Entregable |
|-----------|-------------|------------|
| Procedencia de la clave de anuncio | Los proveedores deben registrar un par de claves Ed25519 que firme cada advert. El bundle de admisión almacena la clave pública junto con una firma de gobernanza. | Extender el esquema `ProviderAdmissionProposalV1` con `advert_key` (32 bytes) y referenciarlo desde el registro (`sorafs_manifest::provider_admission`). |
| Puntero de stake | La admisión requiere un `StakePointer` no cero apuntando a un pool de staking activo. | Añadir validación en `sorafs_manifest::provider_advert::StakePointer::validate()` y exponer errores en CLI/tests. |
| Etiquetas de jurisdicción | Los proveedores declaran jurisdicción + contacto legal. | Extender el esquema de propuesta con `jurisdiction_code` (ISO 3166-1 alpha-2) y `contact_uri` opcional. |
| Atestado de endpoint | Cada endpoint anunciado debe estar respaldado por un reporte de certificado mTLS o QUIC. | Definir payload Norito `EndpointAttestationV1` y almacenarlo por endpoint dentro del bundle de admisión. |

## Flujo de admisión

1. **Creación de propuesta**
   - CLI: añadir `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produciendo `ProviderAdmissionProposalV1` + bundle de atestado.
   - Validación: asegurar campos requeridos, stake > 0, handle canónico de chunker en `profile_id`.
2. **Endoso de gobernanza**
   - El consejo firma `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando el tooling
     de envelope existente (módulo `sorafs_manifest::governance`).
   - El envelope se persiste en `governance/providers/<provider_id>/admission.json`.
3. **Ingesta del registro**
   - Implementar un verificador compartido (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI reutilicen.
   - Actualizar la ruta de admisión en Torii para rechazar adverts cuyo digest o expiración difiera del envelope.
4. **Renovación y revocación**
   - Añadir `ProviderAdmissionRenewalV1` con actualizaciones opcionales de endpoint/stake.
   - Exponer una ruta CLI `--revoke` que registre el motivo de revocación y envíe un evento de gobernanza.

## Tareas de implementación

| Área | Tarea | Owner(s) | Estado |
|------|-------|----------|--------|
| Esquema | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) bajo `crates/sorafs_manifest/src/provider_admission.rs`. Implementado en `sorafs_manifest::provider_admission` con helpers de validación.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ Completado |
| Tooling CLI | Extender `sorafs_manifest_stub` con subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

El flujo de CLI ahora acepta bundles de certificados intermedios (`--endpoint-attestation-intermediate`), emite bytes canónicos de propuesta/envelope y valida firmas del consejo durante `sign`/`verify`. Los operadores pueden proporcionar cuerpos de advert directamente o reutilizar adverts firmados, y los archivos de firma pueden suministrarse combinando `--council-signature-public-key` con `--council-signature-file` para facilitar la automatización.

### Referencia de CLI

Ejecuta cada comando vía `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - Flags requeridos: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y al menos un `--endpoint=<kind:host>`.
  - El atestado por endpoint espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificado vía
    `--endpoint-attestation-leaf=<path>` (más `--endpoint-attestation-intermediate=<path>`
    opcional por cada elemento de la cadena) y cualquier ID ALPN negociado
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC pueden suministrar reportes de transporte con
    `--endpoint-attestation-report[-hex]=...`.
  - Salida: bytes canónicos de propuesta Norito (`--proposal-out`) y un resumen JSON
    (stdout por defecto o `--json-out`).
- `sign`
  - Entradas: una propuesta (`--proposal`), un advert firmado (`--advert`), cuerpo de advert opcional
    (`--advert-body`), retention epoch y al menos una firma del consejo. Las firmas pueden
    suministrarse inline (`--council-signature=<signer_hex:signature_hex>`) o vía archivos combinando
    `--council-signature-public-key` con `--council-signature-file=<path>`.
  - Produce un envelope validado (`--envelope-out`) y un reporte JSON indicando bindings de digest,
    conteo de firmantes y rutas de entrada.
- `verify`
  - Valida un envelope existente (`--envelope`), con comprobación opcional de la propuesta,
    advert o cuerpo de advert correspondiente. El reporte JSON destaca valores de digest, estado
    de verificación de firmas y qué artefactos opcionales coincidieron.
- `renewal`
  - Vincula un envelope recién aprobado al digest previamente ratificado. Requiere
    `--previous-envelope=<path>` y el sucesor `--envelope=<path>` (ambos payloads Norito).
    El CLI verifica que los aliases de perfil, capacidades y claves de advert permanezcan sin cambios,
    mientras permite actualizaciones de stake, endpoints y metadata. Emite los bytes canónicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) más un resumen JSON.
- `revoke`
  - Emite un bundle de emergencia `ProviderAdmissionRevocationV1` para un proveedor cuyo envelope debe
    retirarse. Requiere `--envelope=<path>`, `--reason=<text>`, al menos una
    `--council-signature`, y opcionalmente `--revoked-at`/`--notes`. El CLI firma y valida el
    digest de revocación, escribe el payload Norito vía `--revocation-out` e imprime un reporte JSON
    con el digest y el conteo de firmas.
| Verificación | Implementar verificador compartido usado por Torii, gateways y `sorafs-node`. Proveer pruebas unitarias + de integración de CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ Completado |
| Integración Torii | Cablear el verificador en la ingestión de adverts en Torii, rechazar adverts fuera de política y emitir telemetría. | Networking TL | ✅ Completado | Torii ahora carga envelopes de gobernanza (`torii.sorafs.admission_envelopes_dir`), verifica coincidencias de digest/firma durante la ingestión y expone telemetría de admisión.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renovación | Añadir esquema de renovación/revocación + helpers de CLI, publicar guía de ciclo de vida en docs (ver runbook abajo y comandos CLI en `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ Completado |
| Telemetría | Definir dashboards/alertas `provider_admission` (renovación faltante, expiración de envelope). | Observability | 🟠 En progreso | El contador `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertas pendientes.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renovación y revocación

#### Renovación programada (actualizaciones de stake/topología)
1. Construye el par propuesta/advert sucesor con `provider-admission proposal` y `provider-admission sign`, incrementando `--retention-epoch` y actualizando stake/endpoints según sea necesario.
2. Ejecuta
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   El comando valida campos de capacidad/perfil sin cambios vía
   `AdmissionRecord::apply_renewal`, emite `ProviderAdmissionRenewalV1` e imprime digests para el
   log de gobernanza.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Reemplaza el envelope anterior en `torii.sorafs.admission_envelopes_dir`, confirma el Norito/JSON de renovación en el repositorio de gobernanza y agrega el hash de renovación + retention epoch a `docs/source/sorafs/migration_ledger.md`.
4. Notifica a los operadores que el nuevo envelope está activo y monitorea `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar la ingestión.
5. Regenera y confirma los fixtures canónicos vía `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) valida que las salidas Norito permanezcan estables.

#### Revocación de emergencia
1. Identifica el envelope comprometido y emite una revocación:
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
   El CLI firma `ProviderAdmissionRevocationV1`, verifica el set de firmas vía
   `verify_revocation_signatures`, y reporta el digest de revocación.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Retira el envelope de `torii.sorafs.admission_envelopes_dir`, distribuye el Norito/JSON de revocación a cachés de admisión y registra el hash del motivo en las actas de gobernanza.
3. Observa `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que las cachés descartan el advert revocado; conserva los artefactos de revocación en retrospectivas de incidentes.

## Pruebas y telemetría

- Añadir fixtures golden para propuestas y envelopes de admisión bajo
  `fixtures/sorafs_manifest/provider_admission/`.
- Extender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propuestas y verificar envelopes.
- Los fixtures generados incluyen `metadata.json` con digests canónicos; pruebas downstream afirman
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Proveer pruebas de integración:
  - Torii rechaza adverts con envelopes de admisión faltantes o expirados.
  - El CLI hace round-trip de propuesta → envelope → verificación.
  - La renovación de gobernanza rota el atestado de endpoint sin cambiar el ID del proveedor.
- Requisitos de telemetría:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` en Torii. ✅ `torii_sorafs_admission_total{result,reason}` ahora expone resultados accepted/rejected.
  - Añadir alertas de expiración a dashboards de observabilidad (renovación debida dentro de 7 días).

## Próximos pasos

1. ✅ Finalizadas las modificaciones del esquema Norito y se incorporaron los helpers de validación en
   `sorafs_manifest::provider_admission`. No se requieren feature flags.
2. ✅ Los flujos CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) están documentados y ejercitados vía pruebas de integración; mantén los scripts de gobernanza sincronizados con el runbook.
3. ✅ Torii admission/discovery ingiere los envelopes y expone contadores de telemetría para aceptación/rechazo.
4. Foco en observabilidad: terminar dashboards/alertas de admisión para que las renovaciones debidas dentro de siete días disparen avisos (`torii_sorafs_admission_total`, expiry gauges).
