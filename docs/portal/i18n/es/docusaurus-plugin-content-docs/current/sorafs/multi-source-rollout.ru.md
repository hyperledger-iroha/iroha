---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de múltiples fuentes
título: Runbook по мульти-источниковому rollout y чёрному списку провайдеров
sidebar_label: Lanzamiento de Runbook мульти-источникового
descripción: Lista de verificación de operaciones para la implementación de múltiples sistemas y configuración de seguridad en el modo actual.
---

:::nota Канонический источник
Esta página es `docs/source/sorafs/runbooks/multi_source_rollout.md`. Deje copias sincronizadas, pero no guarde ningún documento relacionado con la documentación.
:::

## Назначение

Este runbook proporciona SRE y muchos desarrolladores que trabajan con dos procesos críticos:

1. Vыкатывать мульти-ISTочниковый оркестратор контролируемыми волнами.
2. Заносить в чёрный список или понижать понижать проблемных провайдеров без дестабилизации текущих сессий.

Previamente, esta orquestación está instalada en los módulos SF-6, utilizando la configuración (`sorafs_orchestrator`, API de puerta de enlace para el servidor, Експортеры телеметрии).

> **См. Etiqueta:** [Runbook по эксплуатации оркестратора](./orchestrator-ops.md) подробно описывает процедуры на прогон (снятие marcador, переключатели поэтапного implementación, reversión). Utilice todas las prendas adecuadas para su uso.

## 1. Validación previa1. **Подтвердить входные данные gobernanza.**
   - Все кандидаты-провайдеры должны публиковать конверты `ProviderAdvertV1` с payload'ами дапазонных возможностей и бюджетами потоков. Pruebe через `/v2/sorafs/providers` y сверяйте с ожидаемыми полями возможностей.
   - Los televisores con métricas latentes/sboevs no deben tardar más de 15 minutos antes del programa canario.
2. **Подготовить конфигурацию.**
   - Configure la configuración JSON del operador en el directorio `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Desactive JSON con limitaciones en la implementación del módulo (`max_providers`, бюджеты ретраев). Utilice Odín y todo el equipo de puesta en escena/producción, ya que hay pocas opciones mínimas.
3. **Прогнать канонические accesorios.**
   - Activar permanentemente el manifiesto/token y activar la recuperación determinada:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     La operación permanente permite almacenar el manifiesto de carga útil de resumen (hexadecimal) y los tokens de flujo de código base64 para el procesador de datos. canario.
   - Seleccione `artifacts/canary.scoreboard.json` según la resolución del programa. Любой новый невалидный провайдер или сдвиг веса >10% требует ревью.
4. **Proverite, что телеметрия подключена.**
   - Exporte Grafana a `docs/examples/sorafs_fetch_dashboard.json`. Tenga en cuenta que las métricas `sorafs_orchestrator_*` se ajustan a la puesta en escena antes del período.

## 2. Экстренное занесение провайдеров в чёрный списокSiga este procedimiento, когда провайдер отдает поврежденные чанки, устойчиво таймаутится o не проходит проверки соответствия.1. **Зафиксировать доказательства.**
   - Exportar el archivo fetch-свод (вывод `--json-out`). Запишите индексы сбойных чанков, алиасы провайдеров и несовпадения digest.
   - Coloque los logotipos de fragmentos relevantes en los destinos `telemetry::sorafs.fetch.*`.
2. **Применить немедленный anulación.**
   - Si un supervisor es penalizado en algunos televisores, el operador no estará disponible (ustanovite `penalty=true` o `token_health`). para `0`). Следующая сборка marcador автоматически исключит провайдера.
   - Para las pruebas de humo ad-hoc que conectan `--deny-provider gw-alpha` a `sorafs_cli fetch`, puede intentar eliminar cualquier problema de limpieza. televisores.
   - Переразверните обновленный пакет телеметрии/конфигурации в затронутой среде (puesta en escena → canario → producción). Verifique la información en un incidente diario.
3. **Anular la modificación.**
   - Повторите buscar accesorio канонического. Tenga en cuenta que este marcador se puede comprobar como no válido según el modelo `policy_denied`.
   - Pruebe `sorafs_orchestrator_provider_failures_total`, чтобы убедиться, что счетчик перестал расти для отклоненного провайдера.
4. **Эскалировать долгие блокировки.**
   - Si el pedido se realiza durante >24 h, guarde el billete de gobernanza en la rotación o el anuncio de su propio anuncio. Si utiliza la lista de denegación y cambia los televisores, no se mostrarán en el marcador.
5. **Протокол отката.**- Чтобы вернуть провайдера, удалите его из deny-lista, perеразверните and snimitе novad snapshot Scoreboard. Pruebe la evaluación del incidente post mortem.

## 3. Implementación del plan postal

| Faza | Охват | Требуемые señales | Criterios Pasa/No pasa |
|------|-------|-------------------|-------------------|
| **Laboratorio** | Claster integral integrado | Ручной CLI recuperar cargas útiles фикстур | Все чанки проходят, счетчики провайдерских сбоев остаются na 0, доля ретраев < 5%. |
| **Puesta en escena** | Plano de control de preparación de Polonia | Tablero Grafana подключен; правила алертов в режиме sólo advertencia | `sorafs_orchestrator_active_fetches` indica que no hay ningún código de prueba progresivo; нет алертов `warn/critical`. |
| **Canarias** | ≤10% del tráfico | Buscapersonas sin monitores telemétricos en lugares reales | Si los retrocesos son < 10%, los diagramas de estado latentes se ajustan a la línea de base de estadificación ±20%. |
| **Envío gratuito** | 100% lanzamiento | Правила buscapersonas activos | La pantalla `NoHealthyProviders` durante 24 h, para el retiro estable, los paneles SLA en el tablero de instrumentos. |

Для каждой фазы:1. Desactive el ordenador JSON con los planificadores `max_providers` y los reproductores de archivos.
2. Introduzca `sorafs_cli fetch` o las pruebas integradas del SDK sobre dispositivos canónicos y el manifiesto de representación de los archivos.
3. Сохраните artefactos marcador + resumen y приложите их к записи о релизе.
4. Coloque los televisores en el tablero de instrumentos en el interior del televisor antes de la fase de inicio.

## 4. Наблюдаемость и incidentes ganchos

- **Metrics:** Tenga en cuenta los monitores Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Резкий всплеск обычно означает деградацию провайдера под нагрузкой.
- **Logo:** Introduzca los objetivos `telemetry::sorafs.fetch.*` en el agregador de registros. Introduzca el número de serie para `event=complete status=failed`, para utilizar este trío.
- **Marcadores:** Сохраняйте каждый marcador de artefactos en долговременное хранилище. La interfaz JSON se utiliza para comprobar el cumplimiento y verificar el cumplimiento.
- **Paneles de control:** Клонируйте канонический Grafana-дэшbord (`docs/examples/sorafs_fetch_dashboard.json`) в продакшн-папку с правилами алертов из `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicaciones y documentación- Registre la función de denegar/impulsar el registro de cambios de operación con personas, operadores, expertos e incidentes experimentados.
- Уведомляйте команды SDK при изменениях весов провайдеров или бюджетов ретраев, чтобы синхронизировать ожидания на стороне cliente.
- Después de la liberación de GA, se implementó el lanzamiento del código `status.md` y se implementó este archivo en el runbook en los archivos de configuración.