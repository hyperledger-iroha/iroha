---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Чанкинг SoraFS → Manifestantes de tubería

Este material está disponible en inicio rápido y se describe en varios papeles, según sea necesario.
байты в манифесты Norito, пригодные для Pin Registry SoraFS. Текст адаптирован из
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consulte este documento sobre las características técnicas y periódicas del usuario.

## 1. Детерминированный чанкинг

SoraFS utiliza el perfil SF-1 (`sorafs.sf1@1.0.0`): роллинг-хэш, вдохновленный FastCDC, с
Miniaturas de 64 KiB, máximas de 256 KiB, máximas de 512 KiB y más de 512 KiB
`0x0000ffff`. El perfil registrado en `sorafs_manifest::chunker_registry`.

### Ayudantes de óxido

- `sorafs_car::CarBuildPlan::single_file` – Выдает смещения, длины и BLAKE3-дайджесты чанков
  при подготовке метаданных CAR.
- `sorafs_car::ChunkStore` – Стримит payloads, сохраняет метаданные чанков и выводит дерево
  выборки Prueba de recuperabilidad (PoR) 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests`: ayudante de la biblioteca, disponible en la CLI.

### Instrumentos CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

El formato JSON permite almacenar datos, datos y datos personales. Сохраняйте план при сборке
Manifestantes o instrucciones específicas del orquestador.

### Свидетели PoR

`ChunkStore` anterior a `--por-proof=<chunk>:<segment>:<leaf>` y `--por-sample=<count>`,
чтобы аудиторы могли запрашивать детерминированные наборы свидетелей. Сочетайте эти флаги с
`--por-proof-out` o `--por-sample-out`, escriben JSON.

## 2. Manifiesto abierto`ManifestBuilder` объединяет метаданные чанков с вложениями gobernanza:

- Корневой CID (dag-cbor) y коммитменты CAR.
- Доказательства alias y reclamaciones возможностей провайдеров.
- Opciones avanzadas y metadanas opcionales (por ejemplo, ID de compilación).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важные выходные данные:

- `payload.manifest` – Norito-кодированные байты манифеста.
- `payload.report.json` – Сводка для людей/автоматизации, включая `chunk_fetch_specs`,
  `payload_digest_hex`, дайджесты CAR y метаданные alias.
- `payload.manifest_signatures.json` – Конверт, содержащий BLAKE3-дайджест манифеста,
  SHA3-дайджест плана чанков и отсортированные подписи Ed25519.

Utilice `--manifest-signatures-in` para comprobar las conversiones de vídeo que se pueden realizar antes
перезаписью, и `--chunker-profile-id` или `--chunker-profile=<handle>` para la ficción
реестра.

## 3. Publicación y fijación1. **Отправка в gobernancia** – Передайте дайджест манифеста и конверт подписей совету, чтобы
   pin мог быть принят. Todos los auditores utilizan el plan SHA3-дайджест плана чанков рядом с
   дайджестом манифеста.
2. **Пиннинг payloads** – Загрузите архив CAR (и опциональный индекс CAR), указанный в
   манифесте, en Registro de PIN. Tenga en cuenta que el manifiesto y el coche utilizan Odín y el CID de Cornevo.
3. **Cerrar televisores**: seleccione archivos JSON, archivos PoR y métricas para buscar en
   релизных артефактах. Эти записи питают операторские дашборды и помогают воспроизводить
   проблемы без загрузки больших cargas útiles.

## 4. Симуляция выборки от нескольких провайдеров

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` se coloca en paralelo con el controlador (`#4`).
- `@<weight>` настраивает смещение планирования; по умолчанию 1.
- `--max-peers=<n>` ограничивает число провайдеров, запланированных на запуск, когда
  обнаружение возвращает больше кандидатов, чем нужно.
- `--expect-payload-digest` e `--expect-payload-len` se ajustan a estos porches.
- `--provider-advert=name=advert.to` проверяет возможности провайдера перед использованием
  en simulaciones.
- `--retry-budget=<n>` переопределяет число повторов на чанк (по умолчанию: 3), чтобы CI
  быстрее выявляла регрессии при тестировании отказов.

`fetch_report.json` incluye métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
y т. д.), подходящие для CI-ассертов и наблюдаемости.

## 5. Обновления реестра и gobernancia

Algunos ejemplos de nuevos perfiles de fragmentación:

1. Introduzca el descriptor en `sorafs_manifest::chunker_registry_data`.
2. Обновите `docs/source/sorafs/chunker_registry.md` и связанные чартеры.
3. Conecte los componentes (`export_vectors`) y elimine los manifiestos.
4. Отправьте отчет о соответствии чартеру с подписями gobernabilidad.

Автоматизации следует предпочитать канонические handles (`namespace.name@semver`) и
возвращаться к числовым ID только при необходимости обратной совместимости.