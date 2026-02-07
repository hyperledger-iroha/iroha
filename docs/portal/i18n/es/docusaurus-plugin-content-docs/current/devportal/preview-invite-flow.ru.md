---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista previa de la página de fotos

## Назначение

Пункт дорожной карты **DOCS-SORA** указывает onboarding ревьюеров and программу приглашений public preview как последние блокеры перед Nuestro portal es beta. Esta página es una descripción detallada de cómo crear una copia de seguridad cuando se utiliza el software antes de usarla. как доказать, что поток аудируем. Используйте вместе с:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) для работы с каждым ревьюером.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para la suma de comprobación de garantía.
- [`devportal/observability`](./observability.md) para televisores deportivos y ganchos colocados.

## Plan de vuelo| Volna | Auditorio | Criterios de vídeo | Criterios de valoración | Примечания |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores Docs/SDK, валидирующие контент дня один. | Comando GitHub `docs-portal-preview` instalado, suma de comprobación de puerta en `npm run serve` instalado, Alertmanager el 7 de diciembre. | Все P0 доки просмотрены, backlog промаркирован, блокирующих инцидентов нет. | Используется для проверки потока; correo electrónico-инвайтов нет, только обмен vista previa артефактами. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii, revisores de gobernanza según NDA. | W0 завершена, юридические условия утверждены, Try-it proxy en preparación. | Socios de aprobación sobran (problema o formulario de confirmación), posición de telemetría =2 документационных релиза прошли через vista previa de canalización antes de la reversión. | Registre los ajustes predeterminados (<=25) y regálelos por lotes. |

Puede activar la documentación inmediatamente en `status.md` y en el rastreador de solicitudes de vista previa, para saber cuál es el estado de su gobierno en la actualidad.

## Comprobación previa a la verificación

Asegúrese de que este diseño **según** la planificación del programa para el volumen:1. **Dispositivos CI**
   - Последний `docs-portal-preview` + descriptor загружен `.github/workflows/docs-portal-preview.yml`.
   - SoraFS pin отмечен в `docs/portal/docs/devportal/deploy-guide.md` (cambio de descriptor присутствует).
2. **Suma de comprobación previa**
   - `docs/portal/scripts/serve-verified-preview.mjs` está conectado a `npm run serve`.
   - Instrucciones `scripts/preview_verify.sh` protegidas en macOS + Linux.
3. **Telemetría**
   - `dashboards/grafana/docs_portal.json` elimina el tráfico de tráfico Pruébelo y avise a `docs.preview.integrity`.
   - La aplicación actualizada en `docs/portal/docs/devportal/observability.md` está disponible exclusivamente en Grafana.
4. **Gobernanza de los artefactos**
   - Rastreador de invitaciones de problemas готов (одна issues на волну).
   - Шаблон реестра ревьюеров скопирован (см. [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Юридические и SRE aprobaciones приложены к cuestión.

Realice una verificación previa del rastreador de invitaciones antes de la prueba.

## Шаги потока1. **Выбор кандидатов**
   - Consulte la hoja de cálculo de la lista de espera o la cola de socios.
   - Убедиться, что у каждого кандидата заполнен request template.
2. ** Одобрение доступа **
   - Aprobador de aprobación del rastreador de invitaciones de problemas.
   - Проверить требования (CLA/contracto, uso aceptable, resumen de seguridad).
3. **Pravka приглашений**
   - Presione los botones en [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Pruebe el descriptor + hash arхива, pruébelo, URL provisional y canales de almacenamiento.
   - Сохранить финальное письмо (или Matrix/Slack transcript) en el número.
4. **Incorporación avanzada**
   - Eliminar las políticas de seguimiento de invitaciones `invite_sent_at`, `expected_exit_at` y estados (`pending`, `active`, `complete`, `revoked`).
   - Привязать solicitud de admisión ревьюера для аудитабельности.
5. **Monitorización de televisores**
   - Seleccione `docs.preview.session_active` y las alertas `TryItProxyErrors`.
   - Puede ocurrir un incidente con los televisores de la línea base y reducir los resultados de una aplicación rápida.
6. **Сбор фидбека и выход**
   - Cierre la configuración después de la instalación de fibra óptica o la instalación `expected_exit_at`.
   - Обновить issues волны кратким резюме (находки, инциденты, следующие шаги) перед переходом к следующей когорте.

## Evidencia y отчетность| Artefacto | Где хранить | Частота обновления |
| --- | --- | --- |
| Rastreador de invitaciones de emisión | Proyecto GitHub `docs-portal-preview` | Обновлять после каждого приглашения. |
| Экспорт lista ревьюеров | Реестр, связанный в `docs/portal/docs/devportal/reviewer-onboarding.md` | Еженедельно. |
| Telemetros de televisión | `docs/source/sdk/android/readiness/dashboards/<date>/` (paquete de telemetría personalizado) | На каждую волну + после инцидентов. |
| Resumen de comentarios | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (paquete en volumen) | В течение 5 dней после выхода из волны. |
| Nota de la reunión de gobernanza | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Utilice la sincronización de gobierno DOCS-SORA. |

Teclado `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
после каждого батча, чтобы получить машинно-читабельный digest. La configuración JSON de la edición de los archivos, la gobernanza de los revisores puede permitir que los archivos adjuntos no se ajusten a ningún otro. loga.

Pruebe las pruebas con `status.md` antes de guardar las tarjetas de crédito, es posible que las tarjetas de crédito estén disponibles.

## Criterios de reversión y pausa

Приостановите поток приглашений (и уведомите gobernancia), если происходит что-либо из следующего:

- Usuario Pruébelo proxy, reversión de la opción (`npm run manage:tryit-proxy`).
- Instalación de alertas: >3 páginas de alerta para puntos finales de solo vista previa en el 7 de diciembre.
- Пробел комплаенса: приглашение отправлено без подписанных условий или без записи request template.
- Риск целостности: falta de coincidencia en la suma de comprobación, обнаруженный `scripts/preview_verify.sh`.Utilice esta opción para corregir la documentación en el rastreador de invitaciones y estabilizar los televisores durante 48 minutos.