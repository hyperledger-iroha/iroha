---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro fragmentador
título: Реестр профилей fragmentador SoraFS
sidebar_label: trozo de queso
descripción: ID de perfil, parámetros y planes de configuración del fragmentador SoraFS.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/chunker_registry.md`. Deje copias sincronizadas, ya que la estrella de Sphinx no se preocupará por sus especificaciones.
:::

## Troquelador de perfil Реестр SoraFS (SF-2a)

El código SoraFS permite la fragmentación de archivos sin espacio de nombres.
El perfil de usuario contiene parámetros determinados CDC, semver metadano y digest/multicodec actualizado, implementado en manifiestos y archivos CAR.

Авторы профилей должны обратиться к
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для получения требуемых метаданных, чеклиста валидации и шаблона предложения перед
отправкой новых записей. После одобрения изменения со стороны gobernabilidad следуйте
[чеклисту rollout реестра](./chunker-registry-rollout-checklist.md) y
[manifiesto del libro de jugadas en puesta en escena](./staging-manifest-playbook), чтобы продвинуть
accesorios в puesta en escena и producción.

### Perfil| Espacio de nombres | Ima | SemVer | Perfil de identificación | Мин (байты) | Цель (байты) | Макс (байты) | Máscara facial | Multihash | Alias ​​| Примечания |
|-----------|-----|--------|------------|-------------|--------------|--------------|---------------|-----------|--------|------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfiles canónicos e instalados en accesorios SF-1 |

Introduzca el código `sorafs_manifest::chunker_registry` (regulación [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Каждая запись
Utilice el `ChunkerProfileDescriptor` según las siguientes políticas:

* `namespace` – perfil de grupo lógico de grupo (por ejemplo, `sorafs`).
* `name` – читаемый человеком ярлык профиля (`sf1`, `sf1-fast`,…).
* `semver` – строка семантической версии для набора параметров.
* `profile` – фактический `ChunkProfile` (mín/objetivo/máx/máscara).
* `multihash_code` – multihash, utilizado para el resumen de la cadena (`0x1f`
  (para SoraFS по умолчанию).El manifiesto de serie del perfil es `ChunkingProfileV1`. Структура записывает метаданные реестра
(espacio de nombres, nombre, semver) вместе с сырыми CDC-параметрами и списком алиасов выше. Потребители
Si desea realizar una búsqueda en el registro `profile_id` e implementar parámetros en línea,
когда встречаются неизвестные ID; список алиасов гарантирует, что HTTP-clиенты могут продолжать
mango canónico (`namespace.name@semver`) был первой записью в `profile_aliases`, за которой

Para poder restringir las herramientas, acceda a la CLI auxiliar:

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

Estas banderas CLI contienen archivos JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`), configure `-` en el archivo de almacenamiento, qué carga útil se transmite en el conjunto de salida estándar
создания файла. Esto está obligado a pagar con herramientas según el estándar estándar.
поведения печати основного отчета.

### Матрица совместимости и PLAN LANZAMIENTO


La tabla no cambia el estado del dispositivo `sorafs.sf1@1.0.0` en los componentes clave.
"Bridge" está disponible en el canal sofisticado CARv1 + SHA-256, este es el tercer canal
клиентской переговорной фазы (`Accept-Chunker` + `Accept-Digest`).| Componente | Estado | Примечания |
|-----------|----------------|------------|
| `sorafs_manifest_chunk_store` | ✅ Поддерживается | Валидирует канонический handle + алиасы, стримит отчеты через `--json-out=-` and применяет charter реестра через `ensure_charter_compliance()`. |
| `sorafs_fetch` (organizador de desarrollo) | ✅ Поддерживается | El `chunk_fetch_specs`, carga las cargas útiles correspondientes al `range` y carga el CARv2. |
| Accesorios SDK (Rust/Go/TS) | ✅ Поддерживается | Перегенерированы через `export_vectors`; канонический mango идет первым в каждом списке алиасов и подписан sobres del consejo. |
| Perfil de negociación en la puerta de enlace Torii | ✅ Поддерживается | Implemente la gramática `Accept-Chunker`, agregue `Content-Chunker` y carv1 bridge para reducir la degradación. |

Развертывание телеметрии:

