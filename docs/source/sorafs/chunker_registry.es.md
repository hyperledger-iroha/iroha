---
lang: es
direction: ltr
source: docs/source/sorafs/chunker_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48ab7fd78ac9dfe11fd8dfeaaeaa830df2410a9a5e71234f05688298404a8e37
source_last_modified: "2026-01-22T15:38:30.691198+00:00"
translation_last_reviewed: 2026-01-30
---

## Registro de perfiles de chunker de SoraFS (SF-2a)

La pila de SoraFS negocia el comportamiento de chunking mediante un registro
pequeño y con espacio de nombres. Cada perfil asigna parámetros CDC
deterministas, metadatos semver y el digest/multicodec esperado usado en
manifiestos y archivos CAR.

Los autores de perfiles deben consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md)
para conocer los metadatos requeridos, la lista de validación y la plantilla de
propuestas antes de enviar nuevas entradas. Una vez que la gobernanza haya
aprobado un cambio, siga la
[lista de despliegue del registro](chunker_registry_rollout_checklist.md) y el
[playbook de manifiestos de staging](runbooks/staging_manifest_playbook.md) para
promover los fixtures a staging y producción.

### Perfiles

| Espacio de nombres | Nombre | SemVer | ID de perfil | Mín (bytes) | Objetivo (bytes) | Máx (bytes) | Máscara de corte | Multihash | Alias | Notas |
|--------------------|--------|--------|--------------|-------------|------------------|-------------|------------------|-----------|-------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65 536 | 262 144 | 524 288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | Perfil canónico usado en los fixtures SF-1 |

El registro vive en el código como `sorafs_manifest::chunker_registry` (gobernado por [`chunker_registry_charter.md`](chunker_registry_charter.md)). Cada entrada
se expresa como un `ChunkerProfileDescriptor` con:

* `namespace` – agrupación lógica de perfiles relacionados (p. ej., `sorafs`).
* `name` – etiqueta de perfil legible para humanos (`sf1`, `sf1-fast`, …).
* `semver` – cadena de versión semántica del conjunto de parámetros.
* `profile` – el `ChunkProfile` real (min/target/max/mask).
* `multihash_code` – el multihash usado al producir digests de chunk (`0x1f`
  para el valor por defecto de SoraFS).

El manifiesto serializa perfiles mediante `ChunkingProfileV1`. La estructura
registra los metadatos del registro (namespace, name, semver) junto con los
parámetros CDC sin procesar y la lista de alias mostrada arriba. Los
consumidores deberían intentar primero una búsqueda en el registro por
`profile_id` y volver a los parámetros inline cuando aparezcan IDs desconocidos.
Las reglas de la carta del registro requieren que el handle canónico
(`namespace.name@semver`) sea la primera entrada en `profile_aliases`.

Para inspeccionar el registro desde las herramientas, ejecute la CLI de ayuda:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]

Todos los flags de la CLI que escriben JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceptan `-` como ruta, lo que transmite el payload a stdout en lugar de
crear un archivo. Esto facilita canalizar los datos a herramientas manteniendo el
comportamiento por defecto de imprimir el informe principal.

Para inspeccionar un testigo PoR específico, proporcione los índices de
chunk/segmento/hoja y opcionalmente persista la prueba en disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puede seleccionar un perfil por id numérico (`--profile-id=1`) o por el handle
registrado (`--profile=sorafs.sf1@1.0.0`); el formato de handle es conveniente
para scripts que enlazan namespace/name/semver directamente desde los metadatos
de gobernanza.

Use `--promote-profile=<handle>` para emitir un bloque de metadatos JSON (incluye
los alias registrados) que se puede pegar en `chunker_registry_data.rs` al
promover un nuevo perfil por defecto:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

El informe principal (y el archivo de prueba opcional) incluyen el digest raíz,
los bytes de hoja muestreados (codificados en hex) y los digests hermanos de
segmento/chunk para que los verificadores puedan rehacer los hashes de las capas
64 KiB/4 KiB contra el valor `por_root_hex`.

Para validar una prueba existente contra un payload, pase la ruta con
`--por-proof-verify` (la CLI añade `"por_proof_verified": true` cuando el
testigo coincide con la raíz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para muestreo por lotes, use `--por-sample=<count>` y, opcionalmente, proporcione
una semilla/ruta de salida. La CLI garantiza orden determinista (semilla
`splitmix64`) y truncará de forma transparente cuando la solicitud exceda las
hojas disponibles:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

El manifest stub refleja los mismos datos, lo que resulta conveniente al
scriptar la selección de `--chunker-profile-id` en pipelines. Ambas CLIs de
chunk store también aceptan el formato de handle canónico (`--profile=sorafs.sf1@1.0.0`) para
que los scripts de build eviten hardcodear IDs numéricos:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan las CLIs
vía `--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociación de chunkers

Gateways y clientes anuncian perfiles compatibles mediante provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

La planificación de chunk multi-source se anuncia mediante la capacidad `range`.
La CLI la acepta con `--capability=range[:streams]`, donde el sufijo numérico
opcional codifica la concurrencia preferida de fetch por rangos del proveedor
(por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven a la pista general `max_streams`
publicada en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker`
que enumere los tuplas `(namespace, name, semver)` soportadas en orden de
preferencia:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Los gateways seleccionan un perfil compatible mutuamente (por defecto
`sorafs.sf1@1.0.0`) y reflejan la decisión mediante el header de respuesta
`Content-Chunker`. Los manifiestos incorporan el perfil elegido para que los
nodos aguas abajo puedan validar el layout de chunks sin depender de la
negociación HTTP.

### Conformidad

* El perfil `sorafs.sf1@1.0.0` mapea a los fixtures públicos en
  `fixtures/sorafs_chunker` y a los corpus registrados bajo
  `fuzz/sorafs_chunker`. La paridad end-to-end se prueba en Rust, Go y Node con
  los tests proporcionados.
* `chunker_registry::lookup_by_profile` afirma que los parámetros del descriptor
  coinciden con `ChunkProfile::DEFAULT` para proteger contra divergencias
  accidentales.
* Los manifiestos producidos por `iroha app sorafs toolkit pack` y
  `sorafs_manifest_stub` incluyen los metadatos del registro.
