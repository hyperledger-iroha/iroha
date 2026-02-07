---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پبلک پریویو دعوتی پلے بک

## پروگرام کے مقاصد

یہ پلے بک وضاحت کرتی ہے کہ ریویور آن بورڈنگ ورک فلو فعال ہونے کے بعد پبلک پریویو کیسے اعلان اور چلایا جائے۔
یہ DOCS-SORA روڈمیپ کو دیانت دار رکھتی ہے کیونکہ ہر دعوت کے ساتھ قابل تصدیق artefactos, سیکیورٹی رہنمائی،
اور واضح comentarios راستہ شامل ہونا یقینی بنایا جاتا ہے۔

- **آڈیئنس:** کمیونٹی ممبرز، پارٹنرز اور mantenedores کی curado فہرست جنہوں نے vista previa de uso aceptable پالیسی سائن کی ہے۔
- **سیلنگز:** tamaño de onda predeterminado <= 25 días de ventana de acceso, 24 horas de respuesta a incidentes

## لانچ گیٹ چیک لسٹ

کوئی بھی دعوت بھیجنے سے پہلے یہ کام مکمل کریں:

1. تازہ ترین vista previa de artefactos CI میں اپلوڈ ہوں (`docs-portal-preview`,
   manifiesto de suma de comprobación, descriptor, paquete SoraFS)۔
2. `npm run --prefix docs/portal serve` (controlada por suma de comprobación) اسی etiqueta پر ٹیسٹ کیا گیا ہو۔
3. ریویور آن بورڈنگ ٹکٹس aprobar ہوں اور invitar ola سے لنک ہوں۔
4. سیکیورٹی، observabilidad, اور incidente ڈاکس validar ہوں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))۔
5. Comentarios فارم یا plantilla de problemas تیار ہو (gravedad, pasos de reproducción, capturas de pantalla, اور información del entorno کے فیلڈز شامل ہوں)۔
6. اعلان کی کاپی Docs/DevRel + Governance نے ریویو کی ہو۔

## دعوتی پیکیج

ہر دعوت میں شامل ہونا چاہیے:1. **Artefactos verificados** — Manifiesto/plan SoraFS یا Artefacto de GitHub کے لنکس دیں،
   ساتھ میں manifiesto de suma de comprobación اور descriptor بھی دیں۔ verificación کمانڈ واضح طور پر لکھیں تاکہ
   ریویورز sitio لانچ کرنے سے پہلے اسے چلا سکیں۔
2. **Instrucciones de publicación**: vista previa controlada por suma de verificación کمانڈ شامل کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Recordatorios de seguridad** — واضح کریں کہ tokens خود بخود caducan ہوتے ہیں، لنکس شیئر نہیں کیے جائیں،
   اور incidentes فوراً رپورٹ کیے جائیں۔
4. **Canal de comentarios** — plantilla/formulario del problema لنک کریں اور expectativas de tiempo de respuesta واضح کریں۔
5. **Fechas del programa**: fechas de inicio/finalización, horario de oficina, sincronizaciones, ventana de actualización فراہم کریں۔

نمونہ ای میل
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
میں دستیاب ہے اور یہ requisitos پوری کرتا ہے۔ بھیجنے سے پہلے marcadores de posición (fechas, URL, contactos)
اپ ڈیٹ کریں۔

## پریویو host کو exponer کریں

جب تک incorporación مکمل نہ ہو اور cambiar ticket منظور نہ ہو تب تک vista previa del anfitrión کو promoción نہ کریں۔
اس سیکشن کے compilar/publicar/verificar los pasos de un extremo a otro کے لئے
[vista previa de la guía de exposición del host](./preview-host-exposure.md) دیکھیں۔

1. **Construir اور پیکیجنگ:** sello de etiqueta de lanzamiento کریں اور artefactos deterministas تیار کریں۔

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

   secuencia de comandos pin `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   اور `portal.dns-cutover.json` کو `artifacts/sorafs/` میں لکھتا ہے۔ ان فائلوں کو ola de invitación
   کے ساتھ adjuntar کریں تاکہ ہر ریویور وہی bits verificar کر سکے۔2. **Vista previa de alias de publicación کریں:** کمانڈ کو `--skip-submit` کے بغیر دوبارہ چلائیں
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` una prueba de alias emitida por el gobierno فراہم کریں)۔
   اسکرپٹ `docs-preview.sora` پر enlace manifiesto کرے گا اور paquete de evidencia کے لئے
   `portal.manifest.submit.summary.json` اور `portal.pin.report.json` نکالے گا۔

