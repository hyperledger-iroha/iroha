---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: Guía de autoría de perfiles de chunker de SoraFS
sidebar_label: Guía de autoridad de chunker
descripción: Lista de verificación para proponer nuevos perfiles y accesorios de trozos de SoraFS.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_profile_authoring.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# Guía de autoría de perfiles de chunker de SoraFS

Esta guía explica cómo proponer y publicar nuevos perfiles de chunker para SoraFS.
Complementa el RFC de arquitectura (SF-1) y la referencia del registro (SF-2a)
con requisitos concretos de autoridad, pasos de validación y plantillas de propuesta.
Para un ejemplo canónico, consulta
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
y el registro de dry-run asociado en
`docs/source/sorafs/reports/sf1_determinism.md`.

## Resumen

Cada perfil que ingresa en el registro debe:

- anunciar parámetros CDC deterministas y ajustes de multihash idénticos entre
  arquitectura;
- Entregar accesorios reproducibles (JSON Rust/Go/TS + corpus fuzz + testigos PoR) que
  Los SDK posteriores pueden verificarse sin herramientas en medida;
- incluir metadatos listos para gobernanza (namespace, name, semver) junto con guía de rollout
  y ventanas operativas; y
- pasar la suite de diff determinista antes de la revisión del consejo.

Sigue la lista de verificación de abajo para preparar una propuesta que cumpla esas reglas.## Resumen de la carta de registro

Antes de redactar una propuesta, confirme que cumple la carta del registro aplicada
por `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Los ID de perfil son enteros positivos que aumentan de forma monótona sin huecos.
- El handle canónico (`namespace.name@semver`) debe aparecer en la lista de alias
  y **debe** ser la primera entrada. Siguen los alias heredados (p. ej., `sorafs.sf1@1.0.0`).
- Ningún alias puede colisionar con otro identificador canónico ni aparecer más de una vez.
- Los alias deben ser no vacíos y recortados de espacios en blanco.

Ayudas útiles de CLI:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos mantienen las propuestas alineadas con la carta del registro y proporcionan los
metadatos canónicos necesarios en las discusiones de gobernanza.

## Metadatos requeridos| Campo | Descripción | Ejemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Agrupación lógica de perfiles relacionados. | `sorafs` |
| `name` | Etiqueta legible para humanos. | `sf1` |
| `semver` | Cadena de versión semántica para el conjunto de parámetros. | `1.0.0` |
| `profile_id` | Identificador numérico monótono asignado una vez que el perfil ingresa. Reserve el siguiente id pero no reutilice los números existentes. | `1` |
| `profile_aliases` | Maneja adicionales opcionales (nombres heredados, abreviaturas) expuestos a clientes durante la negociación. Incluye siempre el mango canónico como la primera entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Longitud mínima de fragmento en bytes. | `65536` |
| `profile.target_size` | Longitud objetivo de fragmento en bytes. | `262144` |
| `profile.max_size` | Longitud máxima de fragmento en bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa usada por el Rolling Hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Engranaje constante del polinomio (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed usada para derivar la tabla gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para resúmenes por fragmento. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumen del paquete canónico de accesorios. | `13fa...c482` || `fixtures_root` | Directorio relativo que contiene los aparatos regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para el muestreo PoR determinista (`splitmix64`). | `0xfeedbeefcafebabe` (ejemplo) |

Los metadatos deben aparecer tanto en el documento de propuesta como dentro de los accesorios.
regenerados para que el registro, el tooling de CLI y la automatización de gobernanza puedan
confirmar los valores sin cruces manuales. Si hay dudas, ejecuta los CLI de chunk-store y
manifest con `--json-out=-` para transmitir los metadatos calculados a las notas de revisión.

### Puntos de contacto de CLI y registro

- `sorafs_manifest_chunk_store --profile=<handle>` — volver a ejecutar metadatos de fragmento,
  resumen de manifiesto y comprobaciones PoR con los parámetros propuestos.
- `sorafs_manifest_chunk_store --json-out=-` — transmite el informe de chunk-store a
  stdout para comparaciones automáticas.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmar que manifests y planes CAR
  embeben el mango canónico más los alias.
- `sorafs_manifest_stub --plan=-` — volver a alimentar el `chunk_fetch_specs` anterior para
  verificar compensaciones/resúmenes después del cambio.

Registra la salida de comandos (digests, raíces PoR, hashes de manifest) en la propuesta para que
los revisores pueden reproducirlos textualmente.

## Lista de verificación de determinismo y validación1. **Regenerar accesorios**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar la suite de paridad** — `cargo test -p sorafs_chunker` y el arnés diff
   entre lenguajes (`crates/sorafs_chunker/tests/vectors.rs`) deben estar en verde con los
   Nuevos accesorios en su lugar.
3. **Reproducir corpora fuzz/back-pression** — ejecuta `cargo fuzz list` y el arnés de
   streaming (`fuzz/sorafs_chunker`) contra los activos regenerados.
4. **Verificar testigos Prueba de recuperabilidad** — ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` usando el perfil propuesto y confirma que las
   raíces coinciden con el manifiesto de accesorios.
