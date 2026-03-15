---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Despliegue de anuncios y anuncios actualizados SoraFS"
---

> Adaptado a [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План rollout y совместимости anuncio провайдеров SoraFS

Este plan de coordinación del período de publicidad permisiva провайдеров к полностью
управляемой поверхности `ProviderAdvertV1`, необходимой для multi-source выдачи
trozos. Sobre la base de tres entregables:

- **Руководство оператора.** Пошаговые действия, которые провайдеры хранения должны
  выполнить до включения каждого puerta.
- **Покрытие телеметрией.** Paneles y alertas, которые Observabilidad y operaciones используют,
  чтобы подтвердить, что сеть принимает только совместимые anuncios.
  SDK y herramientas pueden planificar aplicaciones.

Rollout согласован с вехами SF-2b/2c в
[hoja de ruta миграции SoraFS](./migration-roadmap) и предполагает, что policy допуска в
[política de admisión de proveedores](./provider-admission-policy) уже действует.

## Таймлайн фаз

| Faza | Окно (цель) | Поведение | Operadores de distrito | Focos de atención |
|-------|-----------------|-----------|------------------|-------------------|

## Чеклист оператора1. **Inventar anuncios.** Buscar anuncios públicos y anuncios publicitarios:
   - Coloque la envolvente de gobierno (`defaults/nexus/sorafs_admission/...` или Production-эквивалент).
   - Anuncio `profile_id` y `profile_aliases`.
   - Capacidades de navegación (ожидается как minимум `torii_gateway` y `chunk_range_fetch`).
   - Bandera `allow_unknown_capabilities` (según el TLV reservado por el proveedor).
2. **Registro de las herramientas del proveedor.**
   - Пересоберите payload через anuncio del editor, убедившись в:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con el original `max_span`
     - `allow_unknown_capabilities=<true|false>` para grasa TLV
   - Proverte через `/v2/sorafs/providers` y `sorafs_fetch`; предупреждения о
     неизвестных capacidades нужно триажить.
3. **Preparación para múltiples fuentes.**
   - Выполните `sorafs_fetch` con `--provider-advert=<path>`; CLI теперь падает,
     когда отсутствует `chunk_range_fetch`, y печатает предупреждения о
     проигнорированных неизвестных capacidades. Insertar archivos JSON y
     архивируйте его с registros de operaciones.
4. **Подготовка продлений.**
   - Отправьте `ProviderAdmissionRenewalV1` sobres aproximadamente 30 días antes
     aplicación de la puerta de enlace (R2). Продления должны сохранять канонический mango y
     capacidades de набор; менять следует только apuesta, puntos finales y metadatos.
5. **Коммуникация с зависимыми командами.**
   - Владельцы SDK должны выпускать версии, которые показывают advertencias del operadorпри отклонении anuncios.
   - DevRel anonsiruetet каждую фазу; включайте ссылки на tableros y lógica
     порогов ниже.
6. **Paneles de control y alertas.**
   - Importar Grafana exportar y personalizar en **SoraFS / Proveedor
     Implementación** con UID `sorafs-provider-admission`.
   - Убедитесь, что reglas de alerta disponibles en el canal externo
     `sorafs-advert-rollout` en puesta en escena y producción.

## Telemetría y tableros

Las métricas utilizadas en las configuraciones son `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — счетчики принятия, отклонения
  y advertencias. Причины включают `missing_envelope`, `unknown_capability`, `stale` y
  `policy_violation`.

Exportación Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importar archivo en el repositorio de datos (`observability/dashboards`) y
обновите только UID datasource antes de la publicación.

Panel de control publicado en el paquete Grafana **SoraFS / Implementación del proveedor** con
стабильным UID `sorafs-provider-admission`. Reglas de alerta
`sorafs-admission-warn` (advertencia) y `sorafs-admission-reject` (crítico)
преднастроены на política уведомлений `sorafs-advert-rollout`; hombre de contacto
пункт при изменении списка получателей вместо правки JSON дашборда.

Paneles recomendados Grafana:| Paneles | Consulta | Notas |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pila para visualización aceptar, advertir y rechazar. Alerta por advertencia > 0,05 * total (advertencia) o rechazo > 0 (crítico). |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Однолинейная series temporales, питающая порог buscapersonas (tasa de advertencia del 5% en una sesión de 15 minutos). |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Для triaje en runbook; прикрепляйте ссылки на шаги mitigación. |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Указывает на proveedores, пропустивших fecha límite de actualización; сверяйте с логами descubrimiento de caché. |

Artefactos CLI para ручных дашбордов:

- `sorafs_fetch --provider-metrics-out` piezas de repuesto `failures`, `successes` y
  `disabled` según el proveedor. Importar en paneles ad-hoc, чтобы
  supervisar el orquestador de ejecución en seco antes de los proveedores de producción.
- Polos `chunk_retry_rate` e `provider_failure_rate` en formato JSON
  подсвечивают estrangulamiento o síntomas de cargas útiles obsoletas, которые часто предшествуют
  отклонениям admisión.

### Раскладка Grafana дашборда

Tablero de observabilidad публикует отдельный — **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) — в **SoraFS / Lanzamiento del proveedor**
со следующими каноническими ID del panel:- Panel 1 — *Tasa de resultados de admisión* (área apilada, единица "ops/min").
- Panel 2 — *Relación de advertencia* (serie única), выражение
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motivos de rechazo* (serie temporal, сгруппированные по `reason`), сортировка по
  `rate(...[5m])`.
- Panel 4 — *Actualizar deuda* (estadística), отражает запрос из таблицы выше и
  Fecha límite de actualización del libro mayor de migración.

Guardar (o guardar) el esqueleto JSON en los repositorios de infraestructura
`observability/dashboards/sorafs_provider_admission.json`, затем обновите только
fuente de datos UID; ID de panel y reglas de alerta implementadas en runbooks no, no hay publicaciones
перенумеровывайте их без обновления этой документации.

Для удобства репозиторий уже содержит definición del panel de referencia en
`docs/source/grafana_sorafs_admission.json`; скопируйте его в вашу Grafana папку,
No hay ninguna variante inicial para la prueba local.

### Правила алертов Prometheus

Добавьте следующую группу правил в
`observability/prometheus/sorafs_admission.rules.yml` (создайте файл, если это
первая группа правил SoraFS) and подключите ее в конфигурации Prometheus.
Introduzca `<pagerduty>` en la etiqueta de enrutamiento real para las rutas de guardia.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Sello `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
перед отправкой изменений, чтобы убедиться, что синтаксис проходит
`promtool check rules`.

## Матрица совместимости| Anuncio de características | R0 | R1 | R2 | R3 |
|--------------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, alias канонические, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Capacidad neta `chunk_range_fetch` | ⚠️ Avisar (ingesta + telemetría) | ⚠️ Advertir | ❌ Rechazar (`reason="missing_capability"`) | ❌ Rechazar |
| Capacidad necesaria de TLV según `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rechazar | ❌ Rechazar |
| Texto `refresh_deadline` | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar |
| `signature_strict=false` (accesorios de diagnóstico) | ✅ (desarrollo de только) | ⚠️ Advertir | ⚠️ Advertir | ❌ Rechazar |

Все времена указаны в UTC. Даты aplicación de la ley отражены в migraciones libro mayor и не
будут изменены без голосования consejo; любые изменения требуют обновления этого
файла и ledger в одном PR.

> **Principalmente según las realizaciones:** R1 вводит серию `result="warn"` en
> `torii_sorafs_admission_total`. Патч ingestión Torii, добавляющий новый etiqueta,
> отслеживается вместе с задачами телеметрии SF-2; до его попадания используйте

## Коммуникация и обработка инцидентов- **Еженедельная рассылка статуса.** DevRel рассылает краткое резюме метрик
  admisión, текущих advertencias и предстоящих plazos.
- **Respuesta a incidentes.** Если срабатывают alertas `reject`, ingenieros de guardia:
  1. Забирают проблемный anuncio через descubrimiento Torii (`/v2/sorafs/providers`).
  2. Publicar anuncios válidos en la canalización del proveedor y publicarlos
     `/v2/sorafs/providers`, чтобы воспроизвести ошибку.
  3. Coordinar con anuncios rotativos según la fecha límite de actualización.
- **Заморозка изменений.** Никаких изменений capacidades de esquema en R1/R2, если
  lanzamiento del comité не одобрит; La aplicación de grasa debe proporcionarse en cualquier lugar
  окно обслуживания и фиксируйте в migración libro mayor.

## Ссылки

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)