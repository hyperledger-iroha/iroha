---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry
כותרת: Registro de perfiles de chunker de SoraFS
sidebar_label: Registro de chunker
תיאור: תעודות זהות, פרמטרים ותוכנית ניהול עבור רישום של chunker de SoraFS.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

## רישום פרופילי chunker de SoraFS (SF-2a)

El stack de SoraFS negocia el comportamiento de chunking mediante un registro pequeño con espacios de nombres.
Cada perfil asigna parametros CDC deterministas, metadatos semver y el digest/multicodec esperado usado en manifests y archivos CAR.

Los autores de perfiles deben consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
עבור דרישת השינויים, רשימת התיוג ותקשורת ה-Plantilla de Propuesta Antes de Enviar Nuevas Entradas.
Una vez que la gobernanza aprueba un cambio, sigue el
[רשימת בדיקה לרישום](./chunker-registry-rollout-checklist.md) y el
[playbook de manifest en staging](./staging-manifest-playbook) עבור מקדם
los fixtures בימוי וייצור.

### פרפילים

| מרחב שמות | Nombre | SemVer | ID de perfil | דקות (בתים) | יעד (בתים) | מקסימום (בתים) | Máscara de corte | Multihash | כינוי | Notas |
|------------|--------|--------|-------------------------------------------|----------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | פרופיל canónico usado en fixtures SF-1 |

El registro vive en el código como `sorafs_manifest::chunker_registry` (gobernado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). קאדה אנטרדה
ראה ביטוי כמו `ChunkerProfileDescriptor` קו:

* `namespace` – agrupación lógica de perfiles relacionados (עמוד ej., `sorafs`).
* `name` - כללי התנהגות קריא למען אנוש (`sf1`, `sf1-fast`, …).
* `semver` – קדנה דה גרסה סמנטיה עבור אל חיבורי פרמטרים.
* `profile` – el `ChunkProfile` אמיתי (מינימום/יעד/מקסימום/מסכה).
* `multihash_code` – el multihash usado al producer digests de chunk (`0x1f`
  para el default de SoraFS).

El manifest serializa perfiles mediante `ChunkingProfileV1`. La estructura registra
la metadata del registro (מרחב שמות, שם, סמבר) junto con los parametros CDC
en bruto y la list de alias mostrada arriba. Los consumidores deberían intentar una
búsqueda en el registro por `profile_id` y recurrir a los parámetros inline cuando
aparezcan IDs desconocidos; רשימה דה alias garantiza que los clientes HTTP puedan
seguir enviando מטפל ב-heredados en `Accept-Chunker` sin adivinar. לאס רגלס דה לה
carta del registro exigen que el handle canónico (`namespace.name@semver`) sea la
primera entrada en `profile_aliases`, seguida de cualquier alias heredado.

עבור בדיקה של רישום כלי עבודה, הוצאת מסייע CLI:

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
```כל הדגלים של CLI que escriben JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceptan `-` como ruta, lo que transmite el payload a stdout en lugar de
צור ארכיון. Esto facilita canalizar los datas a tooling mientras se mantiene el
comportamiento por defecto de imprimir el reporte המנהל.

### פריסה ותוכנית שחרור


La tabla siguiente captura el estado actual de soporte para `sorafs.sf1@1.0.0` en
רכיבים עיקריים. "הגשר" ראה את ה-Carril CARv1 + SHA-256
que requiere negociación explícita del cliente (`Accept-Chunker` + `Accept-Digest`).

| רכיב | Estado | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ סופורטדו | תוקף ידית קנוניק + כינוי, העברת דיווחים דרך `--json-out=-` y aplica la carta del registro con `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retirado | Constructor de manifest fuera de soporte; ארה"ב `iroha app sorafs toolkit pack` para empaquetado CAR/manifest y mantén `--plan=-` para revalidación determinista. |
| `sorafs_provider_advert_stub` | ⚠️ Retirado | Helper de validación offline únicamente; los ספק פרסומות deben producirse por el pipeline de publicación y validarse vía `/v1/sorafs/providers`. |
| `sorafs_fetch` (מתזמר מפתח) | ✅ סופורטדו | Lee `chunk_fetch_specs`, מטענים פעילים של capacidad `range` y ensambla salida CARv2. |
| Fixtures de SDK (Rust/Go/TS) | ✅ סופורטדו | Regeneradas vía `export_vectors`; el handle canónico aparece primero in cada list de alias y está firmado por sobres del consejo. |
| Negociación de perfiles en gateway Torii | ✅ סופורטדו | יישום השלמה של `Accept-Chunker`, כולל כותרות `Content-Chunker` y expone el bridge CARv1 solo and solicitudes de degradation explícitas. |