- **Телеметрия fetch чанков** — CLI Iroha `sorafs toolkit pack` emite resúmenes de чанков, метаданные CAR y корни PoR para la ingestión en los tableros.
- **Anuncios de proveedores**: anuncios de carga útil que incluyen capacidades y alias de metadatos; Compruebe la capacidad de `/v2/sorafs/providers` (por ejemplo, la capacidad de `range`).
- **Monitorización de puerta de enlace**: los operadores que utilizan los pares `Content-Chunker`/`Content-Digest`, que bloquean nuevas degradaciones; ожидается, что использование bridge снизится до нуля до депрекации.Avisos políticos: как только утвержден профиль-преемник, запланируйте окно двойной публикации
Puente CARv1 y puertas de enlace de producción.

Para comprobar la coherencia de la imagen PoR, utilice índices de trozos/segmentos/hojas y nuevos elementos
сохраните prueba en el disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puede cambiar el perfil de su ID (`--profile-id=1`) o su identificador
(`--profile=sorafs.sf1@1.0.0`); mango de formato удобен для скриптов, которые
Utilice namespace/name/semver para crear metadatos de gobernanza.

Utilice `--promote-profile=<handle>` para crear un bloque de metadatos JSON (combinado con
зарегистрированные алиасы), который можно вставить в `chunker_registry_data.rs` при
продвижении нового профиля по умолчанию:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (и необязательный файлproof) включает корневой digest, байты выбранного hoja
(en hexadecimal) y resúmenes de hermanos сегмента/чанка, чтобы верификаторы могли пересчитать hash слоев
64 KiB/4 KiB por defecto `por_root_hex`.

Чтобы валидировать существующий относительно payload, передайте путь через
`--por-proof-verify` (CLI para `"por_proof_verified": true`, código de pantalla
совпадает с вычисленным корнем):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Este paquete de paquetes utiliza `--por-sample=<count>` y при желании задайте seed/putt вывода.
CLI garantiza la configuración predeterminada (`splitmix64` seeded) y el uso continuo,
когда он превышает число доступных листьев:```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub отражает те же данные, что удобно при скриптинге выбора `--chunker-profile-id`
в пайплайнах. Оба chunk store CLI также принимают канонический формат handle
(`--profile=sorafs.sf1@1.0.0`), поэтому build-скрипты могут избежать жесткого
хардкода числовых ID:

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

Поле `handle` (`namespace.name@semver`) совпадает с тем, что CLIs принимают через
`--profile=…`, поэтому его можно безопасно копировать в автоматизацию.

### Согласование chunker

Gateways и клиенты объявляют поддерживаемые профили через provider adverts:

```
ProveedorAdvertBodyV1 {
    ...
    chunk_profile: perfil_id (sin registro)
    capacidades: [...]
}
```

Планирование multi-source чанков объявляется через capability `range`. CLI принимает ее с
`--capability=range[:streams]`, где опциональный числовой суффикс кодирует предпочтительную
параллельность range-fetch у провайдера (например, `--capability=range:64` объявляет бюджет 64 streams).
Когда суффикс отсутствует, потребители возвращаются к общему hint `max_streams`, опубликованному в другом
месте advert.

При запросе CAR-данных клиенты должны отправлять заголовок `Accept-Chunker`, перечисляя кортежи
`(namespace, name, semver)` в порядке предпочтения:

```

Gateways выбирают взаимно поддерживаемый профиль (по умолчанию `sorafs.sf1@1.0.0`) и отражают
решение в ответном заголовке `Content-Chunker`. Manifiestos встраивают выбранный профиль, чтобы
узлы downstream pueden validar la conexión según las conexiones HTTP.

### Совместимость COCHE

сохраняется путь экспорта CARv1+SHA-2:

* **Основной путь** – CARv2, resumen de carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, el fragmento de perfil se bloqueó como tal.
  Puede cambiar esta variante, ya que el cliente envía `Accept-Chunker` o descarga
  `Accept-Digest: sha2-256`.

заголовки для совместимости, но не должны заменять канонический digest.

### Соответствие* Perfil `sorafs.sf1@1.0.0` соответствует публичным accesorios en
  `fixtures/sorafs_chunker` y corpus, зарегистрированным в
  `fuzz/sorafs_chunker`. Aplicación de integración de extremo a extremo en Rust, Go y Node
  через предоставленные тесты.
* `chunker_registry::lookup_by_profile` muestra qué parámetros descriptores
  совпадают с `ChunkProfile::DEFAULT`, чтобы защититься от случайной дивергенции.
* Manifiestos, производимые `iroha app sorafs toolkit pack` и `sorafs_manifest_stub`, включают метаданные реестра.