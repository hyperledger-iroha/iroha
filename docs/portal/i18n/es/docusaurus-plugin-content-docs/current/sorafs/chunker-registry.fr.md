---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro fragmentador
título: Registro de perfiles fragmentados SoraFS
sidebar_label: Registro fragmentado
descripción: ID de perfil, parámetros y plan de negociación para el registro fragmentador SoraFS.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry.md`. Guarde las dos copias sincronizadas junto con la retraite complète du set Sphinx hérité.
:::

## Registro de perfiles fragmentados SoraFS (SF-2a)

La pila SoraFS negocia el comportamiento de fragmentación a través de un pequeño espacio de nombres de registro.
Cada perfil asigna los parámetros determinados por CDC, los metadatos cada semana y el resumen/multicodec que se utiliza en los manifiestos y archivos CAR.

Les auteurs de profils doivent consultor
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Para los requisitos requeridos, la lista de verificación de validación y el modelo de propuesta previa.
de soumettre de nuevos platos principales. Una vez que una modificación está aprobada por la
gobernanza, suivez la
[lista de verificación de implementación del registro](./chunker-registry-rollout-checklist.md) y le
[playbook de manifest y staging](./staging-manifest-playbook) para promocionar
les fixtures vers puesta en escena y producción.

### Perfiles| Espacio de nombres | Nombre | SemVer | ID de perfil | Mín (octetos) | Cible (octetos) | Máx. (octetos) | Máscara de ruptura | Multihash | Alias ​​| Notas |
|-----------|-----|--------|-------------|--------------|----------------|--------------|------------------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canónico utilizado en los accesorios SF-1 |

El registro se realiza en el código `sorafs_manifest::chunker_registry` (régimen [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Chaque entrante
est exprimée como un `ChunkerProfileDescriptor` con:

* `namespace` – reagrupamiento logístico de perfiles liés (ej., `sorafs`).
* `name` – calumnia lisible (`sf1`, `sf1-fast`,…).
* `semver` – cadena de versión sémántica para el juego de parámetros.
* `profile` – el carrete `ChunkProfile` (mínimo/objetivo/máximo/máscara).
* `multihash_code`: el multihash se utiliza para la producción de resúmenes de fragmentos (`0x1f`
  por defecto SoraFS).El manifiesto serializa los perfiles a través de `ChunkingProfileV1`. La estructura registra los metadonnées
du registre (namespace, name, semver) aux côtés des paramètres CDC bruts et de la liste d'alias ci-dessus.
Les consommateurs doivent d'abord tenter unae recherche dans le registre par `profile_id` et revenir aux
parámetros en línea cuando los ID no aparecen correctamente; la lista de alias garantiza que los clientes HTTP
du registre exigent que le handle canonique (`namespace.name@semver`) soit la première entrée de
`profile_aliases`, seguido de alias herités.

Para inspeccionar el registro después de las herramientas, ejecute el asistente CLI:

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

Todas las banderas de CLI que se escriben en JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) acepta `-` como camino, esto es lo que transmite la carga útil a la salida estándar en lugar de
crear un archivo. Esto facilita la tubería de los données versus las herramientas todos conservando el
Comportamiento por defecto de imprimación de la relación principal.

### Matriz de implementación y plan de implementación


El cuadro ci-dessous captura el estado de soporte actual para `sorafs.sf1@1.0.0` en les
componentes principales. "Bridge" diseño la vía CARv1 + SHA-256 aquí
Necesite una negociación explícita con el cliente (`Accept-Chunker` + `Accept-Digest`).| Componente | Estatuto | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Apoyado | Valide le handle canonique + alias, transmita las relaciones a través de `--json-out=-` y aplique la carta de registro a través de `ensure_charter_compliance()`. |
| `sorafs_fetch` (organizador de desarrollo) | ✅ Apoyado | Encienda `chunk_fetch_specs`, comprenda las cargas útiles de capacidad `range` y monte una salida CARv2. |
| SDK de accesorios (Rust/Go/TS) | ✅ Apoyado | Régénérées vía `export_vectors`; le handle canonique apparaît en premier dans cada lista de alias et est signé par des sobres du conseil. |
| Negociación de perfiles de puerta de enlace Torii | ✅ Apoyado | Implemente toda la gramática `Accept-Chunker`, incluidos los encabezados `Content-Chunker` y no exponga el puente CARv1 que sur des demandes de downgrade explícitamente. |

Despliegue de la televisión:- **Télémétrie de fetch de chunks** — la CLI Iroha `sorafs toolkit pack` emet des digestes de chunks, des métadonnées CAR y des racines PoR para ingestión en los tableros.
- **Anuncios del proveedor**: las cargas útiles de los anuncios incluyen las metadonnées de capacidades y alias; validez de la cobertura vía `/v2/sorafs/providers` (ej., presencia de la capacidad `range`).
- **Puerta de enlace de vigilancia**: los operadores deben informar sobre los acoplamientos `Content-Chunker`/`Content-Digest` para detectar las degradaciones inatendidas; El uso del puente está censurado tendre a cero antes de la depreciación.

Política de depreciación: una vez que un perfil sucesor está ratificado, planifique una ventana de doble publicación
(documentée dans la proposition) avant de marquer `sorafs.sf1@1.0.0` como déprécié dans le registre et de retirer le
puente CARv1 des gateways en producción.

Para inspeccionar un témoin PoR específico, indique los índices de trozo/segmento/hoja y, opcionalmente,
persiste la preuve sur disque:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puede seleccionar un perfil por ID numérico (`--profile-id=1`) o por identificador de registro.
(`--profile=sorafs.sf1@1.0.0`); la forma manejada es práctica para los scripts qui
transmettent namespace/name/semver directement depuis les métadonnées de gouvernance.Utilice `--promote-profile=<handle>` para crear un bloque JSON de metadonnées (y comprende todos los alias
registrados) qui peut être collé dans `chunker_registry_data.rs` lors de la promoción de un nuevo perfil
por defecto:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

La relación principal (y el archivo de preuve opcional) incluye el resumen racine, los octetos de hojas encantilladas
(codificados en hexadécimal) y los resúmenes hermanos de segmento/chunk para que los verificadores puedan rehacher
Los sofás de 64 KiB/4 KiB se enfrentan al valor `por_root_hex`.

Para validar un preuve existente contre un payload, pase el camino a través
`--por-proof-verify` (el CLI agrega `"por_proof_verified": true` cuando el témoin
corresponden a la racine calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para el échantillonnage por lotes, utilice `--por-sample=<count>` y conecte eventualmente un camino de semilla/salida.
La CLI garantiza un orden determinado (semilla con `splitmix64`) y se activa automáticamente cuando
la petición de pasar las hojas disponibles :```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

