---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: Плейбук операций SoraFS
sidebar_label: operación completa
descripción: Руководства по реагированию на инциденты и процедуры хаос-дриллов для операторов SoraFS.
---

:::nota Канонический источник
Esta página está disponible en `docs/source/sorafs_ops_playbook.md`. Deje copias sincronizadas de los documentos de Sphinx que no necesitan ayuda para migrar.
:::

## Ключевые ссылки

- Activaciones activas: active los paneles Grafana en `dashboards/grafana/` y active las alertas Prometheus en `dashboards/alerts/`.
- Métrica del catálogo: `docs/source/sorafs_observability_plan.md`.
- Telemetros de control: `docs/source/sorafs_orchestrator_plan.md`.

## Матрица эскалации

| Prioridad | Primeros disparadores | Основной de guardia | Reservas | Примечания |
|-----------|-------------------|-----------------|--------|------------|
| P1 | Puerta de enlace global, aumento de PoR > 5% (15 min), pedidos pendientes de respuesta cada 10 min | Almacenamiento SRE | Observabilidad TL | Подключите совет по gobernancia, если влияние превышает 30 minutos. |
| P2 | Configuración regional de SLO para puerta de enlace de latencia, reintentos completos del operador sin conexión a SLA | Observabilidad TL | Almacenamiento SRE | Implementación mejorada, no hay nuevos manifiestos. |
| P3 | Некритичные алерты (se manifiesta estancamiento, емкость 80–90%) | Triaje de admisión | Gremio de operaciones | Consulte con el propietario de este lugar. |## Отключение gateway / деградация доступности

**Обнаружение**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tablero: `dashboards/grafana/sorafs_gateway_overview.json`.

**Nuevo diseño**

1. Подтвердите масштаб (один провайдер или весь флот) по по панели request-rate.
2. Coloque la marca Torii en здоровых провайдеров (multiproveedor), coloque `sorafs_gateway_route_weights` en la configuración de operaciones (`docs/source/sorafs_gateway_self_cert.md`).
3. Si está conectado a un proveedor, active la “búsqueda directa” alternativa para el cliente CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triaje**

- Проверьте утилизацию stream token относительно `sorafs_gateway_stream_token_limit`.
- Posibilitar el registro de entrada en TLS o los controles de admisión.
- Introduzca `scripts/telemetry/run_schema_diff.sh`, para saber qué puerta de enlace deportiva tiene una versión actualizada.

**Varias soluciones**

- Перезапускайте только затронутый процесс gateway; избегайте перезапуска всего кластера, если не падают несколько провайдеров.
- Actualmente se limita el token de flujo al 10-15%, o se puede utilizar.
- Повторно выполните self-cert (`scripts/sorafs_gateway_self_cert.sh`) после стабилизации.

**Después del incidente**

- Forme el terminal P1 con el botón `docs/source/sorafs/postmortem_template.md`.
- Siga el procedimiento de eliminación del caos y aplique remedios según el dispositivo adecuado.

## Prueba de Всплеск отказов (PoR / PoTR)

**Обнаружение**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tablero: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetría: `torii_sorafs_proof_stream_events_total` y la conexión `sorafs.fetch.error` con `provider_reason=corrupt_proof`.**Nuevo diseño**

1. Заморозьте прием новых manifests, пометив реестр manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Уведомите Gobernanza о приостановке стимулов для затронутых провайдеров.

**Triaje**

- Проверьте глубину очереди PoR Challenge относительно `sorafs_node_replication_backlog_total`.
- Validación de la prueba de prueba de tuberías (`crates/sorafs_node/src/potr.rs`) para los despliegues posteriores.
- Seleccione la versión del firmware proporcionada por el operador del sistema.

**Varias soluciones**

- Introduzca las repeticiones de PoR con el manifiesto `sorafs_cli proof stream`.
- Если pruebas стабильно падают, исключите провайдера из активного набора через обновление реестра gobernancia y принудительное обновление marcadores orquestador.

**Después del incidente**

- Запустите сценарий хаос-дрилла PoR до следующего продакшен-деплоя.
- Guarde las cosas en el sitio después de la muerte y elimine la lista de verificación de calificaciones de los proveedores.

## Задержка репликации / рост trabajo pendiente

**Обнаружение**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importar
  `dashboards/alerts/sorafs_capacity_rules.yml` y выполните
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Para la promoción, Alertmanager contiene documentos documentados.
- Tablero: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Nuevo diseño**

1. Evite la acumulación de archivos (o провайдер или весь флот) y приостановите несущественные задачи репликации.
2. Si el trabajo pendiente es local, siempre es necesario realizar nuevas tareas en las alternativas de respuesta del programador.**Triaje**

- Evite los reintentos del operador del televisor, ya que puede eliminar el trabajo pendiente.
- Observe, что у целей хранения достаточно headroom (`sorafs_node_capacity_utilisation_percent`).
- Mejora de las configuraciones de configuración (perfil de fragmentos actualizado, pruebas de cadencia).

**Varias soluciones**

- Introduzca `sorafs_cli` con la opción `--rebalance` para ampliar el contenido.
- Масштабируйте trabajadores de replicación горизонтально для затронутого провайдера.
- Inicie la actualización del manifiesto para la configuración TTL.

**Después del incidente**

- Запланируйте taladro de capacidad, фокусированный на сбоях из-за насыщения провайдеров.
- Retire la documentación de réplicas de SLA en `docs/source/sorafs_node_client_protocol.md`.

## Периодичность хаос-дриллов

- **Ежеквартально**: совмещенная симуляция остановки gateway + reintento del operador de tormenta.
- **Два раза в год**: инъекция сбоев PoR/PoTR на двух провайдерах с восстановлением.
- **Ежемесячная spot-проверка**: réplicas de anuncios de escenarios de manifiestos de puesta en escena implementados.
- Instale ejercicios en el registro de runbook (`ops/drill-log.md`) como:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Pruebe el registro antes de las confirmaciones:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilice `--status scheduled` para taladros de mano, `pass`/`fail` para taladros de banco y `follow-up`, если остались открытые действия.
- Asegúrese de seleccionar el número `--log` para el funcionamiento en seco o para el funcionamiento automático; Sin ningún script, el producto está modificado `ops/drill-log.md`.

## Шаблон постмортема

Utilice `docs/source/sorafs/postmortem_template.md` para el incidente P1/P2 y para la retrospectiva de la cámara. Шаблон покрывает таймлайн, количественную оценку влияния, факторы, коректирующие действия и задачи последующей validaciones.