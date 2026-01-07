---
lang: ar
direction: rtl
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/space-directory.md (Space Directory Operator Playbook) -->

# دليل تشغيل Space Directory للمشغلين

يشرح هذا الدليل كيفية إعداد ونشر وتدقيق وتدوير إدخالات **Space Directory** الخاصة
بداتاسبaces Nexus. وهو يكمل ملاحظات التصميم في `docs/source/nexus.md` وخطة
onboarding لـ CBDC (`docs/source/cbdc_lane_playbook.md`) من خلال توفير إجراءات عملية
وـ fixtures ونماذج حوكمة جاهزة للاستخدام.

> **النطاق.** يعمل Space Directory كسجلّ كوني (canonical) لـ manifests الخاصة بالـ
> dataspace، وسياسات قدرات UAID (Universal Account ID) وسجل التدقيق (audit trail)
> الذي يعتمد عليه المنظمون. على الرغم من أن العقد الداعم ما زال قيد التطوير النشط
> (NX‑15)، فإن الـ fixtures والعمليات الواردة أدناه جاهزة للدمج في الأدوات واختبارات
> التكامل.

## 1. المفاهيم الأساسية

| المصطلح | الوصف | المراجع |
|--------|-------|---------|
| Dataspace | سياق تنفيذ/Lane يشغّل مجموعة عقود تمت الموافقة عليها من الحوكمة. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | نوع `UniversalAccountId` (هاش blake2b‑32) يُستخدم لربط صلاحيات cross‑dataspace. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | كائن `AssetPermissionManifest` يصف قواعد allow/deny حتمية لزوج UAID/dataspace (الـ deny يغلب). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | بيانات حوكمة + DA تُنشر بجانب manifests حتى يتمكن المشغلون من إعادة بناء مجموعات الـ validators، وقوائم composability المسموح بها، و hooks التدقيق. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | أحداث مشفّرة بـ Norito يتم إصدارها عند تفعيل/انتهاء/سحب manifests. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. دورة حياة manifest

يطبق Space Directory **إدارة دورة حياة مبنية على epoch**. كل تغيير ينتج bundle
manifest موقَّع بالإضافة إلى حدث:

| الحدث | المشغّل | الإجراءات المطلوبة |
|-------|---------|---------------------|
| `ManifestActivated` | وصول manifest جديد إلى `activation_epoch`. | بث الـ bundle، تحديث الـ caches، أرشفة قرار الحوكمة. |
| `ManifestExpired` | مرور `expiry_epoch` بدون تجديد. | إخطار المشغلين، تنظيف روابط UAID، تجهيز manifest بديل. |
| `ManifestRevoked` | قرار طارئ من نوع deny‑wins قبل انتهاء الصلاحية. | إلغاء الـ UAID فورًا، إصدار تقرير حادث، جدولة مراجعة حوكمة لاحقة. |

يجب على المشتركين استخدام `DataEventFilter::SpaceDirectory` لمراقبة dataspaces أو
UAIDs بعينها. مثال بالـ Rust:

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. سير عمل المشغل

