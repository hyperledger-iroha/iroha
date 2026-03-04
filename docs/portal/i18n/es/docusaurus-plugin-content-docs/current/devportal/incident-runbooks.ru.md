---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбуки инцидентов и тренировки reversión

## Назначение

Пункт дорожной карты **DOCS-9** требует прикладных playbook'ов и плана repetiций, чтобы
El portal del operador puede estar disponible después de realizar envíos sin cargo. Эта заметка
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки, доказывающие, что rollback alias
и синтетическая валидация продолжают работать de extremo a extremo.

### Materiales usados

- [`devportal/deploy-guide`](./deploy-guide) — flujo de trabajo de flujo de trabajo, nombres de usuario y alias de promoción.
- [`devportal/observability`](./observability) — etiquetas de lanzamiento, análisis y sondas, упомянутые ниже.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — televisores y aparatos eléctricos.
- `docs/portal/scripts/sorafs-pin-release.sh` y ayudantes `npm run probe:*`
  упомянуты в чеклистах.

### Telemetría e instrumentos| Señal / Instrumento | Назначение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | Выявляет остановки репликации и нарушения SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Измеряет глубину atraso y задержку завершения для triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Coloque una puerta de enlace de acero, una forma de implementar el sistema. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas, reversiones de compuertas y retrocesos de prueba. |
| `npm run check:links` | Puerta битых ссылок; используется после каждой mitigación. |
| `sorafs_cli manifest submit ... --alias-*` (через `scripts/sorafs-pin-release.sh`) | Механизм alias de promoción/reversión. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Агрегирует rechazos/alias/TLS/replicación telemétrica. Las alertas de PagerDuty se instalan en estos paneles de configuración. |

## Ранбук - Неудачный деплой или плохой артефакт

### Условия срабатывания

- Пробы vista previa/producción падают (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` después del lanzamiento.
- QA вручную замечает сломанные маршруты или сбои proxy Pruébalo сразу после
  alias de promoción.

### Немедленное сдерживание1. **Mostrar configuración:** eliminar la canalización de CI `DEPLOY_FREEZE=1` (flujo de trabajo de GitHub de entrada)
   или приостановить Jenkins job, чтобы новые артефакты не выходили.
2. **Зафиксировать артефакты:** скачать `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, y sondas de falla de construcción,
   чтобы rollback ссылался на точные resúmenes.
3. **Otros equipos:** SRE de almacenamiento, responsable principal de Docs/DevRel y responsable de gobernanza
   (особенно если затронут `docs.sora`).

### Reversión del proceso

1. Actualizar el manifiesto del último bien conocido (LKG). Flujo de trabajo de producción хранит их в
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Utilice el alias de este manifiesto con el ayudante de envío:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Complete la reversión del resumen del ticket de incidente con el resumen de LKG y el manifiesto no actualizado.

### Validación

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (см. guía de implementación), чтобы убедиться, что manifiesto repromocionado совпадает с архивным CAR.
4. `npm run probe:tryit-proxy` está disponible, en el proxy de preparación Try-It de verano.

### После инцидента

1. Despliegue la tubería para implementar la causa raíz.
2. Haga clic en "Lecciones aprendidas" en [`devportal/deploy-guide`](./deploy-guide)
   новыми выводами, если есть.
3. Завести defectos для провалившихся тестов (sonda, verificador de enlaces, y т.д.).

## Ранбук - Деградация репликации

### Условия срабатывания- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  0,95` en 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` en la técnica 10 minutos (см.
  `pin-registry-ops.md`).
- Gobernanza сообщает о медленной доступности alias после lanzamiento.

### Triaje

1. Проверить paneles de control [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops), чтобы
   понять, локализован ли backlog в clase de almacenamiento или в flota провайдеров.
2. Repita los registros Torii en `sorafs_registry::submit_manifest`, чтобы определить,
   не падают ли presentaciones.
