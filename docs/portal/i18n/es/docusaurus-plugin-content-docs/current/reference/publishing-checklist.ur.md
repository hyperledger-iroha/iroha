---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificación de publicación

جب بھی آپ portal de desarrolladores اپڈیٹ کریں تو اس lista de verificación کا استعمال کریں۔ یہ یقینی بناتی ہے کہ compilación de CI, implementación de páginas de GitHub, اور دستی pruebas de humo ہر سیکشن کو کور کریں اس سے پہلے کہ کوئی lanzamiento یا hito de la hoja de ruta آئے۔

## 1. Validación local

- `npm run sync-openapi -- --version=current --latest` (Torii OpenAPI بدلنے پر ایک یا زیادہ `--mirror=<label>` flags شامل کریں تاکہ ایک instantánea congelada بنے).
- `npm run build` – تصدیق کریں کہ `Build on Iroha with confidence` copia de héroe اب بھی `build/index.html` میں موجود ہے۔
- `./docs/portal/scripts/preview_verify.sh --build-dir build`: manifiesto de suma de comprobación verificar کریں (artefactos de CI descargados ٹیسٹ کرتے وقت `--descriptor`/`--archive` شامل کریں).
- `npm run serve` – ayudante de vista previa controlada por suma de comprobación لانچ کرتا ہے جو `docusaurus serve` کو کال کرنے سے پہلے manifiesto verificar کرتا ہے، تاکہ revisores کبھی instantánea sin firmar نہ براؤز کریں (`serve:verified` alias واضح کالز کیلئے موجود رہتا ہے).
- `npm run start` اور servidor de recarga en vivo کے ذریعے اس markdown کو spot-check کریں جسے آپ نے touch کیا ہے۔

## 2. Verificaciones de solicitud de extracción

- `.github/workflows/check-docs.yml` میں `docs-portal-build` trabajo کی کامیابی verificar کریں۔
- تصدیق کریں کہ `ci/check_docs_portal.sh` چلا (CI registra la verificación de humo del héroe دکھائی دیتا ہے)۔
- یقینی بنائیں کہ vista previa del flujo de trabajo نے manifiesto (`build/checksums.sha256`) carga کیا اور vista previa del script de verificación کامیاب ہوا (registros CI میں `scripts/preview_verify.sh` salida دکھائی دیتا ہے)۔
- Entorno de páginas de GitHub کی URL de vista previa publicada کو Descripción de relaciones públicas میں شامل کریں۔## 3. Aprobación de sección

| Sección | Propietario | Lista de verificación |
|---------|-------|-----------|
| Página de inicio | Desarrollol | Copia de héroe renderizado ہو، tarjetas de inicio rápido rutas válidas پر جائیں، Los botones CTA resuelven ہوں۔ |
| Norito | Norito GT | Descripción general, guías de introducción, indicadores de CLI, documentos de esquema Norito, referencia |
| SoraFS | Equipo de almacenamiento | Inicio rápido مکمل ہو، campos del informe de manifiesto documentados ہوں، buscar instrucciones de simulación verificar ہوں۔ |
| Guías SDK | Clientes potenciales del SDK | Guías de Rust/Python/JS Ejemplos de compilación کریں اور repositorios en vivo سے enlace ہوں۔ |
| Referencia | Documentos/DevRel | Índice تازہ ترین especificaciones دکھائے، Norito referencia del códec `norito.md` سے coincidencia کرے۔ |
| Vista previa del artefacto | Documentos/DevRel | `docs-portal-preview` artefacto PR کے ساتھ adjuntar ہو، controles de humo pasan ہوں، enlaces revisores کے ساتھ compartir ہو۔ |
| Seguridad y Pruébelo en la zona de pruebas | Documentos/DevRel · Seguridad | Configuración de inicio de sesión con código de dispositivo OAuth ہو (`DOCS_OAUTH_*`), `security-hardening.md` lista de verificación ejecutar ہو، encabezados CSP/Tipos de confianza `npm run build` یا `npm run probe:portal` سے verificar ہوں۔ |

ہر fila کو اپنے Revisión de relaciones públicas کا حصہ بنائیں، یا tareas de seguimiento لکھیں تاکہ seguimiento de estado درست رہے۔

## 4. Notas de la versión- `https://docs.iroha.tech/` (trabajo de implementación y URL del entorno) y notas de la versión y actualizaciones de estado.
- نئے یا تبدیل شدہ secciones کو واضح طور پر بیان کریں تاکہ equipos posteriores جان سکیں کہ انہیں اپنے pruebas de humo کہاں دوبارہ چلانے ہیں۔