| المرحلة | المالك(ون) | الخطوات | الأدلة |
|---------|-----------|---------|--------|
| Draft | مالك الـ dataspace | استنساخ fixture، تعديل صلاحيات/حوكمة، تشغيل `cargo test -p iroha_data_model nexus::manifest`. | فرق Git، سجل الاختبارات. |
| Review | مجموعة الحوكمة | التحقق من JSON الخاص بالـ manifest + بايتات Norito، وتوقيع سجل القرار. | محضر موقَّع، هاش manifest (BLAKE3 + Norito `.to`). |
| Publish | مشغلو الـ lane | الإرسال عبر CLI (`iroha space-directory manifest publish`) باستخدام payload Norito `.to` أو JSON خام **أو** إرسال POST إلى `/v1/space-directory/manifests` مع JSON للـ manifest + سبب اختياري، ثم التحقق من استجابة Torii والتقاط `SpaceDirectoryEvent`. | إيصال CLI/Torii، سجل الأحداث. |
| Expire | مشغلو الـ lane / الحوكمة | تشغيل `iroha space-directory manifest expire` (UAID، dataspace، epoch) عندما يصل manifest لنهاية عمره، التحقق من `SpaceDirectoryEvent::ManifestExpired`، وأرشفة دلائل تنظيف الـ bindings. | مخرجات CLI، سجل الأحداث. |
| Revoke | الحوكمة + مشغلو الـ lane | تشغيل `iroha space-directory manifest revoke` (UAID، dataspace، epoch، reason) **أو** إرسال POST إلى `/v1/space-directory/manifests/revoke` بالـ payload نفسه إلى Torii، التحقق من `SpaceDirectoryEvent::ManifestRevoked` وتحديث حزمة الأدلة. | إيصال CLI/Torii، سجل الأحداث، ملاحظة في التذكرة. |
| Monitor | SRE/الامتثال | مراقبة الـ telemetry + سجلات التدقيق، وضبط تنبيهات لحالات revocation/expiry. | لقطة شاشة من Grafana، سجلات مؤرشفة. |
| Rotate/Revoke | مشغلو الـ lane + الحوكمة | تجهيز manifest بديل (epoch جديد)، إجراء تمرين tabletop، فتح incident (في حالة revoke). | تذكرة روتेशन، تقرير post‑mortem. |

يجب حفظ جميع الأدوات والأدلة (artefacts) الخاصة بأي rollout ضمن
`artifacts/nexus/<dataspace>/<timestamp>/` مع manifest checksums لتلبية طلبات الأدلة من
المنظمين بسهولة.

## 4. قالب manifest والـ fixtures

