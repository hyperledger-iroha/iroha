---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: Заметки по развертыванию SoraFS
sidebar_label: Artículos relacionados con la actualización
descripción: Чеклист для продвижения пайплайна SoraFS из CI в продакшен.
---

:::nota Канонический источник
:::

# Заметки по развертыванию

El panel de control SoraFS utiliza un parámetro de configuración, un período de CI en el modo de operación actual barandillas. Utilice esta lista de verificación en la herramienta de implementación de proveedores de almacenamiento y puertas de enlace reales.

## Предварительная проверка

- **Выравнивание реестра** — убедитесь, что профили fragmenter and manifests ссылаются на одинаковый кортеж `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Admisión política**: compruebe los anuncios de proveedores y las pruebas de alias, no disponibles para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Registro de PIN de Runbook** — держите `docs/source/sorafs/runbooks/pin_registry_ops.md` под рукой для сценариев восстановления (ротация alias, сбои репликации).

## Configuración de configuración

- Las puertas de enlace permiten la transmisión a prueba de endpoints (`POST /v2/sorafs/proof/stream`), y la CLI puede transmitir datos telefónicos.
- Configure la política `sorafs_alias_cache` con el asistente CLI (`sorafs_cli manifest submit --alias-*`).
- Guarde los tokens de transmisión (o el Torii) en su administrador secreto.
- Включите телеметрические exportadores (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) и отправляйте их в ваш стек Prometheus/OTel.## Lanzamiento de la estrategia

1. **Manifiestos azul/verde**
   - Se implementó `manifest submit --summary-out` para la implementación del archivo anterior.
   - Siga `torii_sorafs_gateway_refusals_total`, ya que no necesita agua.
2. **Pruebas de Проверка**
   - Считайте сбои в `sorafs_cli proof stream` блокерами развертывания; всплески латентности часто означают провайдера или неверно настроенные niveles.
   - `proof verify` para realizar una prueba de humo después de un pin, para que se pruebe el vehículo y para que se adapte al manifiesto de digestión.
3. **Paneles telemétricos**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` a Grafana.
   - Paneles de datos para el registro de pines (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y rango de fragmentos estadísticos.
4. **Включение multifuente**
   - Следуйте этапным шагам rollout из `docs/source/sorafs/runbooks/multi_source_rollout.md` при включении Orchestrator y архивируйте артефакты marcador/telemetros para audiciones.

## Обработка инцидентов

- Siga las escalas en `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para la puerta de enlace de interrupción y el token de transmisión.
  - `dispute_revocation_runbook.md`, когда возникают споры репликации.
  - `sorafs_node_ops.md` para la instalación del nuevo sistema.
  - `multi_source_rollout.md` para anular el administrador, incluir en la lista negra a pares y realizar implementaciones periódicas.
- Complete las pruebas y anomalías latentes en GovernanceLog para verificar la API de seguimiento de PoR, cómo puede permitir la gobernanza провайдеров.

## Следующие шаги- Orquestador de búsqueda automático integrado (`sorafs_car::multi_fetch`), orquestador de búsqueda de múltiples fuentes (SF-6b).
- Отслеживайте обновления PDP/PoTR в рамках SF-13/SF-14; La CLI y los documentos están actualizados, están eliminados y hay varios niveles, y estas pruebas se estabilizan.

Notas sobre la configuración de inicio rápido y recetas de CI, los comandos que utilizan parámetros de experiencia locales de grado de producción Tuberías SoraFS с повторяемым и наблюдаемым процессом.