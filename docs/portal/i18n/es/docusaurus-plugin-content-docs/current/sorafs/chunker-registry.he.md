---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e743e3a6abc24e80b0ece3ccde56c4e48fab8d65c7853016cacfd6e5ab04af9
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: chunker-registry
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

## Registro de perfiles de chunker de SoraFS (SF-2a)

El stack de SoraFS negocia el comportamiento de chunking mediante un registro pequeño con espacios de nombres.
Cada perfil asigna parámetros CDC deterministas, metadatos semver y el digest/multicodec esperado usado en manifests y archivos CAR.

Los autores de perfiles deben consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para los metadatos requeridos, el checklist de validación y la plantilla de propuesta antes de enviar nuevas entradas.
Una vez que la gobernanza aprueba un cambio, sigue el
[checklist de rollout del registro](./chunker-registry-rollout-checklist.md) y el
[playbook de manifest en staging](./staging-manifest-playbook) para promover
los fixtures a staging y producción.

### Perfiles

| Namespace | Nombre | SemVer | ID de perfil | Min (bytes) | Target (bytes) | Max (bytes) | Máscara de corte | Multihash | Alias | Notas |
|-----------|--------|--------|-------------|-------------|----------------|-------------|-----------------|-----------|-------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canónico usado en fixtures SF-1 |

