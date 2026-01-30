---
lang: es
direction: ltr
source: docs/source/sorafs/chunker_profile_authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c1175d924c631a7b97bc78cbe08ef1d02455f95d61556fc908706ed9ea3472d6
source_last_modified: "2026-01-04T10:50:53.654130+00:00"
translation_last_reviewed: 2026-01-30
---

# Guía de creación de perfiles de chunker de SoraFS

Esta guía explica cómo proponer y publicar nuevos perfiles de chunker para SoraFS.
Complementa el RFC de arquitectura (SF-1) y la referencia del registro (SF-2a)
con requisitos concretos de autoría, pasos de validación y plantillas de propuesta.
Para un ejemplo canónico, consulte
[`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`](proposals/sorafs_sf1_profile_v1.json)
y el registro de dry-run correspondiente en
[`docs/source/sorafs/reports/sf1_determinism.md`](reports/sf1_determinism.md).

## Resumen

Cada perfil que entra en el registro debe:

- anunciar parámetros CDC deterministas y ajustes de multihash idénticos entre
  arquitecturas;
- entregar fixtures reproducibles (JSON de Rust/Go/TS + corpus de fuzzing +
  testigos PoR) que los SDKs aguas abajo puedan verificar sin herramientas a
  medida;
- incluir metadatos listos para gobernanza (namespace, name, semver) más
  información de migración;
- superar la suite de diff determinista antes de la revisión del consejo.

Siga la lista de verificación a continuación para preparar una propuesta que
cumpla estas reglas.

## Resumen de la carta del registro

Antes de redactar una propuesta, confirme que cumple con la carta del registro
aplicada por `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monótona sin
  huecos.
- El identificador canónico (`namespace.name@semver`) debe aparecer en la lista
  de alias y **debe** ser la primera entrada.
- Ningún alias puede colisionar con otro identificador canónico ni aparecer más
  de una vez.
- Los alias no deben estar vacíos y deben estar recortados de espacios en blanco.

Herramientas CLI útiles:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil candidato por defecto (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos mantienen las propuestas alineadas con la carta del registro y
proporcionan los metadatos canónicos necesarios en las discusiones de gobernanza.

## Metadatos requeridos

| Campo | Descripción | Ejemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Agrupación lógica de perfiles relacionados. | `sorafs` |
| `name` | Etiqueta legible para humanos. | `sf1` |
| `semver` | Cadena de versión semántica para el conjunto de parámetros. | `1.0.0` |
| `profile_id` | Identificador numérico monótono asignado una vez que el perfil se publica. Reserve el siguiente id pero no reutilice números existentes. | `1` |
| `profile_aliases` | Identificadores adicionales opcionales expuestos a los clientes durante la negociación. Incluya siempre el handle canónico como primera entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Longitud mínima del chunk en bytes. | `65536` |
| `profile.target_size` | Longitud objetivo del chunk en bytes. | `262144` |
| `profile.max_size` | Longitud máxima del chunk en bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa usada por el hash rodante (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante polinómica Gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semilla usada para derivar la tabla Gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para digests por chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest del paquete canónico de fixtures. | `13fa...c482` |
| `fixtures_root` | Directorio relativo que contiene los fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semilla para el muestreo PoR determinista (`splitmix64`). | `0xfeedbeefcafebabe` (ejemplo) |

Los metadatos deben aparecer tanto en el documento de propuesta como dentro de
los fixtures generados para que el registro, las herramientas CLI y la
automatización de gobernanza puedan confirmar los valores sin cruces manuales.
En caso de duda, ejecute los CLIs del chunk-store y del manifest con `--json-out=-`
para transmitir los metadatos calculados a las notas de revisión.

### Puntos de contacto de CLI y registro

- `sorafs_manifest_chunk_store --profile=<handle>` – vuelve a ejecutar los
  metadatos del chunk, el digest del manifiesto y las comprobaciones PoR con los
  parámetros propuestos.
- `sorafs_manifest_chunk_store --json-out=-` – envía el informe del chunk-store
  a stdout para comparaciones automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` – confirma que los
  manifiestos y planes CAR incorporen el handle canónico más los alias.
