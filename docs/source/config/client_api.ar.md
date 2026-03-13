---
lang: ar
direction: rtl
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## مرجع تكوين واجهة برمجة تطبيقات العميل

يتتبع هذا المستند مقابض التكوين التي تواجه العميل Torii
الأسطح من خلال `iroha_config::parameters::user::Torii`. القسم أدناه
يركز على عناصر التحكم في النقل Norito-RPC المقدمة لـ NRPC-1؛ المستقبل
يجب أن تقوم إعدادات واجهة برمجة تطبيقات العميل بتوسيع هذا الملف.

###`torii.transport.norito_rpc`

| مفتاح | اكتب | الافتراضي | الوصف |
|-----|------|--------|-------------|
| `enabled` | `bool` | `true` | مفتاح رئيسي يتيح فك التشفير الثنائي Norito. عندما يرفض `false`، Torii كل طلب Norito-RPC مع `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | طبقة الطرح: `disabled`، أو `canary`، أو `ga`. تقود المراحل قرارات القبول وإخراج `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | يفرض mTLS لنقل Norito-RPC: عندما يرفض Torii Torii طلبات Norito-RPC التي لا تحمل رأس علامة mTLS (على سبيل المثال، `X-Forwarded-Client-Cert`). تظهر العلامة عبر `/rpc/capabilities` حتى تتمكن مجموعات SDK من التحذير من البيئات التي تم تكوينها بشكل خاطئ. |
| `allowed_clients` | `array<string>` | `[]` | القائمة المسموح بها لطائر الكناري. عندما يكون `stage = "canary"`، يتم قبول الطلبات التي تحمل رأس `X-API-Token` الموجود في هذه القائمة فقط. |

تكوين المثال:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

دلالات المرحلة:

- **معطل** — Norito-RPC غير متاح حتى إذا كان `enabled = true`. العملاء
  تلقي `403 norito_rpc_disabled`.
- **canary** — يجب أن تتضمن الطلبات رأس `X-API-Token` الذي يطابق واحدًا
  من `allowed_clients`. تتلقى كافة الطلبات الأخرى `403
  norito_rpc_canary_denied`.
- **ga** — يتوفر Norito-RPC لكل متصل تمت مصادقته (يخضع لـ
  المعدل المعتاد وحدود ما قبل المصادقة).

يمكن للمشغلين تحديث هذه القيم ديناميكيًا من خلال `/v2/config`. كل تغيير
ينعكس على الفور في `/rpc/capabilities`، مما يسمح بمجموعات SDK وإمكانية المراقبة
لوحات المعلومات لإظهار وضع النقل المباشر.