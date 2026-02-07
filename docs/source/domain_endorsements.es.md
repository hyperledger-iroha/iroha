---
lang: es
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Respaldos de dominio

Los respaldos de dominio permiten a los operadores controlar la creación y reutilización de dominios según una declaración firmada por el comité. La carga útil de respaldo es un objeto Norito registrado en la cadena para que los clientes puedan auditar quién atestiguó qué dominio y cuándo.

## Forma de carga útil

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: identificador de dominio canónico
- `committee_id`: etiqueta del comité legible por humanos
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: validez de límite de alturas de bloque
- `scope`: espacio de datos opcional más una ventana `[block_start, block_end]` opcional (inclusive) que **debe** cubrir la altura del bloque de aceptación
- `signatures`: firmas sobre `body_hash()` (endoso con `signatures = []`)
- `metadata`: metadatos Norito opcionales (identificadores de propuesta, enlaces de auditoría, etc.)

## Aplicación

- Se requieren respaldos cuando Nexus está habilitado e `nexus.endorsement.quorum > 0`, o cuando una política por dominio marca el dominio como requerido.
- La validación aplica el enlace hash de dominio/sentencia, la versión, la ventana de bloqueo, la membresía del espacio de datos, la caducidad/edad y el quórum del comité. Los firmantes deben tener claves de consenso activas con el rol `Endorsement`. Las repeticiones son rechazadas por `body_hash`.
- Los respaldos adjuntos al registro de dominio utilizan la clave de metadatos `endorsement`. La instrucción `SubmitDomainEndorsement` utiliza la misma ruta de validación, que registra respaldos para auditoría sin registrar un nuevo dominio.

## Comités y políticas

- Los comités pueden registrarse en cadena (`RegisterDomainCommittee`) o derivarse de los valores predeterminados de configuración (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- Las políticas por dominio se configuran a través de `SetDomainEndorsementPolicy` (identificador de comité, indicador `max_endorsement_age`, `required`). Cuando está ausente, se utilizan los valores predeterminados Nexus.

## ayudantes de CLI

- Crear/firmar un respaldo (genera Norito JSON en la salida estándar):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Presentar un respaldo:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Gestionar la gobernanza:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Los errores de validación devuelven cadenas de error estables (no coinciden el quórum, respaldo obsoleto/caducado, no coinciden el alcance, espacio de datos desconocido, comité faltante).