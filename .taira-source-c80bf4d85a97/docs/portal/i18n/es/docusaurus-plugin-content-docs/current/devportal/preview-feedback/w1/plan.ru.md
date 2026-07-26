---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-plan
título: Plan de verificación previa para socios W1
sidebar_label: Plan W1
descripción: Задачи, владельцы и чек-лист доказательств для партнерской когорты de vista previa.
---

| Punto | Detalles |
| --- | --- |
| Volna | W1 - socios e integradores Torii |
| Целевое окно | Q2 2025 неделя 3 |
| Тег артефакта (plan) | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |

## Цели

1. Vista previa de получить юридические и Governance-одобрения условий партнерского.
2. Подготовить Pruébelo proxy y televisores para el paquete de descarga.
3. Обновить checksum-верифицированный vista previa-artefacto y resultados de la sonda.
4. Finalice la operación con todos los socios y proveedores de programas de explotación.

## Разбивка задач| identificación | Задача | Владелец | Срок | Estado | Примечания |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Получить юридическое одобрение на дополнение условий vista previa | Líder de Docs/DevRel -> Legal | 2025-04-05 | ✅ Завершено | Юридический тикет `DOCS-SORA-Preview-W1-Legal` одобрен 2025-04-05; PDF приложен к трекеру. |
| W1-P2 | Зафиксировать staging-окно Pruébalo proxy (2025-04-10) y prueba здоровье прокси | Documentos/DevRel + Operaciones | 2025-04-06 | ✅ Завершено | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` publicado el 2025-04-06; Transcripción CLI y archivos `.env.tryit-proxy.bak`. |
| W1-P3 | Собрать vista previa-artefacto (`preview-2025-04-12`), запустить `scripts/preview_verify.sh` + `npm run probe:portal`, архивировать descriptor/checksums | Portal TL | 2025-04-08 | ✅ Завершено | Артефакт и логи проверки сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`; вывод sonda приложен к трекеру. |
| W1-P4 | Obtenga formularios de admisión de socios (`DOCS-SORA-Preview-REQ-P01...P08`), pueda consultar contactos y acuerdos de confidencialidad | Enlace de gobernanza | 2025-04-07 | ✅ Завершено | Все восемь запросов одобрены (последние два 2025-04-11); ссылки на одобрения в трекере. |
| W1-P5 | Coloque las aplicaciones de texto (en la base `docs/examples/docs_preview_invite_template.md`), descargue `<preview_tag>` y `<request_ticket>` para el socio | Líder de Docs/DevRel | 2025-04-08 | ✅ Завершено | Черновик приглашения отправлен 2025-04-12 15:00 UTC вместе с ссылками на артефакт. |

## Comprobación previa a la verificación> Sovet: запустите `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`, чтобы автоматически выполнить шаги 1-5 (build, проверка checksum, portal probe, link checker y обновление Pruébelo proxy). El script de archivo JSON se puede reproducir en un problema.

1. `npm run build` (с `DOCS_RELEASE_TAG=preview-2025-04-12`) para la renovación de `build/checksums.sha256` y `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archiva `build/link-report.json` рядом с descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (или указать нужный target через `--tryit-target`); desinstale el `.env.tryit-proxy` y actualice el `.bak` para revertirlo.
6. Actualice el problema W1 con los logotipos (descriptor de suma de verificación, sonda de prueba, configuración Pruébelo como proxy y instantáneas Grafana).

## Чек-лист доказательств

- [x] Подписанное юридическое одобрение (PDF или ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] Grafana pantallas para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y suma de comprobación-лог `preview-2025-04-12` сохранены в `artifacts/docs_preview/W1/`.
- [x] Таблица roster приглашений с заполненными `invite_sent_at` (см. W1 log в трекере).
- [x] Артефакты обратной связи отражены в [`preview-feedback/w1/log.md`](./log.md) с одной строкой на партнера (обновлено 2025-04-26 данными roster/telemetria/issues).

Обновляйте этот plan по мере продвижения; трекер ссылается на него, чтобы сохранить hoja de ruta de auditabilidad.

## Процесс обратной связи1. El crítico de Для каждого дублировать шаблон
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   заполнить метаданные и сохранить готовую копию в
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Сводить приглашения, телеметрические checkpoints and открытые issues в живом логе
   [`preview-feedback/w1/log.md`](./log.md), чтобы revisores de gobernanza могли воспроизвести волну
   полностью, не покидая репозиторий.
3. Когда приходят экспорты conocimiento-verificación o прикреплять их по пути артефакта, указанному в логе,
   и связывать с cuestión трекера.