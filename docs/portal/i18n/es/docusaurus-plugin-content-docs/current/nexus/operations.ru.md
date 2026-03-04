---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: Runbook по операциям Nexus
descripción: Практичный полевой обзор рабочего процесса оператора Nexus, отражающий `docs/source/nexus_operations.md`.
---

Utilice esta página para saber si está escrita en `docs/source/nexus_operations.md`. En la lista de control de operaciones integrada, se pueden configurar y ajustar los televisores y los ajustes necesarios. operadores Nexus.

## Чек-лист жизненного цикла

| Etapa | Действия | Доказательства |
|-------|--------|----------|
| Предстарт | Pruebe estas/posiciones, introduzca `profile = "iroha3"` y configure configuraciones de configuración. | Вывод `scripts/select_release_profile.py`, журнал checksum, подписанный paquete манифестов. |
| Catálogo completo | Обновить каталог `[nexus]`, политику маршрутизации и пороги DA по манифесту совета, затем сохранить `--trace-config`. | Вывод `irohad --sora --config ... --trace-config`, сохраненный с тикетом onboarding. |
| Humo y corte | Cargue `irohad --sora --config ... --trace-config`, cargue CLI smoke (`FindNetworkStatus`), cargue los televisores de exportación y cargue la admisión. | Лог prueba de humo + подтверждение Alertmanager. |
| Штатный режим | Deslice los paneles/alertas, active las teclas de control gráfico y sincronice las configuraciones/runbooks de los controladores. | Протоколы квартального обзора, скриншоты дашбордов, ID тикетов ротации. |La incorporación de personas (замена ключей, шаблоны маршрутизации, шаги профиля релиза) se encuentra en `docs/source/sora_nexus_operator_onboarding.md`.

## Управление изменениями

1. **Resolución de actualización** - deslice la configuración en `status.md`/`roadmap.md`; прикладывайте чек-лист incorporación к каждому PR релиза.
2. **Изменения манифестов lane** - Pruebe los paquetes disponibles en Space Directory y archívelos en `docs/source/project_tracker/nexus_config_deltas/`.
3. **Дельты конфигурации** - каждое изменение `config/config.toml` требует тикет с ссылкой на lane/data-space. Сохраняйте редактированную копию эфективной конфигурации при join/upgrade узлов.
4. **Reversión de funciones** - ежеквартально репетируйте detener/restaurar/ahumar; Los resultados de la grabación son `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Cumplimiento de la normativa** - carriles privados/CBDC должны получить одобрение перед изменением политики DA o perillas de redacción de televisores (см. `docs/source/cbdc_lane_playbook.md`).

## Telemetría y SLO- Paneles de control: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, un vídeo específico del SDK (por ejemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` y el transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas de monitorización:
  - `nexus_lane_height{lane_id}` - alerta sobre el progreso de tres ranuras.
  - `nexus_da_backlog_chunks{lane_id}` - alerta de todos los carriles (64 públicos / 8 privados).
  - `nexus_settlement_latency_seconds{lane_id}` - El código de alerta P99 dura entre 900 ms (público) y 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta o 5 minutos para una exposición >2%.
  - `telemetry_redaction_override_total` - Sev 2 немедленно; Sin embargo, esto anula el cumplimiento de los requisitos.
- Complete la lista de verificación de televisores con [plan de remediación de telemetría Nexus] (./nexus-telemetry-remediation) durante un mínimo de tiempo y ajuste. formulario de operación del operador.

## Матрица инцидентов| Gravedad | Определение | Ответ |
|----------|------------|----------|
| Septiembre 1 | Нарушение изоляции espacio de datos, asentamiento de остановка >15 minutos или порча голосования gobernanza. | Пейджинг Nexus Primario + Ingeniería de lanzamiento + Cumplimiento, admisión de заморозить, собрать артефакты, выпустить коммуникации 30 minут, провал rollout manifest. | Пейджинг Nexus Primary + SRE, смягчение <=4 часа, оформить seguimientos в течение 2 рабочих дней. |
| Septiembre 3 | Не блокирующий дрейф (documentos, alertas). | Зафиксировать в tracker y запланировать исправление в sprint. |

Los identificadores de entradas/espacios de datos ficticios de la línea de ID/espacio de datos, estos manifestantes, tamiláin, поддерживающие métricas/logotipos, y seguimiento задачи/ответственных.

## Архив доказательств

- Храните paquetes/manifiestos/telemetros deportivos en `artifacts/nexus/<lane>/<date>/`.
- Сохраняйте редактированные configs + вывод `--trace-config` для каждого релиза.
- Utilice protocolos compatibles + opciones de configuración según configuración adecuada o manifiesto.
- Utilice todas las instantáneas Prometheus con la métrica Nexus en una tecnología de 12 meses.
- El runbook de programas de ficción en `docs/source/project_tracker/nexus_config_deltas/README.md`, donde los auditores están disponibles, es mucho más confiable.

## Materiales nuevos- Consulta: [Descripción general de Nexus](./nexus-overview)
- Especificaciones: [Especificación Nexus] (./nexus-spec)
- Carril geométrico: [modelo de carril Nexus] (./nexus-lane-model)
- Pasos y cuñas de enrutamiento: [Notas de transición Nexus](./nexus-transition-notes)
- Incorporación de operadores: [Incorporación de operadores Sora Nexus](./nexus-operator-onboarding)
- Remediación de telemetría: [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)