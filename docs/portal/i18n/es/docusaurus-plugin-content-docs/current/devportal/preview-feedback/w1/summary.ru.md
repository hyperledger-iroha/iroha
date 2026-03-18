---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: Сводка отзывов и закрытие W1
sidebar_label: Сводка W1
descripción: Выводы, действия и доказательства выхода для волны партнеров/интеграторов Torii.
---

| Punto | Detalles |
| --- | --- |
| Volna | W1 - socios e integradores Torii |
| Окно приглашений | 2025-04-12 -> 2025-04-26 |
| Тег артефакта | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |
| Участники | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Основные моменты

1. **Suma de comprobación del flujo de trabajo** - Todos los revisores proporcionan el descriptor/archivo según `scripts/preview_verify.sh`; логи сохранены рядом с подтверждениями приглашения.
2. **Medición** - Los tableros `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` están instalados en las estaciones de servicio; инцидентов или páginas de alerta не было.
3. **Comentarios sobre documentos (`docs-preview/w1`)** - Зафиксированы две небольшие правки:
   - `docs-preview/w1 #1`: уточнить навигационную формулировку в разделе Pruébelo (закрыто).
   - `docs-preview/w1 #2`: обновить скриншот Pruébelo (закрыто).
4. **Runbook del modelo** - Los operadores SoraFS pueden ver cómo se conectan los nuevos enlaces cruzados mediante `orchestrator-ops` e `multi-source-rollout`. замечания W0.

## Пункты действий| identificación | Descripción | Владелец | Estado |
| --- | --- | --- | --- |
| W1-A1 | Desactivar formularios de navegación Pruébelo en `docs-preview/w1 #1`. | Documentos-core-02 | ✅ Завершено (2025-04-18). |
| W1-A2 | Обновить скриншот Pruébalo en `docs-preview/w1 #2`. | Documentos-core-03 | ✅ Завершено (2025-04-19). |
| W1-A3 | Свести выводы партнеров and телеметрию в roadmap/status. | Líder de Docs/DevRel | ✅ Завершено (см. tracker + status.md). |

## Esta respuesta (2025-04-26)

- Todos los revisores pueden consultar los horarios de oficina finales, los artefactos locales y las opciones de descarga.
- La televisión está instalada en el lugar de residencia; instantáneas finales приложены к `DOCS-SORA-Preview-W1`.
- Лог приглашений обновлен подтверждениями выхода; tracker отметил W1 как 🈴 и добавил puntos de control.
- Paquete de documentación (descriptor, registro de suma de comprobación, salida de sonda, transcripción de proxy Pruébelo, capturas de pantalla de telemetría, resumen de comentarios) архивирован в `artifacts/docs_preview/W1/`.

## Следующие шаги

- Подготовить план admisión comunitaria W2 (aprobación de la gobernanza + plantilla de solicitud de mejoras).
- Eliminar la etiqueta de artefacto de vista previa para el volumen W2 y verificar el script de verificación previa después de finalizar los datos.
- Перенести применимые выводы W1 в roadmap/status, чтобы community wave получила актуальные рекомендации.