```
$ ejecución de carga -p sorafs_manifest --bin sorafs_manifest_stub --list-chunker-profiles
[
  {
    "id_perfil": 1,
    "espacio de nombres": "sorafs",
    "nombre": "sf1",
    "semver": "1.0.0",
    "manejar": "sorafs.sf1@1.0.0",
    "tamaño_mínimo": 65536,
    "tamaño_objetivo": 262144,
    "tamaño_máximo": 524288,
    "break_mask": "0x0000ffff",
    "código_multihash": 31
  }
]
```

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
ProveedorAdvertBodyV1 {
    ...
    chunk_profile: perfil_id (implícito mediante registro)
    capacidades: [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

Las puertas de enlace seleccionan un perfil compatible entre sí (por defecto `sorafs.sf1@1.0.0`)
y refleja la decisión a través del encabezado de respuesta `Content-Chunker`. Los manifiestos
integre la elección del perfil a fin de que los nudos posteriores puedan validar el diseño de los fragmentos
Sin utilizar la negociación HTTP.

### Coche de soporte

Conservamos un camino de exportación CARv1+SHA-2:

* **Chemin principal** – CARv2, resumen de carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de fragmento registrado como ci-dessus.
  PEUVENT expositor esta variante cuando el cliente omet `Accept-Chunker` o demanda
  `Accept-Digest: sha2-256`.

No se deben reemplazar los suplementos para la transición con el resumen canónico.

### Conformidad* Le perfil `sorafs.sf1@1.0.0` corresponde a accesorios auxiliares públicos en
  `fixtures/sorafs_chunker` y aux corpora registrados bajo techo
  `fuzz/sorafs_chunker`. La pareja de combate se ejercita en Rust, Go y Node
  a través de les tests fournis.
* `chunker_registry::lookup_by_profile` afirma que los parámetros del descriptor
  corresponsal a `ChunkProfile::DEFAULT` para evitar toda divergencia accidental.
* Les manifests produits par `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` incluyen les métadonnées du registre.