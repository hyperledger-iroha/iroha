---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de orquestador
título: Runbook по эксплуатации оркестратора SoraFS
sidebar_label: Operador de Runbook
descripción: Posibilidad de funcionamiento de múltiples operadores, monitorización y operación.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Si necesita copias sincronizadas de los documentos de Sphinx, no necesita ayuda para migratorios.
:::

Este runbook proporciona SRE para archivos, configuraciones y extracciones de múltiples operadores de búsqueda. Он дополняет руководство разработчика процедурами, рассчитанными на прод-роллауты, включая потапное включение и занесение пиров в чёрный список.

> **См. Etiqueta:** [Runbook по мульти-источниковому rollout](./multi-source-rollout.md) посвящён волнам развёртывания на уровне флота и экстренному отклонению провайдеров. Используйте его для координации gobernancia / puesta en escena, а этот документ — для повседневной эксплуатации оркестратора.

## 1. Lista de verificación anterior

1. **Собрать входные данные от провайдеров**
   - Последние анонсы провайдеров (`ProviderAdvertV1`) y снимок телеметрии для целевого флота.
   - La carga útil del plan (`plan.json`), según el manifiesto del test.
2. ** Сформировать детерминированный marcador **

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- Tenga en cuenta que `artifacts/scoreboard.json` está conectado al producto del producto como `eligible`.
   - Архивируйте JSON-сводку вместе со marcador; Los auditores que operan en el sector privado retiran las pruebas de certificación de la empresa.
3. **Ejecución en seco de accesorios** — Utilice el comando de accesorios públicos `docs/examples/sorafs_ci_sample/`, que esté conectado o que sea un orquestador binario. ожидаемой версии, antes de cargar las cargas útiles del producto.

## 2. Implementación del proceso de publicación

1. **Канарейка (≤2 провайдера)**
   - Mantenga el marcador y desbloquee el `--max-peers=2`, para que el orquestador no pueda funcionar correctamente.
   - Monitorear:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Prodolжайте, когда доля ретраев держится ниже 1% для полного fetch manifestа and ни один провайдер не накапливает ошибок.
2. **Этап разгона (50% de descuento)**
   - Utilice `--max-peers` y conecte los televisores más pequeños.
   - Coloque el teclado en `--provider-metrics-out` y `--chunk-receipts-out`. Храните артефакты ≥7 días.
3. **Implementación avanzada**
   - Уберите `--max-peers` (или установите его на полный набор elegible).
   - Utilice el operador de la implementación de los clientes: actualice el marcador electrónico y la configuración JSON del sistema de actualización. configuración.
   - Retire el tablero, coloque `sorafs_orchestrator_fetch_duration_ms` p95/p99 y los registros en la región.## 3. Блокировка и усиление пиров

Si se anula la configuración política en la CLI, se eliminarán los problemas de la gobernanza actual.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` elimina el alias de la unidad en la sesión técnica.
- `--boost-provider=<alias>=<weight>` повышает вес провайдера в планировщике. Значения добавляются к нормализованному весу marcador и применяются только к локальному запуску.
- Evite anulaciones de etiquetas accidentales y archivos JSON, para que los comandos no autorizados puedan modificar la configuración. после устранения первопричины.

Для постоянных изменений обновите исходную телеметрию (пометьте нарушителя как penalizado) o ли пересоздайте advert с обновлёнными бюджетами потоков до очистки Anulación de CLI.

## 4. Разбор сбоев

Когда buscar падает:1. Antes de empezar a usar artefactos de alta calidad:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Pruebe `session.summary.json` en человекочитаемую строку ошибки:
   - `no providers were supplied` → проверьте пути к провайдерам и объявления.
   - `retry budget exhausted ...` → увеличьте `--retry-budget` или сключите нестабильных пиров.
   - `no compatible providers available ...` → проверьте метаданные диапазонных возможностей провайдера-нарушителя.
3. Coloque el medidor en `sorafs_orchestrator_provider_failures_total` y coloque el billete en su lugar, o un índice de medición.
4. Utilice la línea de búsqueda `--scoreboard-json` y un televisor desmontado que esté determinado por el dispositivo.

## 5. Revertir

Чтобы откатить оркестратора de implementación:

1. Configure la configuración con `--max-peers=1` (funciones de múltiples planes de instalación) o conecte a los clientes a устаревший одноисточниковый buscar-путь.
2. Уберите любые anula `--boost-provider`, чтобы marcador вернулся к нейтральному весу.
3. Prodolжайте собирать метрики оркестратора минмимум сутки, чтобы подтвердить отсутствие оставшихся fetch-operaций.

Las implementaciones disciplinarias de artefactos y dispositivos garantizan múltiples aplicaciones оркестратора на разнородных флотах провайдеров при сохранении требований по наблюдаемости и аудиту.