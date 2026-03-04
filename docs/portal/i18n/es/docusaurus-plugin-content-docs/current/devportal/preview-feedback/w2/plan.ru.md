---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: План admisión comunitaria W2
sidebar_label: Plan W2
descripción: Admisión, одобрения и чек-лист доказательств для comunidad vista previa когорты.
---

| Punto | Detalles |
| --- | --- |
| Volna | W2 - revisores de la comunidad |
| Целевое окно | Q3 2025 неделя 1 (предварительно) |
| Тег артефакта (plan) | `preview-2025-06-15` |
| Трекер | `DOCS-SORA-Preview-W2` |

## Цели

1. Aplicar criterios de admisión comunitaria y verificación del flujo de trabajo.
2. Política de gobernanza de la lista previa y apéndice de uso aceptable.
3. Desactive la suma de comprobación, la verificación de vista previa, el artefacto y el paquete telemétrico con una nueva opción.
4. Haga clic en Pruébelo como proxy y paneles de control para aplicaciones de software.

## Разбивка задач| identificación | Задача | Владелец | Срок | Estado | Примечания |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | Подготовить критерии admisión de la comunidad (elegibilidad, espacios máximos, требования CoC) y разослать в gobernancia | Líder de Docs/DevRel | 2025-05-15 | ✅ Завершено | La ingesta política se fusionó con `DOCS-SORA-Preview-W2` y se actualizó en 2025-05-20. |
| W2-P2 | Обновить plantilla de solicitud para la comunidad (motivación, disponibilidad, necesidades de localización) | Documentos-core-01 | 2025-05-18 | ✅ Завершено | `docs/examples/docs_preview_request_template.md` теперь включает раздел Comunidad y указан в admisión formulario. |
| W2-P3 | Получить gobernanza-одобрение плана ingesta (голосование + протокол) | Enlace de gobernanza | 2025-05-22 | ✅ Завершено | Голосование прошло единогласно 2025-05-20; протокол и roll roll связаны в `DOCS-SORA-Preview-W2`. |
| W2-P4 | Запланировать staging Pruébalo proxy + teléfono para el encendido W2 (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | ✅ Завершено | Cambiar billete `OPS-TRYIT-188` одобрен и выполнен 2025-06-09 02:00-04:00 UTC; Grafana скриншоты сохранены с тикетом. |
| W2-P5 | Obtener/proverificar nueva etiqueta de artefacto de vista previa (`preview-2025-06-15`) y registrar registros de descriptor/suma de comprobación/sonda | Portal TL | 2025-06-07 | ✅ Завершено | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` publicado el 2025-06-10; salidas сохранены в `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | Сформировать roster community приглашений (<=25 revisores, lotes preparados) с контактами, одобренными gobernanza | Responsable de la comunidad | 2025-06-10 | ✅ Завершено | Первая когорта из 8 revisores de la comunidad одобрена; IDs `DOCS-SORA-Preview-REQ-C01...C08` se escriben en el árbol. |

## Чек-лист доказательств

- [x] Запись aprobación de gobernanza (заметки встречи + ссылка на голосование) приложена к `DOCS-SORA-Preview-W2`.
- [x] Plantilla de solicitud actualizada disponible en `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, registro de suma de verificación, salida de sonda, informe de enlace y Pruébelo transcripción proxy en `artifacts/docs_preview/W2/`.
- [x] Capturas de pantalla de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) сняты для окна preflight W2.
- [x] Tabla de listas de ID de revisores, tickets de solicitud y marcas de tiempo de la lista de usuarios (en la sección W2 en трекере).

Deje este plan actual; Трекер ссылается на него, чтобы roadmap DOCS-SORA видел, что осталось до рассылки W2 приглашений.