3. **Sonda de implementación کریں:** invita بھیجنے سے پہلے alias resolver ہونا اور suma de comprobación کا etiqueta سے coincidencia ہونا
   یقینی بنائیں۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) کو respaldo کے طور پر práctico رکھیں تاکہ
   Borde de vista previa میں مسئلہ ہو تو ریویورز لوکل کاپی چلا سکیں۔

## کمیونیکیشن ٹائم لائن

| دن | ایکشن | Propietario |
| --- | --- | --- |
| D-3 | دعوتی کاپی finalizar کرنا، artefactos actualizar کرنا، verificación کا simulacro | Documentos/DevRel |
| D-2 | Aprobación de gobernanza + ticket de cambio | Documentos/DevRel + Gobernanza |
| D-1 | plantilla کے ذریعے دعوتیں بھیجیں، rastreador میں lista de destinatarios اپ ڈیٹ کریں | Documentos/DevRel |
| D | Llamada inicial/horario de oficina, paneles de telemetría مانیٹر کریں | Documentos/DevRel + De guardia |
| D+7 | resumen de retroalimentación de punto medio, problemas de bloqueo y clasificación | Documentos/DevRel |
| D+14 | onda بند کریں، عارضی رسائی revocar کریں، `status.md` میں خلاصہ شائع کریں | Documentos/DevRel |

## Seguimiento de acceso y telemetría1. ہر destinatario, marca de tiempo de invitación, اور fecha de revocación کو registro de comentarios de vista previa کے ساتھ ریکارڈ کریں
   (دیکھیں [`preview-feedback-log`](./preview-feedback-log)) تاکہ ہر ola ایک ہی rastro de evidencia شیئر کرے:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Eventos admitidos ہیں `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, y `access-revoked`۔ iniciar sesión ڈیفالٹ طور پر
   `artifacts/docs_portal_preview/feedback_log.json` Número de modelo ہے؛ اسے ola de invitación ٹکٹ کے ساتھ
   formularios de consentimiento سمیت adjuntar کریں۔ cierre نوٹ سے پہلے ayudante de resumen استعمال کریں تاکہ
   Este es un resumen auditable:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   resumen JSON ہر wave کے invitaciones, کھلے destinatarios, comentarios cuenta, اور حالیہ ترین evento کے
   marca de tiempo کو enumerar کرتا ہے۔ ayudante
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   پر مبنی ہے، اس لئے وہی flujo de trabajo لوکل یا CI میں چل سکتا ہے۔ resumen شائع کرتے وقت
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Plantilla de resumen استعمال کریں۔
2. paneles de telemetría کو wave میں استعمال ہونے والے `DOCS_RELEASE_TAG` کے ساتھ tag کریں تاکہ
   picos کو invitar cohortes سے correlacionar کیا جا سکے۔
3. implementar کے بعد `npm run probe:portal -- --expect-release=<tag>` چلائیں تاکہ entorno de vista previa
   درست publicar metadatos anunciar کرے۔
4. کسی بھی incidente کو plantilla de runbook میں captura کریں اور اسے cohorte سے enlace کریں۔

## Comentarios y cierre1. comentarios کو documento compartido یا tablero de problemas میں جمع کریں۔ artículos کو `docs-preview/<wave>` سے etiqueta کریں تاکہ
   propietarios de la hoja de ruta انہیں آسانی سے consulta کر سکیں۔
2. Vista previa del registrador کی salida resumida سے informe de onda بھریں، پھر cohort کو `status.md` میں resumen کریں
   (participantes, hallazgos, correcciones planificadas) اور اگر DOCS-SORA hito بدلا ہو تو `roadmap.md` اپ ڈیٹ کریں۔
3. [`reviewer-onboarding`](./reviewer-onboarding.md) کے los pasos de baja siguen کریں: acceso revocar کریں،
   archivo de solicitudes کریں، اور participantes کا شکریہ ادا کریں۔
4. اگلی wave کے لئے artefactos actualizar کریں، checksum gates دوبارہ چلائیں، اور plantilla de invitación کو نئی fechas سے اپ ڈیٹ کریں۔

اس playbook کو مسلسل لاگو کرنے سے vista previa پروگرام auditable رہتا ہے اور Docs/DevRel کو دعوتیں
اسکیل کرنے کا repetible طریقہ ملتا ہے جیسے جیسے پورٹل GA کے قریب آتا ہے۔