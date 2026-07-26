---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرجع ترميز Norito

Norito está conectado a Iroha. Conexión en línea y carga útil Conexión API y conexión Norito Conexión de conexión a Internet اختلاف العتاد. هذه الصفحة تلخص الاجزاء المتحركة وتشير الى المواصفة الكاملة في `norito.md`.

## البنية الاساسية

| المكون | الغرض | المصدر |
| --- | --- | --- |
| **الرأس** | Incluye cargas útiles como magic/version/schema hash y CRC64 y otras funciones v1 incluye `VERSION_MINOR = 0x00` y los indicadores de encabezado están disponibles en el enlace (`0x00`). | `norito::header` — راجع `norito.md` ("Encabezado y banderas" ، جذر المستودع) |
| **Carga útil بدون رأس** | ترميز قيم حتمي يستخدم للـ hash/المقارنة. النقل on-wire يستخدم دائما رأسا؛ البايتات بدون رأس داخلية فقط. | `norito::codec::{Encode, Decode}` |
| **الضغط** | Zstd اختياري (وتسريع GPU تجريبي) يتم اختياره عبر بايت الضغط في الرأس. | `norito.md`, “Negociación de compresión” |

Este diseño de banderas (estructura empaquetada, secuencia empaquetada, conjunto de bits de campo, longitudes compactas) se basa en `norito::header::flags`. تستخدم V1 افتراضيا flags `0x00` لكنها تقبل flags صريحة ضمن القناع المدعوم؛ يتم رفض البتات غير المعروفة. يتم الاحتفاظ بـ `norito::header::Flags` للفحص الداخلي والنسخ المستقبلية.

## derivar

Aquí `norito_derive` contiene `Encode`, `Decode`, `IntoSchema` y JSON. اهم الاعراف:- المشتقات تولد مسارات AoS y empaquetado؛ v1 يستخدم تخطيط AoS افتراضيا (flags `0x00`) ما لم تختَر header flags متغيرات empaquetados. Este es el nombre de `crates/norito_derive/src/derive_struct.rs`.
- الميزات المؤثرة على التخطيط (`packed-struct`, `packed-seq`, `compact-len`) هي opt-in عبر banderas de encabezado y ترميزها/فك ترميزها بشكل متسق عبر compañeros.
- Configuración JSON (`norito::json`) Configuración JSON basada en el código API Norito. استخدم `norito::json::{to_json_pretty, from_json}` — y لا تستخدم `serde_json`.

## Multicodec وجداول المعرفات

Este es el multicódec Norito para `norito::multicodec`. الجدول المرجعي (hashes, انواع المفاتيح، واصفات payload) محفوظ في `multicodec.md` بجذر المستودع. عند اضافة معرف جديد:

1. Nombre `norito::multicodec::registry`.
2. وسع الجدول في `multicodec.md`.
3. Utilice enlaces posteriores (Python/Java) para crear enlaces.

## اعادة توليد documentos y accesorios

Para obtener más información sobre los precios de Markdown y para obtener información sobre los precios de Markdown:

- **Especificación**: `norito.md`
- **Multicodec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`

عندما تعمل اتوماتة Docusaurus, ، سيتم تحديث البوابة عبر سكربت sync (متابع في `docs/portal/scripts/`) الذي يسحب البيانات من هذه الملفات. حتى ذلك الحين، حافظ على مواءمة هذه الصفحة يدويا كلما تغيرت المواصفة.