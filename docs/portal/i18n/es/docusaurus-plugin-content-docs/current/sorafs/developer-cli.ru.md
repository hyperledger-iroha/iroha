---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: Книга рецептов CLI SoraFS
sidebar_label: Opciones CLI
descripción: Практический разбор по задачам для консолидированной поверхности `sorafs_cli`.
---

:::nota Канонический источник
:::

Consola de almacenamiento `sorafs_cli` (caja de almacenamiento `sorafs_car` con función adicional `cli`) раскрывает каждый шаг, необходимый для подготовки артефактов SoraFS. Utilice este libro de cocina para saber cuáles son los flujos de trabajo más importantes; Utilice el programador de canalizaciones y el organizador de runbooks para el contacto operativo.

## Cargas útiles de Упаковка

Utilice `car pack` para determinar los componentes del vehículo y los planos. El comando automático utiliza el fragmentador SF-1 y no tiene mango.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Troceador de mango estándar: `sorafs.sf1@1.0.0`.
- Todos los directorios de códigos de registro de las sumas de comprobación están instalados de forma estable en las plataformas.
- Resúmenes de carga útil integrados en formato JSON, fragmentos de metadatos y CID actualizados, registros y orquestadores desplegados.

## Сборка se manifiesta

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Las opciones `--pin-*` se adaptan a los polos `PinPolicy` y `sorafs_manifest::ManifestBuilder`.
- Utilice `--chunk-plan`, o haga clic en el resumen SHA3 de CLI para el fragmento antes de la operación; иначе он использует digest из resumen.
- JSON-вывод отражает Norito payload для удобных diffs при ревью.## Подписание se manifiesta без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Principales tokens en línea, operaciones permanentes o archivos de archivos.
- El origen del archivo metadano (`token_source`, `token_hash_hex`, fragmento de resumen) no está disponible en JWT sin formato, y no es necesario `--include-token=true`.
- Удобно для CI: объедините с GitHub Actions OIDC, установив `--identity-token-provider=github-actions`.

## Отправка manifiesta en Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Выполняет Norito-декодирование alias pruebas y проверяет соответствие resumen manifiesto de POST en Torii.
- El fragmento de resumen SHA3 es un plan que evita que se produzcan ataques no deseados.
- Restablezca la configuración del estado HTTP, los encabezados y las cargas útiles para la auditoría posterior.

## Проверка содержимого CAR y pruebas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Realice un PoR y complete los resúmenes de carga útil con el manifiesto de respuesta.
- Фиксирует количества и идентификаторы, нужные при отправке pruebas de replicación en la gobernanza.

## Pruebas de televisión de Potokova

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Генерирует NDJSON элементы для каждого переданного pruebas (отключите replay через `--emit-events=false`).
- Agregue varios tipos de aplicaciones/sistemas, programas de latencia y archivos adjuntos en resumen JSON, paneles de control que pueden crear gráficos entre sí. чтения логов.
- Завершает работу с ненулевым кодом, когда gateway сообщает об ошибках или локальная проверка PoR (через `--por-root-hex`) отклоняет pruebas. Utilice varias veces `--max-failures` e `--max-verification-failures` para repetirlas.
- Сегодня поддерживается PoR; PDP y PoTR persiguieron el sobre después del SF-13/SF-14.
- `--governance-evidence-dir` muestra respuestas impresas, metadatos (marca de tiempo, versión CLI, puerta de enlace URL, resumen del manifiesto) y copias del manifiesto en el archivo original. Directorio, estos paquetes de gobernanza pueden archivar el flujo de prueba sin necesidad de autorización.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — исчерпывающая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — pruebas de televisores y tablero de instrumentos Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — подробный разбор fragmentación, сборки manifest и обработки CAR.