El registro vive en el código como `sorafs_manifest::chunker_registry` (gobernado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Cada entrada
se expresa como un `ChunkerProfileDescriptor` con:

* `namespace` – agrupación lógica de perfiles relacionados (p. ej., `sorafs`).
* `name` – etiqueta legible para humanos (`sf1`, `sf1-fast`, …).
* `semver` – cadena de versión semántica para el conjunto de parámetros.
* `profile` – el `ChunkProfile` real (min/target/max/mask).
* `multihash_code` – el multihash usado al producir digests de chunk (`0x1f`
  para el default de SoraFS).

El manifest serializa perfiles mediante `ChunkingProfileV1`. La estructura registra
la metadata del registro (namespace, name, semver) junto con los parámetros CDC
en bruto y la lista de alias mostrada arriba. Los consumidores deberían intentar una
búsqueda en el registro por `profile_id` y recurrir a los parámetros inline cuando
aparezcan IDs desconocidos; la lista de alias garantiza que los clientes HTTP puedan
seguir enviando handles heredados en `Accept-Chunker` sin adivinar. Las reglas de la
carta del registro exigen que el handle canónico (`namespace.name@semver`) sea la
primera entrada en `profile_aliases`, seguida de cualquier alias heredado.

Para inspeccionar el registro desde tooling, ejecuta el CLI helper:

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
```

Todas las flags del CLI que escriben JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceptan `-` como ruta, lo que transmite el payload a stdout en lugar de
crear un archivo. Esto facilita canalizar los datos a tooling mientras se mantiene el
comportamiento por defecto de imprimir el reporte principal.

### Matriz de rollout y plan de despliegue


La tabla siguiente captura el estado actual de soporte para `sorafs.sf1@1.0.0` en
componentes principales. "Bridge" se refiere al carril CARv1 + SHA-256
que requiere negociación explícita del cliente (`Accept-Chunker` + `Accept-Digest`).

| Componente | Estado | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Soportado | Valida el handle canónico + alias, transmite reportes vía `--json-out=-` y aplica la carta del registro con `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retirado | Constructor de manifest fuera de soporte; usa `iroha app sorafs toolkit pack` para empaquetado CAR/manifest y mantén `--plan=-` para revalidación determinista. |
| `sorafs_provider_advert_stub` | ⚠️ Retirado | Helper de validación offline únicamente; los provider adverts deben producirse por el pipeline de publicación y validarse vía `/v1/sorafs/providers`. |
| `sorafs_fetch` (developer orchestrator) | ✅ Soportado | Lee `chunk_fetch_specs`, entiende payloads de capacidad `range` y ensambla salida CARv2. |
| Fixtures de SDK (Rust/Go/TS) | ✅ Soportado | Regeneradas vía `export_vectors`; el handle canónico aparece primero en cada lista de alias y está firmado por sobres del consejo. |
| Negociación de perfiles en gateway Torii | ✅ Soportado | Implementa la gramática completa de `Accept-Chunker`, incluye headers `Content-Chunker` y expone el bridge CARv1 solo en solicitudes de downgrade explícitas. |

Despliegue de telemetría:

- **Telemetría de fetch de chunks** — la CLI de Iroha `sorafs toolkit pack` emite digests de chunk, metadata CAR y raíces PoR para ingestión en dashboards.
- **Provider adverts** — los payloads de adverts incluyen metadata de capacidades y alias; valida cobertura vía `/v1/sorafs/providers` (p. ej., presencia de la capacidad `range`).
- **Monitoreo de gateway** — los operadores deben reportar los pareos `Content-Chunker`/`Content-Digest` para detectar downgrades inesperados; se espera que el uso del bridge tienda a cero antes de la deprecación.

Política de deprecación: una vez que se ratifique un perfil sucesor, programa una ventana de publicación dual
(documentada en la propuesta) antes de marcar `sorafs.sf1@1.0.0` como deprecado en el registro y eliminar el
bridge CARv1 de los gateways en producción.

Para inspeccionar un testigo PoR específico, proporciona índices de chunk/segmento/hoja y opcionalmente
persiste la prueba a disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puedes seleccionar un perfil por id numérico (`--profile-id=1`) o por handle de registro
(`--profile=sorafs.sf1@1.0.0`); la forma con handle es conveniente para scripts que
pasen namespace/name/semver directamente desde metadatos de gobernanza.

Usa `--promote-profile=<handle>` para emitir un bloque JSON de metadatos (incluyendo todos los alias
registrados) que puede pegarse en `chunker_registry_data.rs` al promover un nuevo perfil por defecto:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

El reporte principal (y el archivo de prueba opcional) incluye el digest raíz, los bytes de hoja muestreados
(codificados en hex) y los digests hermanos de segmento/chunk para que los verificadores puedan rehashear
las capas de 64 KiB/4 KiB contra el valor `por_root_hex`.

Para validar una prueba existente contra un payload, pasa la ruta vía
`--por-proof-verify` (el CLI añade `"por_proof_verified": true` cuando el testigo
coincide con la raíz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para muestreo en lote, usa `--por-sample=<count>` y opcionalmente proporciona una ruta de seed/salida.
El CLI garantiza un orden determinista (sembrado con `splitmix64`) y truncará de forma transparente cuando
la solicitud supere las hojas disponibles:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

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

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implícito vía registro)
    capabilities: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

Los gateways seleccionan un perfil soportado mutuamente (por defecto `sorafs.sf1@1.0.0`)
y reflejan la decisión vía el header de respuesta `Content-Chunker`. Los manifests
embeben el perfil elegido para que los nodos downstream puedan validar el layout de chunks
sin depender de la negociación HTTP.

### Soporte CAR

retendremos una ruta de exportación CARv1+SHA-2:

* **Ruta primaria** – CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de chunk registrado como arriba.
  PUEDEN exponer esta variante cuando el cliente omite `Accept-Chunker` o solicita
  `Accept-Digest: sha2-256`.

adicionales para transición pero no deben reemplazar el digest canónico.

### Conformidad

* El perfil `sorafs.sf1@1.0.0` se asigna a los fixtures públicos en
  `fixtures/sorafs_chunker` y a los corpora registrados en
  `fuzz/sorafs_chunker`. La paridad end-to-end se ejerce en Rust, Go y Node
  mediante las pruebas provistas.
* `chunker_registry::lookup_by_profile` afirma que los parámetros del descriptor
  coinciden con `ChunkProfile::DEFAULT` para evitar divergencias accidentales.
* Los manifests producidos por `iroha app sorafs toolkit pack` y `sorafs_manifest_stub` incluyen la metadata del registro.