5. **Dry run de CI** — invoca `ci/check_sorafs_fixtures.sh` localmente; el guión
   debe tener éxito con los nuevos accesorios y el `manifest_signatures.json` existente.
6. **Confirmación cross-runtime** — asegura que los enlaces Go/TS consumieron el JSON regenerado
   y emiten límites y digiere idénticos.

Documenta los comandos y los resúmenes resultantes en la propuesta para que el Tooling WG pueda
repetirlos sin conjeturas.

### Confirmación de manifiesto / PoR

Después de regenerar accesorios, ejecuta el pipeline completo de manifiesto para asegurar que los
metadatos CAR y las pruebas PoR siguen siendo consistentes:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Reemplaza el archivo de entrada con cualquier corpus representativo usado por tus accesorios
(p. ej., el stream determinista de 1 GiB) y adjunta los resúmenes resultantes a la propuesta.

## Plantilla de propuestaLas propuestas se envían como registros Norito `ChunkerProfileProposalV1` registrados en
`docs/source/sorafs/proposals/`. La plantilla JSON de abajo ilustra la forma esperada
(sustituye tus valores según sea necesario):


Proporciona un informe Markdown correspondiente (`determinism_report`) que captura la
salida de comandos, los resúmenes de fragmentos y cualquier desviación encontrada durante la validación.

## Flujo de gobernanza

1. **Enviar PR con propuesta + fixtures.** Incluye los activos generados, la propuesta
   Norito y las actualizaciones a `chunker_registry_data.rs`.
2. **Revisión del Tooling WG.** Los revisores reejecutan el checklist de validación y
   confirman que la propuesta se alinea con las reglas del registro (sin reutilización de id,
   determinismo satisfecho).
3. **Sobre del consejo.** Una vez aprobado, los miembros del consejo firman el digest de
   la propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) y anexan sus
   firmas al sobre del perfil guardado junto a los accesorios.
4. **Publicación del registro.** El merge actualiza el registro, los documentos y los accesorios. el
   CLI por defecto permanece en el perfil anterior hasta que la gobernanza declare la migración
   lista.
5. **Seguimiento de deprecación.** Después de la ventana de migración, actualice el registro
   libro de migraciones.

##Consejos de autoridad- Prefiere límites de potencia de dos pares para minimizar el comportamiento de fragmentación en casos fronterizos.
- Evita cambiar el código multihash sin coordinar consumidores de manifest y gateway; incluye una
  nota operativa cuando lo hagas.
- Mantén las semillas de la tabla gear legibles para humanos pero globalmente únicas para simplificar
  auditorías.
- Guarda cualquier artefacto de benchmarking (p. ej., comparaciones de rendimiento) bajo
  `docs/source/sorafs/reports/` para referencia futura.

Para expectativas operativas durante el lanzamiento consulta el libro mayor de migración
(`docs/source/sorafs/migration_ledger.md`). Para reglas de conformidad en versión runtime
`docs/source/sorafs/chunker_conformance.md`.