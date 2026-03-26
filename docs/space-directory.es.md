---
lang: es
direction: ltr
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/space-directory.md (Space Directory Operator Playbook) -->

# Playbook del Operador de Space Directory

Este playbook explica cómo redactar, publicar, auditar y rotar entradas del
**Space Directory** para dataspaces de Nexus. Complementa las notas de arquitectura en
`docs/source/nexus.md` y el plan de onboarding de CBDC
(`docs/source/cbdc_lane_playbook.md`) proporcionando procedimientos prácticos, fixtures y
plantillas de gobernanza.

> **Ámbito.** El Space Directory actúa como registro canónico de los manifests de
> dataspace, de las políticas de capacidades (UAID) de Universal Account ID y del trail de
> auditoría que exigen los reguladores. Aunque el contrato subyacente sigue en desarrollo
> activo (NX‑15), los fixtures y procesos de este documento están listos para integrarse
> en tooling y pruebas de integración.

## 1. Conceptos básicos

| Término | Descripción | Referencias |
|--------|-------------|------------|
| Dataspace | Contexto de ejecución/lane que ejecuta un conjunto de contratos aprobado por gobernanza. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (hash blake2b‑32) usado para anclar permisos cross‑dataspace. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` que describe reglas deterministas de allow/deny para un par UAID/dataspace (deny prevalece). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | Metadatos de gobernanza + DA publicados junto a los manifests, para que los operadores puedan reconstruir conjuntos de validadores, listas de composición permitida y ganchos de auditoría. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Eventos codificados en Norito emitidos cuando los manifests se activan, expiran o revocan. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Ciclo de vida del manifest

Space Directory aplica **gestión de ciclo de vida basada en epochs**. Cada cambio
produce un bundle de manifest firmado más un evento:

| Evento | Disparador | Acciones requeridas |
|--------|------------|---------------------|
| `ManifestActivated` | Nuevo manifest alcanza `activation_epoch`. | Difundir bundle, actualizar cachés, archivar aprobación de gobernanza. |
| `ManifestExpired` | `expiry_epoch` pasa sin renovación. | Avisar a operadores, limpiar handles de UAID, preparar manifest de reemplazo. |
| `ManifestRevoked` | Decisión de deny‑wins de emergencia antes del expiry. | Revocar UAID de inmediato, emitir informe de incidente, programar revisión de gobernanza. |

Los suscriptores deben usar `DataEventFilter::SpaceDirectory` para vigilar dataspaces o
UAIDs concretos. Filtro de ejemplo (Rust):

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. Flujo de trabajo del operador

| Fase | Responsable(s) | Pasos | Evidencia |
|------|----------------|-------|----------|
| Draft | Propietario del dataspace | Clonar fixture, editar permisos/gobernanza, ejecutar `cargo test -p iroha_data_model nexus::manifest`. | Diff de Git, log de tests. |
| Review | Governance WG | Validar JSON del manifest + bytes Norito, firmar registro de decisión. | Acta firmada, hash del manifest (BLAKE3 + Norito `.to`). |
| Publish | Lane ops | Enviar vía CLI (`iroha app space-directory manifest publish`) usando un payload Norito `.to` o JSON raw **o** hacer POST a `/v1/space-directory/manifests` con el JSON del manifest + reason opcional, verificar la respuesta de Torii y capturar `SpaceDirectoryEvent`. | Recibo CLI/Torii, log de eventos. |
| Expire | Lane ops / Gobernanza | Ejecutar `iroha app space-directory manifest expire` (UAID, dataspace, epoch) cuando un manifest llegue al final de su vida útil, verificar `SpaceDirectoryEvent::ManifestExpired`, archivar evidencias de limpieza de bindings. | Salida de CLI, log de eventos. |
| Revoke | Gobernanza + Lane ops | Ejecutar `iroha app space-directory manifest revoke` (UAID, dataspace, epoch, reason) **o** hacer POST a `/v1/space-directory/manifests/revoke` con el mismo payload hacia Torii, verificar `SpaceDirectoryEvent::ManifestRevoked`, actualizar el paquete de evidencias. | Recibo CLI/Torii, log de eventos, nota en el ticket. |
| Monitor | SRE/Compliance | Seguir la telemetría + logs de auditoría, configurar alertas para revocaciones/expiración. | Captura de Grafana, logs archivados. |
| Rotate/Revoke | Lane ops + Gobernanza | Preparar manifest de reemplazo (nuevo epoch), hacer tabletop, abrir incidente (en caso de revoke). | Ticket de rotación, post‑mortem del incidente. |

Todos los artefactos de un rollout se guardan bajo
`artifacts/nexus/<dataspace>/<timestamp>/` con un manifest de checksums para cubrir
peticiones de evidencia de reguladores.

## 4. Plantilla de manifest y fixtures

Usa los fixtures curados como referencia canónica de esquema. El ejemplo de CBDC
mayorista (`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) incluye
entradas de allow y deny:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

La regla deny‑wins típica de onboarding CBDC podría verse así:

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

