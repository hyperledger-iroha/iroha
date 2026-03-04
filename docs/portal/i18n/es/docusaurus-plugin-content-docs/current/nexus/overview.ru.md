---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: Обзор Sora Nexus
descripción: Высокоуровневый обзор архитектуры Iroha 3 (Sora Nexus) с указателями на канонические документы monorepozitoria.
---

Nexus (Iroha 3) расширяет Iroha 2 para una implementación de varios carriles, un control de la gobernanza y la mejora общих инструментов для каждого SDK. Esta página contiene el nuevo elemento `docs/source/nexus_overview.md` en monorepositorios, que son algunos de los elementos del portal архитектуры сочетаются.

## Линейки релизов

- **Iroha 2** - самостоятельные развертывания для консорциумов или частных сетей.
- **Iroha 3 / Sora Nexus** - configuración pública de varios carriles, registro de operadores de varios carriles y registros de operaciones инструменты gobernanza, liquidación y observabilidad.
- Обе линейки собираются из одного workspace (IVM + toolchain Kotodama), поэтому справления SDK, обновления ABI and фикстуры Norito está instalado permanentemente. Los operadores han descargado el paquete `iroha3-<version>-<os>.tar.zst`, según el código Nexus; обратитесь к `docs/source/sora_nexus_operator_onboarding.md` за полноэкранным чек-листом.

## bloques de estrellas| Componente | Responder | Portal de noticias |
|-----------|---------|--------------|
| Пространство данных (DS) | Определенная gobernancia область исполнения/хранения, владеющая одним или несколькими lane, объявляющая наборы валидаторов, класс приватности и политику комиссий + DA. | См. [Especificación Nexus](./nexus-spec) para estos Manifestantes. |
| Carril | Детерминированный шард исполнения; выпускает коммитменты, которые упорядочивает globalmente NPoS кольцо. El carril de clases incluye `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | [Modelo de carril](./nexus-lane-model) описывает геометрию, префиксы хранения и удержание. |
| Plan de tiempo | Identificadores de marcador de posición, marcas de marcación y configuración de un perfil de usuario diferente, como una modificación de la nueva generación эволюционируют в Nexus. | [Notas de transición](./nexus-transition-notes) документируют каждую фазу миграции. |
| Directorio espacial | Реестровый контракт, который хранит манифесты + версии DS. Los operadores pueden consultar el catálogo completo con este catálogo antes de completarlo. | El indicador de diferenciación de Trekker se encuentra en `docs/source/project_tracker/nexus_config_deltas/`. |
| Carril del catálogo | La sección de configuraciones `[nexus]` admite carriles de ID con alias, políticas de marrutización y programas DA. `irohad --sora --config … --trace-config` incluye un catálogo de auditoría personalizado. | Utilice `docs/source/sora_nexus_operator_onboarding.md` para el tutorial de CLI. || Liquidación de enrutadores | El operador XOR es un carril CBDC exclusivo con un carril de liquidez pública. | `docs/source/cbdc_lane_playbook.md` Descripción de puertas políticas y telemétricas. |
| Telemetría/SLO | Дашборды + алерты в `dashboards/grafana/nexus_*.json` фиксируют высоту lane, backlog DA, задержку задержку очереди y глубину Governance очереди. | [Plan de remediación de telemetría](./nexus-telemetry-remediation) описывает дашборды, алерты и аудит-доказательства. |

## Снимок развертывания| Faza | Enfoque | Criterios de valoración |
|-------|-------|-----------------------|
| N0 - Закрытая бета | Registrador под управлением совета (`.sora`), ручной onboarding оperatorov, статический каталог lane. | Подписанные DS манифесты + отрепетированные передачи gobernancia. |
| N1 - Публичный запуск | Добавляет суффиксы `.nexus`, аукционы, registrador de autoservicio, проводку liquidación XOR. | Тесты синхронизации resolver/gateway, дашборды сверки биллинга, настольные учения по спорам. |
| N2 - Расширение | Вводит `.dao`, API de revendedor, análisis, portales de información, cuadros de mando para azafatas. | Версионированные артефакты de cumplimiento, conjunto de herramientas en línea para jurados de políticas, отчеты прозрачности казначейства. |
| Vorot NX-12/13/14 | Motor de cumplimiento, paneles telemétricos y documentación de los pilotos asociados. | [Descripción general de Nexus](./nexus-overview) + [Operaciones de Nexus](./nexus-operations) опубликованы, дашборды подключены, motor de políticas слит. |

## Operadores externos1. **Configuraciones de configuración**: deje sincronizar `config/config.toml` con la línea de catálogos públicos y espacios de datos; Guarde el dispositivo `--trace-config` con un temporizador de cuenta regresiva.
2. **Manifestadores** - agregue el catálogo de archivos al paquete actualizado Space Directory antes de su implementación o desbloqueo.
3. **Покрытие телеметрией** - publique `nexus_lanes.json`, `nexus_settlement.json` y los paneles SDK integrados; Puede enviar alertas a PagerDuty y proporcionar medidas individuales para el plan de corrección.
4. **Отчетность об инцидентах** - seleccione la matriz de servidores en [operaciones Nexus](./nexus-operations) y coloque RCA en la tecnología пяти рабочих дней.
5. **Готовность к gobernancia** - участвуйте в голосованиях совета Nexus, влияющих на ваши lane, и раз в квартал репетируйте инструкции rollback (отслеживается через `docs/source/project_tracker/nexus_config_deltas/`).

## См. также

- Código canónico: `docs/source/nexus_overview.md`
- Especificaciones técnicas: [./nexus-spec](./nexus-spec)
- Carril geométrico: [./nexus-lane-model](./nexus-lane-model)
- Plan de actualización: [./nexus-transition-notes](./nexus-transition-notes)
- Telemetría de remediación del plan: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operación Runbook: [./nexus-operaciones](./nexus-operations)
- Гайд по incorporación de operadores: `docs/source/sora_nexus_operator_onboarding.md`