استخدم الـ fixtures المنسَّقة كمرجع قياسي للمخطط (schema). مثال CBDC wholesale
(`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) يحتوي على إدخالات
allow و deny:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

قاعدة deny‑wins نموذجية في سياق onboarding لـ CBDC قد تبدو كما يلي:

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

عند نشر manifest، تصبح مجموعة `uaid` + `dataspace` هي المفتاح الأساسي لسجل
الصلاحيات. يتم ربط manifests اللاحقة عبر `activation_epoch`، ويضمن السجل دائمًا أن
أحدث نسخة مفعَّلة تلتزم بدلالات “deny يفوز”.

## 5. واجهة Torii

### نشر manifest

يمكن للمشغلين نشر manifests مباشرة عبر Torii من دون الاعتماد على CLI.

```
POST /v1/space-directory/manifests
```

| الحقل | النوع | الوصف |
|-------|-------|--------|
| `authority` | `AccountId` | الحساب الذي يوقّع معاملة النشر. |
| `private_key` | `ExposedPrivateKey` | مفتاح خاص مشفّر بـ base64 يستخدمه Torii للتوقيع بالنيابة عن `authority`. |
| `manifest` | `SpaceDirectoryManifest` | manifest كامل بصيغة JSON سيتم ترميزه إلى Norito داخليًا. |
| `reason` | `Option<String>` | رسالة تدقيق اختيارية تُخزَّن مع بيانات دورة الحياة. |

مثال على جسم JSON:

```jsonc
{
  "authority": "ops@cbdc",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

يرد Torii بـ `202 Accepted` بمجرد دخول المعاملة إلى قائمة الانتظار. عند تنفيذ الكتلة
يتم إصدار `SpaceDirectoryEvent::ManifestActivated` (وفقًا لـ `activation_epoch`)،
وتُعاد بناء الـ bindings تلقائيًا، ويبدأ endpoint جرد manifests في إظهار الـ payload
الجديد. ضوابط الوصول (Access controls) مماثلة لبقية واجهات الكتابة في Space
Directory (بوابات CIDR / token / سياسة الرسوم).

### واجهة سحب manifest

لم تعد revocations الطارئة تتطلب استدعاء CLI: يمكن للمشغلين إرسال طلب POST مباشرة
إلى Torii لإضافة التعليمات القياسية `RevokeSpaceDirectoryManifest` إلى قائمة
الانتظار. يجب أن يمتلك الحساب المرسل صلاحية
`CanPublishSpaceDirectoryManifest { dataspace }`، كما في workflow الخاص بالـ CLI.

```
POST /v1/space-directory/manifests/revoke
```

| الحقل | النوع | الوصف |
|-------|-------|--------|
| `authority` | `AccountId` | الحساب الذي يوقّع معاملة السحب (revocation). |
| `private_key` | `ExposedPrivateKey` | مفتاح خاص base64 يستخدمه Torii للتوقيع بالنيابة عن `authority`. |
| `uaid` | `String` | قيمة UAID (`uaid:<hex>` أو digest hex مكون من 64 خانة، LSB=1). |
| `dataspace` | `u64` | معرف dataspace الذي يستضيف manifest. |
| `revoked_epoch` | `u64` | الـ epoch (شاملاً) الذي ينبغي أن يبدأ عنده السحب. |
| `reason` | `Option<String>` | رسالة تدقيق اختيارية تُخزَّن مع بيانات دورة الحياة. |

مثال على جسم JSON:

```jsonc
{
  "authority": "ops@cbdc",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

يرجع Torii القيمة `202 Accepted` عند دخول المعاملة إلى قائمة الانتظار. بعد تنفيذ
الكتلة ستستلم `SpaceDirectoryEvent::ManifestRevoked`، وسيُعاد بناء
`uaid_dataspaces` تلقائيًا، كما ستبدأ نقاط النهاية `/portfolio` وجرد manifests في
عرض حالة الإلغاء (revoked) فورًا. بوابات CIDR/سياسة الرسوم مماثلة لنقاط القراءة.

## 6. قالب ملف تعريف dataspace

تجمع الـ profiles كل ما يحتاجه validator جديد قبل الاتصال. Fixture
`profile/cbdc_lane_profile.json` يوضح:

- مصدر الحوكمة/الكورم (`parliament@cbdc` + معرف تذكرة الأدلة).
- مجموعة الـ validators + الكورم والمساحات المحمية (`cbdc`, `gov`).
- ملف DA (الفئة A، قائمة الشهود (attesters)، وتيرة التدوير).
- معرف مجموعة composability والـ whitelist التي تربط UAIDs مع capability manifests.
- Hooks التدقيق (قائمة الأحداث، مخطط السجلات، خدمة PagerDuty).

استخدم هذا JSON كنقطة انطلاق لـ dataspaces جديدة، ثم حدّث مسارات الـ whitelist
للإشارة إلى capability manifests ذات الصلة.

## 7. النشر والتدوير

1. **ترميز UAID.** استخرج digest من نوع blake2b‑32 وأضف بادئة `uaid:`:

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **ترميز الـ payload الخاص بـ Norito.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   أو شغّل أداة CLI المساعدة (تكتب ملف `.to` وملف `.hash` يحتوي على digest من نوع
   BLAKE3‑256):

   ```bash
   iroha space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **النشر عبر Torii.**

   ```bash
   # إذا كنت قد شفّرت manifest مسبقًا إلى Norito:
   iroha space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   أو استخدم واجهة HTTP الموصوفة أعلاه. في كلا الحالتين، تأكد من أرشفة:
   - الـ JSON الأصلي للـ manifest؛
   - payload الـ Norito `.to` والهاش BLAKE3؛
   - استجابة Torii والحدث `SpaceDirectoryEvent`.

4. **التدوير أو السحب.**

   - من أجل تدوير مخطّط: جهّز manifest بديل يحتوي على `activation_epoch` مستقبليًا،
     نفّذ تمرين tabletop، ونسّق التفعيل مع مشغلي جميع dataspaces المتأثرة.
   - من أجل سحب طارئ: اتّبع playbook السحب (قسم “Revoke” في جدول سير العمل)، تضمّن
     معرف الحادث في `reason` واحتفظ بالسجلات والـ snapshots ذات الصلة.

باتّباع هذا الدليل، يوفّر Space Directory سجلًا قانونيًا، قابلًا للتحقق، ومناسبًا
للمُنظِّمين حول كيفية منح وتقييد وسحب القدرات عبر جميع dataspaces في Nexus.

</div>
