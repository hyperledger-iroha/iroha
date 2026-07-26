---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# قائمة التحقق للنشر

استخدم قائمة التحقق هذه كلما قمت بتحديث بوابة المطورين. Hay CI y GitHub Pages y humo.

## 1. التحقق المحلي

- `npm run sync-openapi -- --version=current --latest` (es decir, `--mirror=<label>` es compatible con Torii OpenAPI para conectar).
- `npm run build` – تحقق من ان نص الـ hero `Build on Iroha with confidence` ما زال يظهر في `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – تحقق من بيان sumas de comprobación (اضف `--descriptor`/`--archive` عند اختبار artefactos محملة من CI).
- `npm run serve` – Vista previa de la suma de comprobación de la suma de comprobación de la suma de comprobación `docusaurus serve`, حتى لا يتصفح المراجعون لقطة غير موقعة (يبقى alias `serve:verified` متاحا للاستدعاءات الصريحة).
- راجع الـ markdown الذي لمسته عبر `npm run start` y recarga en vivo.

## 2. فحوصات طلب السحب

- Coloque el cable `docs-portal-build` en `.github/workflows/check-docs.yml`.
- تاكد من تشغيل `ci/check_docs_portal.sh` (prueba de humo de héroe CI تظهر).
- تاكد ان سير عمل vista previa رفع بيانا (`build/checksums.sha256`) y سكربت التحقق من vista previa نجح (تظهر السجلات خرج `scripts/preview_verify.sh`).
- Esta es una URL para acceder a páginas de GitHub y relaciones públicas.

## 3. اعتماد الاقسام| القسم | المالك | قائمة التحقق |
|---------|-------|-----------|
| Página de inicio | Desarrollol | Aquí está el héroe, el inicio rápido, las aplicaciones de inicio rápido y las llamadas a la acción (CTA). |
| Norito | Norito GT | Para obtener información general y comenzar, use flags en la CLI y en Norito. |
| SoraFS | Equipo de almacenamiento | Este es el inicio rápido, el inicio rápido y el inicio rápido del manifiesto y el inicio de búsqueda. |
| Guías SDK | Nuevo SDK | Utilice Rust/Python/JS para crear aplicaciones de software y aplicaciones. |
| Referencia | Documentos/DevRel | Aquí están las especificaciones y el códec Norito y `norito.md`. |
| Vista previa del artefacto | Documentos/DevRel | Hay un artefacto `docs-portal-preview` que contiene humo y humo y humo. |
| Seguridad y Pruébelo en la zona de pruebas | Documentos/DevRel · Seguridad | Para iniciar sesión con código de dispositivo OAuth (`DOCS_OAUTH_*`), y para usar `security-hardening.md`, y para usar CSP/Trusted Types en `npm run build` y `npm run probe:portal`. |

علّم كل صف كجزء من مراجعة PR، او دوّن اي مهام متابعة حتى يبقى تتبع الحالة دقيقا.

## 4. ملاحظات الاصدار

- Use `https://docs.iroha.tech/` (او عنوان بيئة النشر من مهمة النشر) في ملاحظات الاصدار وتحديثات الحالة.
- اذكر اي اقسام جديدة او معدلة بوضوح حتى تعرف الفرق التابعة اين تعيد تشغيل اختبارات smoke الخاصة بها.