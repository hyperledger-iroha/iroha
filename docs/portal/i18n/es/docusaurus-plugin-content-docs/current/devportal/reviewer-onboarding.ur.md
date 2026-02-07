---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ریویور آن بورڈنگ

## جائزہ

DOCS-SORA ڈویلپر پورٹل کے مرحلہ وار لانچ کو ٹریک کرتا ہے۔ compilaciones controladas por suma de comprobación
(`npm run serve`) اور مضبوط Pruébelo فلو اگلا سنگ میل کھولتے ہیں: پبلک پریویو کے وسیع پیمانے پر
کھلنے سے پہلے ویری فائیڈ ریویورز کی آن بورڈنگ۔ یہ گائیڈ بیان کرتا ہے کہ درخواستیں کیسے جمع کی جائیں،
اہلیت کیسے چیک کی جائے، رسائی کیسے دی جائے، اور شرکاء کو محفوظ طریقے سے آف بورڈ کیسے کیا جائے۔
کوهورٹ پلاننگ، دعوتی cadencia، اور ٹیلی میٹری exportaciones کے لئے
[vista previa del flujo de invitación](./preview-invite-flow.md) دیکھیں؛ نیچے کے مراحل اس پر فوکس کرتے ہیں
کہ ریویور منتخب ہونے کے بعد کون سی کارروائیاں کرنی ہیں۔

- **اسکوپ:** y ریویورز جنہیں GA سے پہلے vista previa de documentos (`docs-preview.sora`, compilaciones de GitHub Pages, paquetes SoraFS) درکار ہے۔
- **آؤٹ آف اسکوپ:** Torii یا SoraFS آپریٹرز (kits de incorporación de میں کور) اور پروڈکشن پورٹل implementaciones (دیکھیں [`devportal/deploy-guide`](./deploy-guide.md))۔

## رولز اور پری ریکویزٹس| رول | عام اہداف | مطلوبہ artefactos | نوٹس |
| --- | --- | --- | --- |
| Mantenedor principal | نئی گائیڈز ویری فائی کرنا، pruebas de humo چلانا۔ | Identificador de GitHub, contacto Matrix, CLA firmado فائل پر۔ | عموما پہلے سے `docs-preview` GitHub ٹیم میں ہوتا ہے؛ پھر بھی درخواست درج کریں تاکہ رسائی auditable رہے۔ |
| Revisor socio | پبلک ریلیز سے پہلے Fragmentos de SDK یا contenido de gobernanza ویری فائی کرنا۔ | Correo electrónico corporativo, POC legal, términos de vista previa firmados۔ | ٹیلی میٹری + requisitos de manejo de datos کی منظوری لازم ہے۔ |
| Voluntario comunitario | گائیڈز پر comentarios de usabilidad دینا۔ | Identificador de GitHub, contacto preferido, zona horaria, CoC قبولیت۔ | کوہورٹس چھوٹے رکھیں؛ acuerdo de colaborador سائن کرنے والے ریویورز کو ترجیح دیں۔ |

تمام ریویور اقسام کو لازمی ہے کہ:

1. vista previa de artefactos کے لئے uso aceptable پالیسی تسلیم کریں۔
2. apéndices de seguridad/observabilidad پڑھیں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Servicio de instantáneas کرنے سے پہلے `docs/portal/scripts/preview_verify.sh` چلانے پر رضامند ہوں۔

## Ingesta ورک فلو1. درخواست دینے والے سے
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   فارم بھرنے کو کہیں (یا edición میں copiar/pegar کریں)۔ کم از کم یہ ریکارڈ کریں: شناخت، رابطے کا طریقہ،
   Identificador de GitHub, fechas de revisión de fechas, documentos de seguridad پڑھنے کی تصدیق۔
2. درخواست کو `docs-preview` tracker (emisión de GitHub یا ticket de gobernanza) میں درج کریں اور aprobador asignar کریں۔
3. پری ریکویزٹس چیک کریں:
   - CLA / acuerdo de colaborador فائل پر موجود ہو (یا referencia del contrato de socio)۔
   - reconocimiento de uso aceptable درخواست میں محفوظ ہو۔
   - evaluación de riesgos مکمل ہو (مثال: socios revisores کو Legal نے aprobar کیا ہو)۔
4. Aprobador درخواست میں aprobación کرے اور problema de seguimiento کو کسی بھی entrada de gestión de cambios سے لنک کرے
   (مثال: `DOCS-SORA-Preview-####`) ۔

## Aprovisionamiento اور ٹولنگ

1. **Artifacts شیئر کریں** — Flujo de trabajo de CI یا SoraFS pin سے تازہ ترین descriptor de vista previa + archivo فراہم کریں
   (Artefacto `docs-portal-preview`) ۔ ریویورز کو یاد دلائیں کہ چلائیں:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Cumplimiento de la suma de verificación کے ساتھ servir کریں** — ریویورز کو کمانڈ کی طرف ریفر کریں controlado por la suma de verificación:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   یہ `scripts/serve-verified-preview.mjs` کو reutilización کرتا ہے تاکہ کوئی compilación no verificada غلطی سے لانچ نہ ہو۔

