---
lang: es
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de operaciones del manifiesto de direcciones (ADDR-7c)

Este runbook operacionaliza el ítem del roadmap **ADDR-7c** al detallar cómo
verificar, publicar y retirar entradas en el manifiesto de cuentas/alias de Sora
Nexus. Complementa el contrato técnico en
[`docs/account_structure.md`](../../account_structure.md) §4 y las expectativas
de telemetría registradas en `dashboards/grafana/address_ingest.json`.

## 1. Alcance e insumos

| Entrada | Fuente | Notas |
|-------|--------|-------|
| Paquete de manifiesto firmado (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin de SoraFS (`sorafs://address-manifests/<CID>/`) y espejo HTTPS | Los paquetes los emite la automatización de releases; conserve la estructura de directorios al espejar. |
| Digest + secuencia del manifiesto anterior | Paquete previo (mismo patrón de ruta) | Requerido para probar monotonicidad/inmutabilidad. |
| Acceso a telemetría | Dashboard `address_ingest` de Grafana + Alertmanager | Necesario para monitorizar retiro de Local‑8 y picos de direcciones inválidas. |
| Herramientas | `cosign`, `shasum`, `b3sum` (o `python3 -m blake3`), `jq`, CLI `iroha`, `scripts/account_fixture_helper.py` | Instale antes de ejecutar la lista de verificación. |

## 2. Estructura de artefactos

Cada paquete sigue el layout de abajo; no renombre archivos al copiar entre
entornos.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

Campos de encabezado de `manifest.json`:

| Campo | Descripción |
|-------|-------------|
| `version` | Versión del esquema (actualmente `1`). |
| `sequence` | Número de revisión monótono; debe incrementarse exactamente en uno. |
| `generated_ms` | Marca de tiempo UTC de publicación (milisegundos desde epoch). |
| `ttl_hours` | Vida máxima de caché que Torii/SDKs pueden respetar (por defecto 24). |
| `previous_digest` | BLAKE3 del cuerpo del manifiesto anterior (hex). |
| `entries` | Arreglo ordenado de registros (`global_domain`, `local_alias` o `tombstone`). |

## 3. Procedimiento de verificación

1. **Descargar el paquete.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Guardarraíl de checksum.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   Todos los archivos deben reportar `OK`; trate los desajustes como manipulación.

3. **Verificación Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Prueba de inmutabilidad.** Compare `sequence` y `previous_digest` contra el
   manifiesto archivado:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   El digest impreso debe coincidir con `previous_digest`. No se permiten saltos
   de secuencia; reemita el manifiesto si se viola.

5. **Cumplimiento de TTL.** Asegure que `generated_ms + ttl_hours` cubra las
   ventanas de despliegue previstas; de lo contrario la gobernanza debe
   republicar antes de que expiren los cachés.

6. **Sanidad de entradas.**
   - Las entradas `global_domain` DEBEN incluir `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - Las entradas `local_alias` DEBEN incorporar el digest de 12 bytes producido
     por Norm v1 (use `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
     para confirmar; el resumen JSON refleja el dominio provisto vía `input_domain` y
     `--append-domain` reproduce la codificación convertida como `<ih58>@<domain>` para manifiestos).
   - Las entradas `tombstone` DEBEN referenciar el selector exacto a retirar e
     incluir los campos `reason_code`, `ticket` y `replaces_sequence`.

7. **Paridad de fixtures.** Regenere vectores canónicos y asegure que la tabla
   de digests Local no cambió inesperadamente:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Guardarraíl de automatización.** Ejecute el verificador del manifiesto para
   revalidar el paquete end-to-end (esquema del encabezado, forma de entradas,
   checksums y cableado de previous-digest):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   La bandera `--previous` apunta al paquete inmediatamente anterior para que la
   herramienta confirme la monotonicidad de `sequence` y recompute la prueba
   BLAKE3 de `previous_digest`. El comando falla rápido cuando un checksum deriva
   o un selector `tombstone` omite los campos requeridos, así que incluya la
   salida en su ticket de cambio antes de solicitar firmas.

## 4. Flujo de cambios de alias y tombstone

1. **Proponer el cambio.** Abra un ticket de gobernanza que indique el código de
   razón (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED`, etc.) y los selectores afectados.
2. **Derivar payloads canónicos.** Para cada alias a actualizar, ejecute:

   ```bash
   iroha tools address convert sora...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **Borrador de entrada de manifiesto.** Agregue un registro JSON como:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   Cuando sustituya un alias Local por uno Global, incluya tanto un registro
   `tombstone` como el registro `global_domain` subsiguiente con el discriminante
   de Nexus.

4. **Validar el paquete.** Reejecute los pasos de verificación anteriores contra
   el manifiesto en borrador antes de solicitar firmas.
5. **Publicar y monitorear.** Tras la firma de gobernanza, siga §3 y mantenga la
   valor por defecto `true` en clústeres de producción una vez que las métricas
   confirmen uso Local‑8 en cero. Solo cambie la bandera a `false` en clústeres
   dev/test cuando necesite tiempo adicional de soak.

## 5. Monitoreo y rollback

- Dashboards: `dashboards/grafana/address_ingest.json` (paneles para
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`, y
  `torii_address_collision_domain_total{endpoint,domain}`) deben mantenerse en
  verde durante 30 días antes de bloquear de forma permanente el tráfico Local‑8/Local‑12.
- Evidencia de bloqueo: exporte una consulta de rango de 30 días de Prometheus para
  `torii_address_local8_total` y `torii_address_collision_total` (p. ej.,
  `promtool query range --output=json ...`) y ejecute
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`;
  adjunte el JSON + salida de CLI a los tickets de despliegue para que gobernanza
  vea la ventana de cobertura y confirme que los contadores se mantuvieron planos.
- Alertas (vea `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — pagina cuando cualquier contexto reporta un nuevo
    incremento de Local‑8. Trátelo como bloqueador de release, detenga despliegues
    hasta remediar el cliente que originó el incremento y limpiar la telemetría.
  - `AddressLocal12Collision` — se dispara en el momento en que dos etiquetas
    Local‑12 hashean al mismo digest. Pause promociones del manifiesto, ejecute
    `scripts/address_local_toolkit.sh` para confirmar el mapeo de digest y
    coordine con la gobernanza de Nexus antes de reemitir la entrada afectada del
    registro.
  - `AddressInvalidRatioSlo` — advierte cuando los envíos IH58/comprimidos inválidos
    exceden el SLO global de 0,1 % durante diez minutos. Investigue
    `torii_address_invalid_total` por contexto/razón y coordine con el equipo
    SDK propietario antes de declarar el incidente resuelto.
- Logs: conserve las líneas de log `manifest_refresh` de Torii y el número de ticket
  de gobernanza en `notes.md`.
- Rollback: republique el paquete anterior (mismos archivos, ticket incrementado
  solo en el entorno afectado hasta resolver el problema.

## 6. Referencias

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (contrato).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (sincronización de fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (digests canónicos).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (telemetría).