3. Выборочно проверить здоровье реплик через `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам).

### Mitigación

1. Перевыпустить manifiesto с более высоким числом реплик (`--pin-min-replicas 7`) через
   `scripts/sorafs-pin-release.sh`, el programador de чтобы распределил нагрузку на больший набор
   провайдеров. Зафиксировать новый resumen en el registro de incidentes.
2. Si el trabajo pendiente se actualiza con un programador de replicación completo, se puede eliminar el programador de replicación
   (описано в `pin-registry-ops.md`) и отправить новый manifest, принуждающий остальных
   провайдеров обновить alias.
3. Para cambiar el alias de la paridad, volver a vincular el alias en el manifiesto cálido en la puesta en escena
   (`docs-preview`), затем опубликовать manifiesto de seguimiento после очистки backlog SRE.

### Recuperación y cierre1. Monitorice `torii_sorafs_replication_sla_total{outcome="missed"}` y compruebe qué
   счетчик стабилизировался.
2. Сохранить вывод `sorafs_cli manifest status` как evidencia, что каждая реплика снова в норме.
3. Actualizar o eliminar el trabajo pendiente de replicación post-mortem con dalneyшими шагами
   (масштабирование провайдеров, tuning fragmenter, y т.д.).

## Ранбук - Отключение аналитики или телеметрии

### Условия срабатывания

- `npm run probe:portal` Proходит, но Dashboards перестают принимать события
  `AnalyticsTracker` aproximadamente 15 minutos.
- Revisión de privacidad фиксирует неожиданный рост eventos eliminados.
- `npm run probe:tryit-proxy` se coloca en la base `/probe/analytics`.

### Respuesta

1. Probar las entradas de tiempo de construcción: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` – artefacto de liberación (`build/release.json`).
2. Presione `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT`, sin cambios
   En el recopilador de preparación, cómo se pueden almacenar las cargas útiles del rastreador.
3. Если coleccionistas недоступны, установить `DOCS_ANALYTICS_ENDPOINT=""` и reconstruir,
   чтобы rastreador de cortocircuito; зафиксировать окно interrupción en el cronograma del incidente.
4. Proverite, что `scripts/check-links.mjs` продолжает huella digital `checksums.sha256`
   (аналитические сбои *не* должны блокировать проверку sitemap).
5. Después del coleccionista de archivos, descargue `npm run test:widgets`, чтобы прогнать
   Ayudante de análisis de pruebas unitarias antes de volver a publicar.

### После инцидента1. Обновить [`devportal/observability`](./observability) с новыми ограничениями
   colector или требованиями muestreo.
2. Aviso de gobernanza, así como análisis de datos por parte de terceros o de terceros
   вне политики.

## Квартальные учения по устойчивости

Запускайте оба drill в **первый вторник каждого квартала** (enero/abril/julio/octubre)
или сразу после любого крупного изменения инфраструктуры. Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Учение | Shagi | Доказательства |
| ----- | ----- | -------- |
| Reversión de alias repetidos | 1. Revertir "Implementación fallida" con el mismo manifiesto de producción.2. Vuelva a vincular las sondas de producción después de usarlas.3. Сохранить `portal.manifest.submit.summary.json` y логи sondas в папке taladro. | `rollback.submit.json`, sondas y etiquetas de liberación repetidas. |
| Auditoría sintética de validación | 1. Запустить `npm run probe:portal` и `npm run probe:tryit-proxy` против producción y puesta en escena.2. Presione `npm run check:links` y arquivirice `build/link-report.json`.3. Haga capturas de pantalla/exportaciones del panel Grafana, que permite usar sondas. | Логи probes + `link-report.json` со ссылкой на manifiesto de huellas dactilares. |

Эскалируйте пропущенные ejercicios менеджеру Docs/DevRel y на revisión de gobernanza SRE,
так как hoja de ruta требует детерминированных квартальных доказательств, что alias rollback
и sondas de portal остаются здоровыми.

## PagerDuty y guardias de guardia- Servicio PagerDuty **Docs Portal Publishing** владеет алертами из
  `dashboards/grafana/docs_portal.json`. Palabra `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry` en Docs/DevRel primarios
  с Almacenamiento SRE как secundario.
- Al usar `DOCS_RELEASE_TAG`, haga capturas de pantalla de las fuentes
  El panel Grafana y agrega la sonda/verificación de enlace en los datos del incidente
  начала mitigación.
- Después de la mitigación (revertir o volver a implementar) se puede descargar `npm run probe:portal`,
  `npm run check:links` y captura las instantáneas Grafana, métricas de medición de ubicación
  в пороги. Utilice todas las pruebas de un incidente con PagerDuty para realizar la descarga.
- Если два алерта срабатывают одновременно (por ejemplo, vencimiento y acumulación de TLS), сначала
  rechazos de clasificación (publicación de остановить), выполнить rollback, затем закрыть TLS/backlog
  с Almacenamiento SRE на puente.