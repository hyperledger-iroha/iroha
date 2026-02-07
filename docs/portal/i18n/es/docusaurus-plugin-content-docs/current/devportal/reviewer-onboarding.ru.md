---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista previa de Онбординг ревьюеров

## Objeto

DOCS-SORA elimina el portal de descargas eléctricas. Сборки с puerta по suma de comprobación
(`npm run serve`) y fotos útiles Pruébelo открывают следующий этап: онбординг проверенных
ревьюеров до широкого открытия vista previa pública. Esta es la descripción que tenemos para saber cómo hacerlo,
проверять соответствие, выдавать доступ и безопасно завершать участие. См.
[vista previa del flujo de invitación](./preview-invite-flow.md) для планирования когорт, каденции приглашений
y televisores deportivos; шаги ниже фокусируются на действиях после выбора ревьюера.

- **В рамках:** ревьюеры, которым нужен доступ k docs de vista previa (`docs-preview.sora`,
  сборки GitHub Pages y бандлы SoraFS) para GA.
- **Вне рамок:** operadores Torii o SoraFS (programas de incorporación de software) y
  продакшн-развертывания портала (см.
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Роли и требования| Роль | Células típicas | Требуемые артефакты | Примечания |
| --- | --- | --- | --- |
| Mantenedor principal | Проверить новые гайды, запустить pruebas de humo. | Identificador de GitHub, contacto con Matrix, proveedor de CLA. | Обычно уже в команде GitHub `docs-preview`; все равно подайте заявку, чтобы доступ был аудируем. |
| Revisor socio | Pruebe los archivos SDK y el contenido actualizado para la publicación. | Correo electrónico corporativo, POC legal, términos de vista previa disponibles. | Asegúrese de conectar tres televisores y televisores. |
| Voluntario comunitario | Дать comentarios по удобству использования гайдов. | Identificador de GitHub, contacto previo, conexión completa, aplicación con CoC. | Держите когорты небольшими; приоритет ревьюерам, подписавшим acuerdo de colaborador. |

Estos son los tipos de versiones de muñecas:

1. Подтвердить политику допустимого использования-preview-artефактов.
2. Прочитать приложения по seguridad/observabilidad
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Согласиться запускать `docs/portal/scripts/preview_verify.sh` перед тем, как
   обслуживать любой локальный instantánea.

## ingesta de proceso1. Попросите заявителя заполнить
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (или вставить ее в problema). Зафиксируйте как minимум: личность, способ связи,
   Mango de GitHub, planificación de datos revisados y actualizaciones de temas de seguridad.
2. Зафиксируйте заявку в трекере `docs-preview` (problema de GitHub o actualización de ticket)
   и назначьте aprobador.
3. Проверьте требования:
   - CLA / соглашение контрибьютора в наличии (o ссылка на партнерский контракт).
   - Подтверждение допустимого использования хранится в заявке.
   - Риск-оценка завершена (например, партNERские ревьюеры одобрены Legal).
4. El aprobador soluciona el problema de seguimiento y soluciona el problema con el código de registro
   gestión de cambios (пример: `DOCS-SORA-Preview-####`).

## Aprovisionamiento e instrumentos

1. **Передать артефакты** — Предоставьте последний vista previa descriptor + archivo из
   Flujo de trabajo de CI en el pin SoraFS (artículo `docs-portal-preview`). Напомните ревьюерам запустить:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir con suma de verificación de cumplimiento** — Укажите команду с gate по checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Esta versión `scripts/serve-verified-preview.mjs` es una compilación incompleta
   не запускался случайно.

3. **Дать доступ к GitHub (opcional)** — Если нужны неопубликованные ветки, добавьте
   Revisiones en el comando GitHub `docs-preview` durante el período de revisión y actualización de la configuración
   в заявке.4. **Коммуникация каналов поддержки** — Contacto de contacto de guardia (Matrix/Slack) y
   El procedimiento es [`incident-runbooks`](./incident-runbooks.md).

5. **Medición + comentarios** — Напомните, что собирается анонимизированная аналитика
   (см. [`observability`](./observability.md)). Formulario de comentarios o publicación de problemas,
   указанный в приглашении, и зафиксируйте событие через helper
   [`preview-feedback-log`](./preview-feedback-log), чтобы сводка волны оставалась актуальной.

## Чеклист ревьюера

Antes de realizar la vista previa de las últimas actualizaciones:

1. Проверить скачанные артефакты (`preview_verify.sh`).
2. Abra el puerto `npm run serve` (o `serve:verified`) para activar la suma de comprobación de guardia.
3. Procure mejorar las condiciones de seguridad y observabilidad.
4. Pruebe OAuth/Pruébelo desde la consola de inicio de sesión con código de dispositivo (primero) y no realice el seguimiento
   fichas de producción.
5. Фиксировать находки в согласованном трекере (problema, общий документ или FORMа) и тегировать
   их релизным тегом vista previa.

## Ответственности мейнтейнеров и offboarding| Faza | Действия |
| --- | --- |
| Inicio | Убедиться, что что чеклист приложен к заявке, поделиться артефактами + инструкциями, добавить запись `invite-sent` Después de [`preview-feedback-log`](./preview-feedback-log), y запланировать промежуточный sync, если ревью длится более недели. |
| Monitoreo | Utilice un televisor de vista previa (tráfico no deseado Pruébelo, con una sonda) y realice un registro de incidentes antes de realizar el pedido. Registre el archivo `feedback-submitted`/`issue-opened` después de una simple instalación, que tiene varias métricas. |
| Baja de embarque | Descargue el archivo GitHub o SoraFS, cierre `access-revoked`, descargue el resumen (comentarios resumidos + comentarios) действия), y обновить реестр ревьюеров. Utilice el editor para actualizar archivos locales y reproducir el resumen en [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilice este proceso para rotar las teclas de volumen. Сохранение следов в репозитории
(problema + шаблоны) помогает DOCS-SORA оставаться аудируемым и позволяет gobernancia подтвердить,
что доступ к previsualización del control de documentos.

## Шаблоны приглашений и трекинг- Начинайте каждое обращение с файла
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  En la computadora portátil, las instrucciones para la vista previa de la suma de verificación y la operación,
  что ревьюеры признают политику допустимого использования.
- Para editar el cable, coloque los canales `<preview_tag>`, `<request_ticket>` y canales.
  Solicite copias finales de las entradas de admisión, revisores, aprobadores y auditores.
  могли сослаться на точный текст.
- Posibles aplicaciones para desactivar la hoja de cálculo de seguimiento o el problema con la marca de tiempo `invite_sent_at`
  и ожидаемой датой завершения, чтобы отчет
  [vista previa del flujo de invitación](./preview-invite-flow.md) автоматически подхватил когорту.