3. **GitHub رسائی دیں (اختیاری)** — اگر ریویورز کو ramas no publicadas چاہییں تو انہیں revisión مدت کے لئے
   `docs-preview` GitHub ٹیم میں شامل کریں اور membresía تبدیلی درخواست میں درج کریں۔4. **Canales de soporte شیئر کریں**: contacto de guardia (Matrix/Slack) y procedimiento de incidentes
   [`incident-runbooks`](./incident-runbooks.md) سے شیئر کریں۔

5. **Telemetría + retroalimentación** — ریویورز کو یاد دلائیں کہ análisis anónimos جمع کی جاتی ہے
   (دیکھیں [`observability`](./observability.md))۔ دعوت میں دیے گئے formulario de comentarios یا plantilla de problema فراہم کریں
   اور evento کو [`preview-feedback-log`](./preview-feedback-log) ayudante سے لاگ کریں تاکہ resumen de onda اپ ٹو ڈیٹ رہے۔

## ریویور چیک لسٹ

پریویو تک رسائی سے پہلے ریویورز کو یہ مکمل کرنا ہوگا:

1. los artefactos descargados verifican کریں (`preview_verify.sh`) ۔
2. `npm run serve` (یا `serve:verified`) سے پورٹل لانچ کریں تاکہ checksum guard فعال ہو۔
3. اوپر لنک کردہ seguridad y observabilidad نوٹس پڑھیں۔
4. Consola OAuth/Pruébelo کو inicio de sesión con código de dispositivo کے ذریعے ٹیسٹ کریں (اگر aplicable ہو) اور tokens de producción دوبارہ استعمال نہ کریں۔
5. Rastreador acordado میں hallazgos درج کریں (problema, documento compartido یا فارم) اور انہیں etiqueta de lanzamiento de vista previa سے etiqueta کریں۔

## Mantenedor ذمہ داریاں اور offboarding| مرحلہ | اقدامات |
| --- | --- |
| Inicio | یقینی بنائیں کہ lista de verificación de admisión درخواست کے ساتھ منسلک ہے، artefactos + ہدایات شیئر کریں، [`preview-feedback-log`](./preview-feedback-log) کے ذریعے `invite-sent` انٹری شامل کریں، اور اگر review ایک ہفتے سے زیادہ ہو تو sincronización de punto medio شیڈول کریں۔ |
| Monitoreo | vista previa de telemetría مانیٹر کریں (غیر معمولی Pruébelo ٹریفک، fallas de sonda) اور اگر کوئی مشکوک چیز ہو تو incidente runbook فالو کریں۔ hallazgos آنے پر `feedback-submitted`/`issue-opened` eventos لاگ کریں تاکہ métricas de onda درست رہیں۔ |
| Baja de embarque | عارضی GitHub یا SoraFS رسائی واپس لیں، `access-revoked` درج کریں، درخواست archive کریں (resumen de comentarios + acciones pendientes شامل کریں)، اور registro de revisores اپ ڈیٹ کریں۔ ریویور سے مقامی builds صاف کرنے کو کہیں اور [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) سے بنا digest منسلک کریں۔ |

ریویورز کو ondas کے درمیان rotar کرتے وقت بھی یہی عمل استعمال کریں۔ repositorio میں rastro de papel
(problema + plantillas) برقرار رکھنے سے DOCS-SORA auditable رہتا ہے اور gobernanza کو تصدیق میں مدد ملتی ہے
کہ vista previa de acceso a controles documentados کے مطابق تھا۔

## Plantillas de invitación y seguimiento- ہر divulgación کی شروعات
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  فائل سے کریں۔ یہ کم از کم قانونی زبان، suma de comprobación de vista previa ہدایات، اور اس توقع کو شامل کرتی ہے کہ
  ریویورز uso aceptable پالیسی تسلیم کریں۔
- plantilla ایڈٹ کرتے وقت `<preview_tag>`, `<request_ticket>` اور marcadores de posición de canales de contacto بدلیں۔
  mensaje final کی ایک کاپی ticket de admisión میں محفوظ کریں تاکہ ریویورز، aprobadores اور auditores
  بھیجے گئے الفاظ کا حوالہ دے سکیں۔
- دعوت بھیجنے کے بعد hoja de cálculo de seguimiento یا problema کو `invite_sent_at` marca de tiempo اور متوقع fecha de finalización کے ساتھ
  اپ ڈیٹ کریں تاکہ [vista previa del flujo de invitación](./preview-invite-flow.md) رپورٹ خودکار طور پر cohort اٹھا سکے۔