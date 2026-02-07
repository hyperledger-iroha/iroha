---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Чек‑list публикации портала
descripción: Шаги проверки перед обновлением портала документации Iroha.
---

Utilice esta lista de verificación en el portal de actualizaciones. Он гарантирует, что
CI‑сборка, implementación de GitHub Pages y ручные smoke‑testы покрывают все разделы перед релизом
или вехой hoja de ruta’а.

## 1. Локальная проверка

- `npm run sync-openapi -- --version=current --latest` (добавьте один или несколько
  (las banderas `--mirror=<label>`, o Torii (OpenAPI pueden usarse para una instantánea de la memoria).
- `npm run build` — убедитесь, что слоган “Construya sobre Iroha con confianza” всё ещё
  отображается в `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` — manifiesto de protección
  checksum'ов (добавьте `--descriptor`/`--archive` при тестировании скачанных CI‑артефактов).
- `npm run serve` — запускает helper‑режим с проверкой checksum’ов, который валидирует
  manifiesto antes de que `docusaurus serve`, чтобы ревьюеры никогда не смотрели на
  instantánea неподписанный (alias `serve:verified` остаётся для явных вызовов).
- Mejora las plantillas de markdown para `npm run start` y dev-server con live-reload.

## 2. Solicitud de extracción de solicitudes- Убедитесь, что job `docs-portal-build` прошёл успешно в
  `.github/workflows/check-docs.yml`.
- Проверьте, что был запущен `ci/check_docs_portal.sh` (в логах CI отображается hero-проверка).
- Убедитесь, qué vista previa del flujo de trabajo muestra el manifiesto (`build/checksums.sha256`) y qué
  скрипт проверки vista previa выполнился успешно (логи CI содержат вывод
  `scripts/preview_verify.sh`).
- Agregue la URL de vista previa pública a la página de GitHub en la página de relaciones públicas.

## 3. Подписание по разделам| Раздел | Владелец | Чек‑лист |
|--------|----------|----------|
| Главная | Desarrollol | Hero‑копирайт отображается; Inicio rápido de tarjetas ведут на валидные маршруты; CTA‑кнопки работают. |
| Norito | Norito GT | Los dispositivos instalados en los robots actuales están conectados a las etiquetas CLI actuales y al esquema Norito. |
| SoraFS | Equipo de almacenamiento | Inicio rápido que incluye instrucciones, instrucciones y simulaciones de recuperación de datos. |
| SDK‑гайды | Nuevo SDK | Rust/Python/JS‑гайды conectan los primeros y posteriores repositorios actuales. |
| Referencia | Documentos/DevRel | Para cumplir con varias especificaciones, utilice el códec Norito y el codificador `norito.md`. |
| Vista previa‑артефакт | Documentos/DevRel | El artefacto `docs-portal-preview` se aplica a PR, protectores de humo, instalados públicamente en los refrigeradores. |
| Seguridad y Pruébelo en la zona de pruebas | Documentos/DevRel · Seguridad | Inicio de sesión con código de dispositivo OAuth (`DOCS_OAUTH_*`), lista de verificación `security-hardening.md`, archivos CSP/Tipos de confianza disponibles en `npm run build` `npm run probe:portal`. |

Отметьте каждую строку в ходе review PR’а или зафиксируйте follow-up-задачи, чтобы статус
оставался точным.

## 4. Notas de la versión- Utilice `https://docs.iroha.tech/` (o URL de configuración de la implementación del trabajo) en las notas de la versión.
  и статусные апдейты.
- Явно перечисляйте новые или изменённые разделы, чтобы downstream-comandы понимали,
  какие smoke‑testы и м требуется перезапустить.