Despliegue de telemetria:

- **Telemetría de fetch de chunks** — la CLI de Iroha `sorafs toolkit pack` emite digests de chunk, metadata CAR y raíces PoR para ingestión en לוחות מחוונים.
- **פרסומות של ספקים** - מטענים פרסומיים כוללים מטא נתונים וכינויים; valida cobertura vía `/v1/sorafs/providers` (עמוד ej., presencia de la capacidad `range`).
- **Monitoreo de gateway** — los operadores deben reportar los pareos `Content-Chunker`/`Content-Digest` para detectar הורדת דירוג אינספרד; se espera que el uso del bridge tienda a cero antes de la deprecación.

מדיניות הביטול: una vez que se ratifique un perfil sucesor, programa una ventana de publicación dual
(documentada en la propuesta) antes de marcar `sorafs.sf1@1.0.0` como deprecado en el registro y eliminar el
גשר CARv1 de los gateways en producción.

Para inspectionar un testigo PoR específico, proporciona indices de chunk/segmento/hoja y opcionalmente
persiste la prueba a disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puedes seleccionar un perfil por id numérico (`--profile-id=1`) o por handle de registro
(`--profile=sorafs.sf1@1.0.0`); La Forma Con Handle הוא נוח עבור סקריפטים
pasen namespace/name/semver directamente desde metadatos de gobernanza.ארה"ב `--promote-profile=<handle>` para emitir un bloque JSON de metadatos (כולל todos los alias
registrados) que puede pegarse en `chunker_registry_data.rs` al promotor un nuevo perfil por defecto:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

El reporte principal (y el archivo de prueba opcional) כולל אל דיג'סט raíz, los bytes de hoja muestreados
(codificados en hex) y los digests hermanos de segmento/chunk para que los verificadores puedan rehashear
las capas de 64 KiB/4 KiB contra el valor `por_root_hex`.

Para validar una prueba existente contra un last, pasa la ruta vía
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
    "פרופיל_מזהה": 1,
    "namespace": "סורפים",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "גודל_מינימלי": 65536,
    "מטרת_גודל": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "קוד multihash": 31
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
    chunk_profile: profile_id (הירשם דרך הרישום)
    יכולות: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

Los gateways seleccionan un perfil soportado mutuamente (לפי defecto `sorafs.sf1@1.0.0`)
y reflejan la decisión vía el header de respuesta `Content-Chunker`. לוס מתבטא
embeben el perfil elegido para que los nodos במורד הזרם puedan validar el layout de chunks
sin depender de la negociación HTTP.

### Soporte CAR

retendremos una ruta de exportación CARv1+SHA-2:

* **Ruta primaria** – CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, קובץ רישום נתח כמו אריבה.
  PUEDEN exponer esta variante cuando el cliente omite `Accept-Chunker` o solicita
  `Accept-Digest: sha2-256`.

adicionales para transición pero no deben reemplazar el digest canónico.

### Conformidad

* El perfil `sorafs.sf1@1.0.0` se asigna a los fixtures públicos en
  `fixtures/sorafs_chunker` y a los corpora registrados en
  `fuzz/sorafs_chunker`. La paridad מקצה לקצה se ejerce en Rust, Go y Node
  mediante las pruebas provistas.
* `chunker_registry::lookup_by_profile` אfirma que los parametros del descriptor
  coinciden con `ChunkProfile::DEFAULT` para evitar divergencias accidentales.
* Los manifests producidos por `iroha app sorafs toolkit pack` y `sorafs_manifest_stub` כולל מטא נתונים של רישום.