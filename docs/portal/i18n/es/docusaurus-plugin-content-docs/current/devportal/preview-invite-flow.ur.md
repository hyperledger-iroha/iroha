---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوتی فلو

## مقصد

روڈ میپ آئٹم **DOCS-SORA** ریویور آن بورڈنگ اور پبلک پریویو دعوتی پروگرام کو وہ آخری رکاوٹیں قرار دیتا ہے جن کے بعد پورٹل بیٹا سے باہر جا سکتا ہے۔ یہ صفحہ بیان کرتا ہے کہ ہر دعوتی ویو کیسے کھولی جائے، کون سے artefactos دعوتیں بھیجنے سے پہلے لازمی ہیں، اور فلو کی auditabilidad کیسے ثابت کی جائے۔ اسے ساتھ استعمال کریں:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) ہر ریویور کی ہینڈلنگ کے لئے۔
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) suma de comprobación ضمانتوں کے لئے۔
- [`devportal/observability`](./observability.md) ٹیلی میٹری exportaciones اور ganchos de alerta کے لئے۔

## ویو پلان| ویو | سامعین | انٹری معیار | ایگزٹ معیار | نوٹس |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Los mantenedores de Docs/SDK deben validar el primer día | GitHub ٹیم `docs-portal-preview` آباد ہو، `npm run serve` checksum gate سبز ہو، Alertmanager 7 días رہے۔ | تمام Documentos P0 ریویو، acumulación ٹیگ شدہ، کوئی incidente de bloqueo نہ ہو۔ | فلو validar کرنے کے لئے؛ دعوتی ای میل نہیں، صرف vista previa de artefactos شیئر کریں۔ |
| **W1 - Socios** | SoraFS integradores Torii, NDA y revisores de gobernanza | W0 ختم، قانونی شرائط منظور، Prueba de preparación del proxy Try-it پر۔ | پارٹنر aprobación جمع (problema یا formulario firmado)، ٹیلی میٹری میں =2 versiones de documentación canal de vista previa سے بغیر rollback گزر چکی ہوں۔ | invitaciones simultáneas محدود (<=25) اور ہفتہ وار بیچ۔ |

`status.md` Rastreador de solicitudes de vista previa میں فعال ویو درج کریں تاکہ gobernancia فوری طور پر پروگرام کی حالت دیکھ سکے۔

## Lista de verificación previa al vuelo

ان اقدامات کو **دعوتیں شیڈول کرنے سے پہلے** مکمل کریں:1. **Artefactos de CI دستیاب**
   - تازہ ترین `docs-portal-preview` + descriptor `.github/workflows/docs-portal-preview.yml` کے ذریعے اپ لوڈ ہو۔
   - SoraFS pin `docs/portal/docs/devportal/deploy-guide.md` میں نوٹ ہو (descriptor de corte موجود ہو).
2. **Cumplimiento de la suma de verificación**
   - `docs/portal/scripts/serve-verified-preview.mjs` `npm run serve` کے ذریعے invocar ہو۔
   - `scripts/preview_verify.sh` ہدایات macOS + Linux پر ٹیسٹ ہوں۔
3. **Línea base de telemetría**
   - `dashboards/grafana/docs_portal.json` صحت مند Pruébelo ٹریفک دکھائے اور `docs.preview.integrity` الرٹ سبز ہو۔
   - `docs/portal/docs/devportal/observability.md` کا تازہ apéndice Grafana لنکس کے ساتھ اپ ڈیٹ ہو۔
