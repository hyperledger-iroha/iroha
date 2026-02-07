---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Плейбук приглашений на vista previa pública

## Estos programas

Este плейбук объясняет, как анонсировать y проводить vista previa pública после того, как
flujo de trabajo онбординга ревьюеров запущен. En el sistema de tarjetas DOCS-SORA,
Garantía, что каждое приглашение уходит с проверяемыми артефактами, инструкциями
по безопасности и ясным каналом обратной связи.

- **Auditoria:** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  подписали политику uso aceptable para la vista previa.
- **Ограничения:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 дней, реакция на
  инциденты в течение 24h.

## Чеклист gate перед запуском

Выполните эти задачи перед отправкой приглашений:

1. Последние vista previa-artefactы загружены в CI (`docs-portal-preview`,
   manifiesto de suma de comprobación, descriptor, paquete SoraFS).
2. `npm run --prefix docs/portal serve` (puerta de suma de comprobación) protegida por la etiqueta.
3. Тикеты онбординга ревьюеров одобрены и связаны с волной приглашений.
4. Documentos sobre seguridad, observabilidad e incidencias
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de comentarios o plantilla de problemas (por gravedad, problemas de respuesta,
   (скриншоты и информация об окружении).
6. Texto anual de Docs/DevRel + Governance.

## Paquete de aplicaciones

Каждое приглашение должно включать:1. **Проверенные артефакты** — Consulte el manifiesto/plan SoraFS o el artefacto GitHub,
   Un manifiesto y un descriptor de suma de comprobación. Явно укажите команду верификации, чтобы
   Los expertos pueden eliminar la información antes de realizarla.
2. **Servicio de instalación** — Включите vista previa del comando с checksum gate:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться, а инциденты нужно сообщать немедленно.
4. ** Канал обратной связи** — Agregue la plantilla/formulario del problema y обозначьте ожидания по времени ответа.
5. **Даты программы** — Укажите даты начала/окончания, horario de oficina o sync встречи y следующее окно refresco.

Пример письма в
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования. Eliminar marcadores de posición (datos, URL, contactos)
перед отправкой.

## Публикация host de vista previa

Продвигайте host de vista previa para después de la incorporación y el cambio de boleto.
См. [руководство по host de vista previa de exposición](./preview-host-exposure.md) para шагов de extremo a extremo
construir/publicar/verificar, используемых в этом разделе.

1. **Compilación y actualización:** Coloque la etiqueta de lanzamiento y guarde los artefactos determinados.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   Скрипт pin записывает `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` en `artifacts/sorafs/`. Приложите эти файлы к волне
   приглашений, чтобы каждый ревьюер мог проверить те же биты.2. **alias de vista previa del usuario:** comando de entrada basado en `--skip-submit`
   (укажите `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y доказательство alias de gobernanza).
   Script de acceso al manifiesto con `docs-preview.sora` y выдаст
   `portal.manifest.submit.summary.json` más `portal.pin.report.json` para un paquete de pruebas.

3. **Implementación:** Убедитесь, что alias разрешается and checksum соответствует tag
   перед отправкой приглашений.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Deje `npm run serve` (`scripts/serve-verified-preview.mjs`) como respaldo,
   чтобы ревьюеры могли поднять локальную копию, если preview edge даст сбой.

## Таймлайн коммуникаций

| День | Действие | Propietario |
| --- | --- | --- |
| D-3 | Finalizar los programas de texto, eliminar artefactos, verificar en seco | Documentos/DevRel |
| D-2 | Aprobación de gobernanza + ticket de cambio | Documentos/DevRel + Gobernanza |
| D-1 | Отправить приглашения по шаблону, обновить tracker со списком получателей | Documentos/DevRel |
| D | Llamada inicial / horario de oficina, monitorización de televisores | Documentos/DevRel + De guardia |
| D+7 | Промежуточный resumen de comentarios, clasificación de problemas de bloques | Documentos/DevRel |
| D+14 | Cerrar todo, eliminar el último archivo, publicar el resumen en `status.md` | Documentos/DevRel |

## Трекинг доступа и телеметрия1. Guardar archivos, marcas de tiempo y datos guardados en el registrador de comentarios de vista previa
   (см. [`preview-feedback-log`](./preview-feedback-log)), чтобы каждая волна разделяла
   один и тот же rastro de evidencia:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Поддерживаемые события: `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened` y `access-revoked`. Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json`; приложите его к тикету волны
   вместе с формами согласия. Используйте resumen de ayuda, чтобы подготовить аудируемый
   roll-up перед финальной заметкой:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Resumen JSON перечисляет приглашения по волнам, открытых получателей, счетчики comentarios
   y marca de tiempo según la seguridad. Ayudante основан на
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   El proceso de flujo de trabajo se puede ejecutar localmente o en CI. Используйте шаблон digest en
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   при публикации resumen de los volúmenes.
2. Тегируйте телеметрийные paneles de control `DOCS_RELEASE_TAG`, использованным для волны, чтобы пики
   можно было сопоставлять с cohort приглашений.
3. Después de implementar, introduzca `npm run probe:portal -- --expect-release=<tag>`, para poder acceder a él.
   Esta vista previa muestra los metadatos de la versión correctos.
4. Любые и нциденты фиксируйте в шаблоне runbook y связывайте с cohort.

## Comentarios y descargas1. Solicite comentarios en otros documentos o en el foro de temas. Elementos marcados `docs-preview/<wave>`,
   Hoja de ruta de los propietarios de чтобы могли быстро их найти.
2. Utilice el resumen del registrador de vista previa para ver el contenido de la vista previa
   `status.md` (участники, ключевые находки, планируемые фиксы) y обновите `roadmap.md`,
   если hito DOCS-SORA изменился.
3. Следуйте шагам offboarding из
   [`reviewer-onboarding`](./reviewer-onboarding.md): отзовите доступ, архивируйте заявки и
   поблагодарите участников.
4. Puede utilizar todas las funciones, descubrir artefactos, puertas de suma de comprobación y
   обновив шаблон приглашения с новыми датами.

Nuevas actualizaciones de este programa de vista previa auditable y datos
Docs/DevRel puede eliminar archivos de archivos de un simple portal de archivos de GA.