Cuando se publica un manifest, la combinación de `uaid` + `dataspace` se convierte en la
clave principal del registro de permisos. Los manifests posteriores se enlazan por
`activation_epoch` y el registro garantiza que la última versión activa respete siempre la
semántica “deny gana”.

## 5. API de Torii

### Publicar manifest

Los operadores pueden publicar manifests directamente vía Torii, sin depender del CLI.

```
POST /v1/space-directory/manifests
```

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `authority` | `AccountId` | Cuenta que firma la transacción de publicación. |
| `private_key` | `ExposedPrivateKey` | Clave privada en base64 que Torii usa para firmar en nombre de `authority`. |
| `manifest` | `SpaceDirectoryManifest` | Manifest completo (JSON) que se codificará en Norito internamente. |
| `reason` | `Option<String>` | Mensaje opcional para el trail de Auditoría que se almacena junto a los datos de ciclo de vida. |

Ejemplo de cuerpo JSON:

```jsonc
{
  "authority": "<katakana-i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

Torii responde con `202 Accepted` tan pronto como la transacción entra en cola. Cuando el
bloque se ejecuta, se emite `SpaceDirectoryEvent::ManifestActivated` (sujeto a
`activation_epoch`), los bindings se reconstruyen automáticamente y el endpoint de
inventario de manifests empieza a reflejar el nuevo payload. Los controles de acceso
coinciden con las demás APIs de escritura de Space Directory (CIDR/token de API/política
de tarifas).

### API de revocación de manifest

Las revocaciones de emergencia ya no requieren invocar el CLI: los operadores pueden hacer
POST directamente a Torii para encolar la instrucción canónica
`RevokeSpaceDirectoryManifest`. La cuenta emisora debe tener
`CanPublishSpaceDirectoryManifest { dataspace }`, alineada con el flujo del CLI.

```
POST /v1/space-directory/manifests/revoke
```

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `authority` | `AccountId` | Cuenta que firma la transacción de revocación. |
| `private_key` | `ExposedPrivateKey` | Clave privada en base64 que Torii usa para firmar en nombre de `authority`. |
| `uaid` | `String` | Literal UAID (`uaid:<hex>` o digest hex de 64 caracteres, LSB=1). |
| `dataspace` | `u64` | Identificador del dataspace que hospeda el manifest. |
| `revoked_epoch` | `u64` | Epoch (incluyente) en el que debe tomar efecto la revocación. |
| `reason` | `Option<String>` | Mensaje opcional de auditoría almacenado junto a los datos de ciclo de vida. |

Ejemplo de cuerpo JSON:

```jsonc
{
  "authority": "<katakana-i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii devuelve `202 Accepted` cuando la transacción entra en la cola. Al ejecutarse el
bloque, se emite `SpaceDirectoryEvent::ManifestRevoked`, se reconstruye automáticamente
`uaid_dataspaces` y tanto `/portfolio` como el inventario de manifests pasan a reflejar el
estado revocado de inmediato. Los filtros por CIDR y por política de tarifas son los
mismos que en los endpoints de lectura.

## 6. Plantilla de perfil de dataspace

Los perfiles capturan todo lo que un nuevo validador necesita antes de conectarse. El
fixture `profile/cbdc_lane_profile.json` documenta:

- Emisor/quórum de gobernanza (`i105...` + ID del ticket de evidencia).
- Conjunto de validadores + quórum y namespaces protegidos (`cbdc`, `gov`).
- Perfil de DA (clase A, lista de attesters, cadencia de rotación).
- ID de grupo de composabilidad y whitelist que vincula UAIDs con manifests de
  capacidades.
- Hooks de auditoría (lista de eventos, esquema de logs, servicio de PagerDuty).

Reutiliza el JSON como punto de partida para nuevos dataspaces y ajusta las rutas de
whitelist para apuntar a los manifests de capacidades relevantes.

## 7. Publicación y rotación

1. **Codificar el UAID.** Deriva el digest blake2b‑32 y añade el prefijo `uaid:`:

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Codificar el payload Norito.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   O bien ejecutar el helper de CLI (que escribe tanto el archivo `.to` como un `.hash`
   con el digest BLAKE3‑256):

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Publicar a través de Torii.**

   ```bash
   # Si ya codificaste el manifest en Norito:
   iroha app space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   O usa la API HTTP descrita en la sección anterior. En ambos casos, asegúrate de
   archivar:
   - el JSON original del manifest;
   - el payload Norito `.to` y el hash BLAKE3;
   - la respuesta de Torii y el evento `SpaceDirectoryEvent`.

4. **Rotar o revocar.**

   - Para rotación planificada: prepara un manifest de reemplazo con `activation_epoch`
     futuro, ejecuta pruebas de tabletop y coordina la activación con los operadores de
     todos los dataspaces afectados.
   - Para revocación de emergencia: sigue el playbook de revocación (sección “Revoke” en
     la tabla de flujo de trabajo), incluye el identificador del incidente en el campo
     `reason` y conserva los logs y snapshots relevantes.

Al seguir este playbook, el Space Directory proporciona un registro canónico, verificable y
preparado para reguladores sobre cómo se conceden, limitan y revocan capacidades en todos
los dataspaces de Nexus.