4. **Artefactos de gobernanza**
   - problema del rastreador de invitaciones تیار ہو (ہر ویو کے لئے ایک problema). 
   - plantilla de registro de revisor کاپی ہو (دیکھیں [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Problema de aprobaciones de SRE قانونی اور کے ساتھ منسلک ہوں۔

دعوت بھیجنے سے پہلے rastreador de invitaciones میں verificación previa مکمل ہونے کا اندراج کریں۔

## فلو کے مراحل1. **امیدوار منتخب کریں**
   - ویٹ لسٹ شیٹ یا پارٹنر کیو سے نکالیں۔
   - ہر امیدوار کے پاس مکمل plantilla de solicitud ہونا یقینی بنائیں۔
2. **رسائی کی منظوری**
   - problema del rastreador de invitaciones پر aprobador اسائن کریں۔
   - requisitos previos چیک کریں (CLA/contrato, uso aceptable, resumen de seguridad).
3. **دعوتیں ارسال کریں**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے marcadores de posición (`<preview_tag>`, `<request_ticket>`, contactos) بھریں۔
   - descriptor + hash de archivo, pruébalo URL provisional, canales de soporte منسلک کریں۔
   - Edición de فائنل ای میل (یا Matrix/Slack transcript) میں محفوظ کریں۔
4. **Incorporación ٹریک کریں**
   - rastreador de invitaciones کو `invite_sent_at`, `expected_exit_at`, y estado (`pending`, `active`, `complete`, `revoked`) کے ساتھ اپ ڈیٹ کریں۔
   - auditabilidad کے لئے solicitud de admisión del revisor کو لنک کریں۔
5. **Telemetría مانیٹر کریں**
   - `docs.preview.session_active` اور `TryItProxyErrors` alertas پر نظر رکھیں۔
   - اگر ٹیلی میٹری línea base سے ہٹے تو incidente کھولیں اور نتیجہ entrada de invitación کے ساتھ نوٹ کریں۔
6. **فیڈبیک جمع کریں اور خارج ہوں**
   - فیڈبیک آنے پر یا `expected_exit_at` گزرنے پر دعوتیں بند کریں۔
   - اگلی cohorte پر جانے سے پہلے ویو problema میں مختصر خلاصہ (hallazgos, incidentes, próximas acciones) اپ ڈیٹ کریں۔

## Pruebas e informes| Artefacto | کہاں محفوظ کریں | اپ ڈیٹ cadencia |
| --- | --- | --- |
| problema con el rastreador de invitaciones | GitHub پروجیکٹ `docs-portal-preview` | ہر دعوت کے بعد اپ ڈیٹ کریں۔ |
| exportación de lista de revisores | `docs/portal/docs/devportal/reviewer-onboarding.md` Registro vinculado میں | ہفتہ وار۔ |
| instantáneas de telemetría | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilización del paquete de telemetría کریں) | ہر ویو + incidentes کے بعد۔ |
| resumen de comentarios | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ہر ویو کیلئے فولڈر بنائیں) | ویو salir کے 5 دن کے اندر۔ |
| nota de la reunión de gobernanza | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | ہر DOCS-SORA sincronización de gobierno سے پہلے بھریں۔ |

ہر بیچ کے بعد `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
چلائیں تاکہ مشین ریڈایبل resumen بنے۔ Problema con JSON کی تصدیق کر سکیں۔

ہر ویو ختم ہونے پر evidencia کی فہرست `status.md` کے ساتھ منسلک کریں تاکہ روڈ میپ انٹری جلدی اپ ڈیٹ ہو سکے۔

## Revertir o pausar

جب درج ذیل میں سے کوئی ہو تو دعوتی فلو روک دیں (اور gobernanza کو مطلع کریں):

- Pruébelo incidente de proxy جس میں rollback کرنا پڑا (`npm run manage:tryit-proxy`).
- Fatiga de alerta: 7 دن کے اندر puntos finales de solo vista previa کے لئے >3 páginas de alerta.
- Brecha de cumplimiento: دعوت بغیر términos firmados یا plantilla de solicitud لاگ کئے بھیجی گئی۔
- Riesgo de integridad: `scripts/preview_verify.sh` Falta de coincidencia en la suma de comprobación پکڑا گیا۔

rastreador de invitaciones میں remediación دستاویز کرنے اور کم از کم 48 گھنٹے تک ٹیلی میٹری ڈیش بورڈ مستحکم ہونے کی تصدیق کے بعد ہی دوبارہ شروع کریں۔