---
lang: ur
direction: rtl
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## کلائنٹ API کنفیگریشن حوالہ

یہ دستاویز Torii کلائنٹ کا سامنا کرنے والی کنفیگریشن نوبس کو ٹریک کرتی ہے جو ہیں
`iroha_config::parameters::user::Torii` کے ذریعے سطحیں۔ ذیل میں سیکشن
NRPC-1 کے لئے متعارف کروائے گئے Norito-RPC ٹرانسپورٹ کنٹرولز پر فوکس ؛ مستقبل
کلائنٹ API کی ترتیبات کو اس فائل میں توسیع کرنی چاہئے۔

### `torii.transport.norito_rpc`

| کلید | قسم | ڈیفالٹ | تفصیل |
| ----- | ------ | --------- | ------------- |
| `enabled` | `bool` | `true` | ماسٹر سوئچ جو بائنری Norito ضابطہ کشائی کو قابل بناتا ہے۔ جب `false` ، Torii `403 norito_rpc_disabled` کے ساتھ ہر Norito-RPC کی درخواست کو مسترد کرتا ہے۔ |
| `stage` | `string` | `"disabled"` | رول آؤٹ ٹائر: `disabled` ، `canary` ، یا `ga`۔ مراحل داخلہ کے فیصلے اور `/rpc/capabilities` آؤٹ پٹ کو چلاتے ہیں۔ |
| `require_mtls` | `bool` | `false` | Norito-RPC ٹرانسپورٹ کے لئے MTLs کا نفاذ کرتا ہے: جب `true` ، Torii Norito-RPC درخواستوں کو مسترد کرتا ہے جو MTLS مارکر ہیڈر نہیں رکھتے ہیں (جیسے `X-Forwarded-Client-Cert`)۔ پرچم `/rpc/capabilities` کے ذریعے منظر عام پر لایا گیا ہے تاکہ SDKs غلط کنفیگرڈ ماحول کے بارے میں متنبہ کرسکیں۔ |
| `allowed_clients` | `array<string>` | `[]` | کینری اجازت نامہ۔ جب `stage = "canary"` ، صرف اس فہرست میں موجود `X-API-Token` ہیڈر لے جانے والی درخواستوں کو قبول کیا جاتا ہے۔ |

مثال کی تشکیل:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

اسٹیج سیمنٹکس:

- ** غیر فعال **- Norito-rpc دستیاب نہیں ہے یہاں تک کہ اگر `enabled = true`۔ مؤکل
  `403 norito_rpc_disabled` وصول کریں۔
- ** کینری ** - درخواستوں میں ایک `X-API-Token` ہیڈر شامل ہونا ضروری ہے جو ایک سے مماثل ہے
  `allowed_clients` کا۔ دیگر تمام درخواستوں کو 3 403 موصول ہوتا ہے
  نوریٹو_ آر پی سی_کنری_ڈینیڈ`۔
- ** گا **- Norito-rpc ہر مستند کالر کے لئے دستیاب ہے (اس کے تابع
  معمول کی شرح اور پہلے سے پہلے کی حدیں)۔

آپریٹرز ان اقدار کو متحرک طور پر `/v2/config` کے ذریعے اپ ڈیٹ کرسکتے ہیں۔ ہر تبدیلی
`/rpc/capabilities` میں فوری طور پر جھلکتا ہے ، جس سے SDKs اور مشاہدہ کی اجازت ہوتی ہے
براہ راست ٹرانسپورٹ کرنسی کو ظاہر کرنے کے لئے ڈیش بورڈز۔