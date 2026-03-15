---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operaciones Registro de PIN
sidebar_label: Operaciones Registro de PIN
descripción: Registro de PIN de monitoreo y clasificación SoraFS y replicaciones de SLA métricas.
---

:::nota Канонический источник
Utilice `docs/source/sorafs/runbooks/pin_registry_ops.md`. Держите обе версии синхронизированными, пока наследственная документация Sphinx не будет выведена из эксплуатации.
:::

## Objeto

Esta descripción de runbook permite monitorear y controlar el registro de pines de clasificación SoraFS y su propia configuración de servicio (SLA) para replicaciones. Las métricas se introducen en `iroha_torii` y se exportan entre Prometheus y el espacio de nombres `torii_sorafs_*`. Torii opera el registro de configuración cada 30 segundos en el teléfono, los paneles de control se activan actualmente cuando no está conectado Los operadores no deben controlar los puntos finales `/v1/sorafs/pin/*`. Importe el tablero de instrumentos (`docs/source/grafana_sorafs_pin_registry.json`) para el diseño del automóvil Grafana, cuyo contenido no está disponible.

## Справочник метрик| Métrica | Etiquetas | Descripción |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | La innovación se manifiesta en cadena después del ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Количество активных alias manifiestos, записанных в registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | La acumulación de pedidos se ha realizado de manera segmentaria según el estado. |
| `torii_sorafs_replication_backlog_total` | — | Удобный calibre, отражающий `pending` заказы. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA: `met` считает заказы, завершенные в срок, `missed` агрегирует поздние завершения + истечения, `pending` отражает незавершенные заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегированная латентность завершения (эпохи между выпуском и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Окна запаса для незавершенных заказов (fecha límite minус эпоха выдачи). |

Cuando los medidores se activan en una instantánea de extracción, los tableros de instrumentos se utilizan en el bloque `1m` o en el bloque.

## Panel de control GrafanaPanel de control JSON que se utiliza como panel de control para los operadores. Запросы перечислены ниже для быстрого справочника, если вы предпочитаете строить собственные графики.

1. **Manifiestos del ciclo Жизненный** – `torii_sorafs_registry_manifests_total` (grupo de personas de `status`).
2. **alias del catálogo** – `torii_sorafs_registry_aliases_total`.
3. **Очередь заказов по статусу** – `torii_sorafs_registry_orders_total` (grupo según `status`).
4. **Backlog vs истекшие заказы** – объединяет `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для выявления насыщения.
5. **Utilización de SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Латентность vs запас до fecha límite** – utilice `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilice la transmisión Grafana, para activar la configuración `min_over_time`, ya que no hay ningún problema предел запаса, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (tarifa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Пороговые значения алертов- **Успех SLA  0**
  - Porción: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Diseño: Проверить manifiestos de gobernanza, чтобы подтвердить proveedores de abandono.
- **p95 завершения > средний запас до fecha límite**
  - Porción: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Убедиться, что proveedores завершают до fecha límite; рассмотреть перераспределение.

### Primera versión Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации SoraFS истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## Clasificación del flujo de trabajo1. **Определить причину**
   - Если пропуски SLA растут, а backlog остается низким, сосредоточьтесь на производительности proveedores (сбои PoR, поздние завершения).
   - Если trabajo pendiente растет при стабильных пропусках, проверьте admisión (`/v1/sorafs/pin/*`), чтобы подтвердить manifiestos, ожидающие утверждения совета.
2. **Provertir proveedores de estado**
   - Introduzca `iroha app sorafs providers list` y compruebe si hay varias réplicas.
   - Proveer los medidores `torii_sorafs_capacity_*`, que permiten GiB aprovisionado y PoR.
3. **Preparar réplicas**
   - Выпустите новые заказы через `sorafs_manifest_stub capacity replication-order`, когда запас backlog (`stat="avg"`) опустится ниже 5 эпох (упаковка manifest/CAR использует `iroha app sorafs toolkit pack`).
   - Уведомите gobernancia, если alias не имеют активных vinculantes manifiestos (неожиданное падение `torii_sorafs_registry_aliases_total`).
4. **Задокументировать результат**
   - Complete los incidentes en la operación diaria SoraFS con marcas de tiempo y manifiestos de resumen.
   - Desactive este runbook y agregue nuevas opciones o paneles de control.

## План развертывания

Continúe con este proceso de configuración de archivos o aplicaciones de caché de alias en los siguientes productos:1. **Подготовить конфигурацию**
   - Обновите `torii.sorafs_alias_cache` en `iroha_config` (usuario -> actual) con configuraciones TTL y funciones de agradecimiento: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Значения по умолчанию соответствуют политике в `docs/source/sorafs_alias_policy.md`.
   - Para los SDK que se ejecutan en las configuraciones de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en enlaces Rust / NAPI / Python), el cliente de aplicación de la aplicación puerta de enlace.
2. **Ejecución en seco y puesta en escena**
   - Modifique la configuración de configuración del grupo de preparación, según la topología del producto.
   - El mensaje `cargo xtask sorafs-pin-fixtures` indica qué dispositivos de alias canónicos están disponibles para la decoración y el proceso de ida y vuelta; любое несовпадение означает corriente arriba, который нужно устранить первым.
   - Progonia los puntos finales `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` con pruebas sintéticas, que incluyen ventanas frescas, de actualización, caducadas y duramente caducadas. Proveer códigos HTTP, encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y archivos JSON de este runbook exclusivo.
3. **Включить в продакшене**
   - Realice nuevas configuraciones en la configuración estándar. Si utiliza Torii, instale gateways/servicios SDK después de implementar nuevas políticas en el entorno logístico.- Importe `docs/source/grafana_sorafs_pin_registry.json` en Grafana (o elimine los paneles de control actualizados) y abra los paneles de actualización de caché de alias en el NOC externo.
4. **Проверка после развертывания**
   - Monitoree `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Las opciones en las teclas `error`/`expired` se correlacionan con las opciones de actualización; No es necesario que los operadores proporcionen alias de prueba y proveedores compatibles antes del proceso.
   - Tenga en cuenta que los logotipos de los clientes utilizan las políticas de resolución (SDK que se pueden utilizar a prueba de errores, instalados o instalados a prueba de errores). El cliente habitual del cliente nunca configura ninguna configuración.
5. **Retroceso**
   - Si el alias del usuario se activa y se actualiza automáticamente, se elimina la política, se activan `refresh_window` e `positive_ttl` en la configuración, затем выполните повторный деплой. Оставьте `hard_expiry` неизменным, чтобы действительно устаревшие продолжали отклоняться.
   - Verifique la configuración anterior, instale la instantánea `iroha_config` o coloque el televisor Cuando se utiliza el alias `error`, se elimina el error de generación de alias.

## Materiales nuevos

- `docs/source/sorafs/pin_registry_plan.md` — дорожная карта реализации и контекст управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — Trabajador de almacenamiento de operaciones, дополняет этот registro de libro de jugadas.