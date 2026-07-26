---
lang: es
direction: ltr
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está escrita `docs/source/da/replication_policy.md`. Держите обе версии
:::

# Disponibilidad de datos de réplicas políticas (DA-4)

_Статус: В работе — Владельцы: Core Protocol WG / Equipo de almacenamiento / SRE_

El tipo de canalización de ingesta de DA es el primero en determinar la retención de células del archivo
blob de clase, descrito en `roadmap.md` (flujo de trabajo DA-4). Torii отказывается
сохранять sobres de retención, предоставленные llamante, если они не совпадают с
política nacional, garantía, что каждый validador/узел хранения удерживает
необходимое число эпох и реплик без опоры на намерения отправителя.

## Política para todos los públicos

| Класс blob | Retención en caliente | Retención de frío | Требуемые реплики | Clase хранения | Etiqueta de gobernanza |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (все остальные классы)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |Estas son las versiones en `torii.da_ingest.replication_policy` y las primeras
всем `/v1/da/ingest` отправкам. Torii переписывает se manifiesta с примененным
retención de perfiles y выдает предупреждение, если personas que llaman передают несоответствующие
значения, чтобы операторы могли выявлять устаревшие SDK.

### Clases de descarga Taikai

Manifiestos de enrutamiento Taikai (`taikai.trm`) объявляют `availability_class`
(`hot`, `warm`, o `cold`). Torii proporciona una política de fragmentación,
чтобы операторы могли масштабировать число replik по stream без редактирования
tablas globales. Valores predeterminados:

| Clases disponibles | Retención en caliente | Retención de frío | Требуемые реплики | Clase хранения | Etiqueta de gobernanza |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Если подсказок нет, используется `hot`, чтобы transmisión en vivo удерживали самый
строгий профиль. Переопределяйте valores predeterminados через
`torii.da_ingest.replication_policy.taikai_availability`, если сеть использует
другие цели.

## ConfiguraciónPolítica de entrada `torii.da_ingest.replication_policy` y anterior
*predeterminado* шаблон плюс массив anulación для каждого класса. Clases de identificación
registros y programas `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, либо `custom:<u16>` для расширений, одобренных управлением.
Las clases de clasificación son `hot`, `warm` o `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Instale un bloque sin configuración para configurar los valores predeterminados. Чтобы ужесточить
класс, обновите соответствующий anulación; чтобы изменить базу для новых классов,
отредактируйте `default_retention`.

Clases de disponibilidad de Taikai можно переопределять отдельно через
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Aplicación de la ley semántica

- Torii coloca el nombre de usuario `RetentionPolicy` en el perfil de fábrica
  Antes de la fragmentación o del manifiesto de выпуском.
- Manifiestos previos, которые декларируют несовпадающий perfil de retención,
  отклоняются с `400 schema mismatch`, чтобы устаревшие клиенты не могли ослабить
  contrato.
- Каждое событие anular la lógica (`blob_class`, política de anulación vs.
  ожидаемая), чтобы выявлять personas que llaman no conformes во время implementación.

Смотрите [Plan de ingesta de disponibilidad de datos](ingest-plan.md) (Lista de verificación de validación) para
обновленного puerta, покрывающего aplicación de la retención.

## Workflow повторной репликации (seguimiento DA-4)Retención de ejecución — лишь первый шаг. Los operadores están transmitiendo en vivo
manifiestos y órdenes de replicación остаются согласованными с настроенной политикой,
чтобы SoraFS мог автоматически volver a replicar blobs no deseados.

1. **Следите за drift.** Torii пишет
   Código `overriding DA retention policy to match configured network baseline`
   la persona que llama отправляет устаревшие retención значения. Сопоставляйте этот лог с
   Telemetro `torii_sorafs_replication_*`, чтобы обнаружить déficits respuesta
   или задержанные redistribuciones.
2. **Consulte la intención y las réplicas en vivo.** Utilice un nuevo asistente de auditoría:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando de configuración `torii.da_ingest.replication_policy` es una configuración,
   Manifestación de código binario (JSON o Norito) y soporte opcional
   cargas útiles `ReplicationOrderV1` en el resumen del manifiesto. Estas son algunas de las situaciones:

   - `policy_mismatch` - manifiesto de perfil de retención расходится с принудительным
     профилем (такого не должно быть, если Torii настроен корректно).
   - `replica_shortfall` - orden de replicación en vivo запрашивает меньше реплик, чем
     `RetentionPolicy.required_replicas`, или выдает меньше назначений, чем цель.Ненулевой код выхода означает активный déficit, чтобы CI/on-call автоматизация
   могла немедленно пейджить. Utilice JSON para crear un paquete
   `docs/examples/da_manifest_review_template.md` para el parlamento español.
3. **Запустите re-replicación.** Si la auditoría detecta un déficit, выпустит
   новый `ReplicationOrderV1` через инструменты управления, описанные в
   [Mercado de capacidad de almacenamiento SoraFS](../sorafs/storage-capacity-marketplace.md),
   и повторяйте аудит, пока набор реплик не сойдется. Для anulaciones de emergencia
   La CLI conectada a `iroha app da prove-availability`, puede contener SRE
   на тот же digest y evidencia del PDP.

Cobertura de regresión incluida en `integration_tests/tests/da/replication_policy.rs`;
suite отправляет несовпадающую политику retención en `/v1/da/ingest` y проверяет,
что полученный manifest показывает принудительный профиль, una persona que llama sin intención.