- `sorafs_manifest_stub --plan=-` – reutiliza el `chunk_fetch_specs` previo para
  verificar offsets/digests después del cambio.

Registre la salida de los comandos (digests, raíces PoR, hashes de manifiesto)
en la propuesta para que los revisores puedan reproducirlos literalmente.

## Lista de verificación de determinismo y validación

1. **Regenerar fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar la suite de paridad** – `cargo test -p sorafs_chunker` y el
   harness de diff entre lenguajes (`crates/sorafs_chunker/tests/vectors.rs`)
   deben quedar en verde con los nuevos fixtures en su lugar.
3. **Reproducir corpus de fuzz/back-pressure** – ejecute `cargo fuzz list` y el
   harness de streaming (`fuzz/sorafs_chunker`) contra los activos regenerados.
4. **Verificar testigos de Prueba de Recuperabilidad (PoR)** – ejecute
   `sorafs_manifest_chunk_store --por-sample=<n>` usando el perfil propuesto y
   confirme que las raíces coincidan con el manifiesto de fixtures.
5. **Dry run de CI** – invoque `ci/check_sorafs_fixtures.sh` localmente; el
   script debería tener éxito con los nuevos fixtures y el `manifest_signatures.json`
   existente.
6. **Confirmación cruzada de runtime** – asegure que los bindings Go/TS consuman
   el JSON regenerado y emitan límites y digests de chunk idénticos.

Documente los comandos y los digests resultantes en la propuesta para que el
Tooling WG pueda re-ejecutarlos sin conjeturas.

### Confirmación de manifiesto / PoR

Después de regenerar los fixtures, ejecute la canalización completa de
manifiestos para garantizar que los metadatos CAR y las pruebas PoR se mantengan
coherentes:

```bash
# Validar metadatos de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifiesto + CAR y capturar el plan de fetch de chunks
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-ejecutar usando el plan de fetch guardado (protege contra offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Sustituya el archivo de entrada por cualquier corpus representativo usado en
sus fixtures (por ejemplo, el stream determinista de 1 GiB) y adjunte los
digests resultantes a la propuesta.

## Plantilla de propuesta

Las propuestas se envían como registros Norito `ChunkerProfileProposalV1`
versionados y registrados en `docs/source/sorafs/proposals/`. La plantilla JSON
a continuación ilustra la forma esperada (sustituya sus valores según sea
necesario):


Proporcione un informe Markdown coincidente (`determinism_report`) que capture
la salida de los comandos, los digests de chunk y cualquier desviación
encontrada durante la validación.

## Flujo de gobernanza

1. **Enviar un PR con propuesta + fixtures.** Incluya los activos generados, la
   propuesta Norito y las actualizaciones de `chunker_registry_data.rs`.
2. **Revisión del Tooling WG.** Los revisores vuelven a ejecutar la lista de
   validación y confirman que la propuesta cumple las reglas del registro (sin
   reutilización de IDs, determinismo satisfecho).
3. **Sobre del consejo.** Una vez aprobado, los miembros del consejo firman el
   digest de la propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`)
   y añaden sus firmas al sobre de perfil almacenado junto a los fixtures.
4. **Publicación del registro.** El merge actualiza el registro, los docs y los
   fixtures. El CLI por defecto permanece en el perfil anterior hasta que la
   gobernanza declare que la migración está lista.
5. **Seguimiento de deprecaciones.** Después de la ventana de migración,
   actualice el registro en el ledger.

## Consejos de autoría

- Prefiera límites pares de potencias de dos para minimizar comportamientos de
  chunking en casos extremos.
- Evite cambiar el código multihash sin coordinar el manifiesto y el gateway.
- Mantenga las semillas de la tabla Gear legibles por humanos pero globalmente
  únicas para simplificar las trazas de auditoría.
- Guarde cualquier artefacto de benchmarking (por ejemplo, comparaciones de
  rendimiento) en `docs/source/sorafs/reports/` para referencia futura.

Para las expectativas operativas durante el despliegue consulte el ledger de
migración (`docs/source/sorafs/migration_ledger.md`). Para las reglas de
conformidad en runtime consulte `docs/source/sorafs/chunker_conformance.md`.
