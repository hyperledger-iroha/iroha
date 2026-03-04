---
lang: ar
direction: rtl
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# مخطط مقترح مجلس جدول الأعمال (MINFO-2a)

مرجع خريطة الطريق: **MINFO-2a — أداة التحقق من تنسيق الاقتراح.**

يقوم سير عمل مجلس الأجندة بدمج القائمة السوداء المقدمة من المواطنين وتغيير السياسة
المقترحات قبل أن تقوم لجان الحوكمة بمراجعتها. تحدد هذه الوثيقة
مخطط الحمولة الأساسي ومتطلبات الأدلة وقواعد اكتشاف التكرار
يستهلكها المدقق الجديد (`cargo xtask ministry-agenda validate`) لذلك
يمكن لمقدمي العروض فحص عمليات إرسال JSON محليًا قبل تحميلها على البوابة الإلكترونية.

## نظرة عامة على الحمولة

تستخدم مقترحات جدول الأعمال المخطط `AgendaProposalV1` Norito
(`iroha_data_model::ministry::AgendaProposalV1`). يتم ترميز الحقول كـ JSON متى
التقديم من خلال أسطح CLI/البوابة.

| المجال | اكتب | المتطلبات |
|-------|------|--------------|
| `version` | `1` (u16) | يجب أن يساوي `AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | سلسلة (`AC-YYYY-###`) | معرف مستقر؛ يتم تنفيذها أثناء التحقق من الصحة. |
| `submitted_at_unix_ms` | u64 | ميلي ثانية منذ عصر يونكس. |
| `language` | سلسلة | علامة BCP‑47 (`"en"`، `"ja-JP"`، وما إلى ذلك). |
| `action` | التعداد (`add-to-denylist`، `remove-from-denylist`، `amend-policy`) | الإجراء المطلوب من الوزارة. |
| `summary.title` | سلسلة | ≥256 حرفًا مستحسن. |
| `summary.motivation` | سلسلة | لماذا الإجراء مطلوب. |
| `summary.expected_impact` | سلسلة | النتائج إذا تم قبول الإجراء. |
| `tags[]` | سلاسل صغيرة | تسميات الفرز الاختيارية. القيم المسموح بها: `csam`، `malware`، `fraud`، `harassment`، `impersonation`، `policy-escalation`، `terrorism`، `spam`. |
| `targets[]` | أشياء | واحد أو أكثر من إدخالات عائلة التجزئة (انظر أدناه). |
| `evidence[]` | أشياء | واحد أو أكثر من مرفقات الأدلة (انظر أدناه). |
| `submitter.name` | سلسلة | عرض الاسم أو المنظمة. |
| `submitter.contact` | سلسلة | البريد الإلكتروني، أو مقبض المصفوفة، أو الهاتف؛ تم تنقيحه من لوحات المعلومات العامة. |
| `submitter.organization` | سلسلة (اختياري) | مرئي في واجهة مستخدم المراجع. |
| `submitter.pgp_fingerprint` | سلسلة (اختياري) | بصمة 40 سداسية كبيرة. |
| `duplicates[]` | سلاسل | مراجع اختيارية لمعرفات الاقتراحات المقدمة مسبقًا. |

### الإدخالات المستهدفة (`targets[]`)

يمثل كل هدف ملخص عائلة التجزئة المشار إليه في الاقتراح.

| المجال | الوصف | التحقق من الصحة |
|-------|------------|------------|
| `label` | اسم مألوف لسياق المراجع. | غير فارغة. |
| `hash_family` | معرف التجزئة (`blake3-256`، `sha256`، وما إلى ذلك). | أحرف/أرقام ASCII/`-_.`، ≥48 حرفًا. |
| `hash_hex` | تم ترميز الملخص بأحرف صغيرة. | ≥16 بايت (32 حرفًا سداسيًا عشريًا) ويجب أن يكون سداسيًا عشريًا صالحًا. |
| `reason` | وصف موجز لسبب وجوب تنفيذ الملخص. | غير فارغة. |

يرفض المدقق أزواج `hash_family:hash_hex` المكررة داخل نفس الزوج
يتعارض الاقتراح والتقارير عندما تكون نفس البصمة موجودة بالفعل في
تسجيل مكرر (انظر أدناه).

### مرفقات الأدلة (`evidence[]`)

وثيقة إدخالات الأدلة حيث يمكن للمراجعين جلب السياق الداعم.| المجال | اكتب | ملاحظات |
|-------|------|-------|
| `kind` | التعداد (`url`، `torii-case`، `sorafs-cid`، `attachment`) | يحدد متطلبات الخلاصة. |
| `uri` | سلسلة | عنوان URL لـ HTTP(S)، أو معرف الحالة Torii، أو SoraFS URI. |
| `digest_blake3_hex` | سلسلة | مطلوب لأنواع `sorafs-cid` و`attachment`؛ اختياري للآخرين. |
| `description` | سلسلة | نص اختياري حر للمراجعين. |

### تسجيل مكرر

يمكن للمشغلين الاحتفاظ بسجل لبصمات الأصابع الموجودة لمنع التكرار
الحالات. يقبل المدقق ملف JSON على الشكل التالي:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

عندما يتطابق هدف الاقتراح مع إدخال ما، يتم إيقاف أداة التحقق ما لم
تم تحديد `--allow-registry-conflicts` (لا تزال التحذيرات مرسلة).
استخدم [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) لـ
قم بإنشاء ملخص جاهز للاستفتاء والذي يشير إلى النسخة المكررة
لقطات التسجيل والسياسة.

## استخدام سطر الأوامر

قم بفحص اقتراح واحد والتحقق منه مقابل سجل مكرر:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

قم بتمرير `--allow-registry-conflicts` لخفض مستوى الزيارات المكررة إلى التحذيرات عندما
إجراء عمليات التدقيق التاريخية.

تعتمد واجهة سطر الأوامر (CLI) على نفس مخطط Norito ومساعدي التحقق من الصحة الذين تم شحنهم
`iroha_data_model`، لذا يمكن لمجموعات SDK/البوابات الإلكترونية إعادة استخدام `AgendaProposalV1::validate`
طريقة السلوك المستمر.

## ترتيب سطر الأوامر (MINFO-2b)

مرجع خريطة الطريق: **MINFO-2b — سجل الفرز والتدقيق متعدد الفتحات.**

تتم الآن إدارة قائمة مجلس الأجندة من خلال الفرز الحتمي للمواطنين
يمكن تدقيق كل سحب بشكل مستقل. استخدم الأمر الجديد:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — ملف JSON يصف كل عضو مؤهل:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  يوجد ملف المثال في
  `docs/examples/ministry/agenda_council_roster.json`. الحقول الاختيارية (الدور،
  المنظمة، جهة الاتصال، البيانات الوصفية) في ورقة ميركل حتى يتمكن المدققون من تسجيلها
  يمكن أن تثبت القائمة التي غذت القرعة.

- `--slots` — عدد مقاعد المجلس المطلوب شغلها.
- `--seed` — بذرة BLAKE3 بحجم 32 بايت (64 حرفًا سداسيًا عشريًا صغيرًا) مسجلة في
  محضر الحكم لإجراء القرعة.
- `--out` — مسار الإخراج الاختياري. عند الحذف، تتم طباعة ملخص JSON إلى
  com.stdout.

### ملخص الإخراج

يصدر الأمر `SortitionSummary` JSON blob. يتم تخزين إخراج العينة في
`docs/examples/ministry/agenda_sortition_summary_example.json`. الحقول الرئيسية:

| المجال | الوصف |
|-------|------------|
| `algorithm` | تسمية الفرز (`agenda-sortition-blake3-v1`). |
| `roster_digest` | ملخص BLAKE3 + SHA-256 لملف القائمة (يُستخدم لتأكيد عمليات التدقيق التي تتم على نفس قائمة الأعضاء). |
| `seed_hex` / `slots` | قم بتكرار مدخلات واجهة سطر الأوامر (CLI) حتى يتمكن المدققون من إعادة إنتاج الرسم. |
| `merkle_root_hex` | جذر شجرة Merkle القائمة (`hash_node`/`hash_leaf` المساعدين في `xtask/src/ministry_agenda.rs`). |
| `selected[]` | إدخالات لكل فتحة، بما في ذلك البيانات التعريفية الأساسية للأعضاء، والفهرس المؤهل، وفهرس القائمة الأصلي، وإنتروبيا السحب الحتمية، وتجزئة الأوراق، وأشقاء إثبات Merkle. |

### التحقق من التعادل1. قم بإحضار القائمة المشار إليها بواسطة `roster_path` وتحقق من BLAKE3/SHA-256
   تطابق الملخصات الملخص.
2. أعد تشغيل واجهة سطر الأوامر (CLI) بنفس المصدر/الفتحات/القائمة؛ الناتج `selected[].member_id`
   يجب أن يتطابق الطلب مع الملخص المنشور.
3. بالنسبة لعضو معين، قم بحساب ورقة Merkle باستخدام العضو المتسلسل JSON
   (`norito::json::to_vec(&sortition_member)`) وأضعاف كل تجزئة إثبات. النهائي
   يجب أن يساوي الملخص `merkle_root_hex`. يظهر المساعد في ملخص المثال
   كيفية الجمع بين `eligible_index` و`leaf_hash_hex` و`merkle_proof[]`.

تلبي هذه المصنوعات اليدوية متطلبات MINFO-2b الخاصة بالعشوائية التي يمكن التحقق منها،
تحديد k-of-m وسجلات تدقيق الإلحاق فقط حتى يتم توصيل واجهة برمجة التطبيقات على السلسلة.

## مرجع خطأ التحقق من الصحة

`AgendaProposalV1::validate` يصدر متغيرات `AgendaProposalValidationError`
عندما تفشل الحمولة في عملية الفحص. ويلخص الجدول أدناه الأكثر شيوعا
الأخطاء حتى يتمكن مراجعو البوابة الإلكترونية من ترجمة مخرجات واجهة سطر الأوامر (CLI) إلى إرشادات قابلة للتنفيذ.| خطأ | معنى | علاج |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | تختلف الحمولة `version` عن المخطط المدعوم للمدقق. | قم بإعادة إنشاء JSON باستخدام أحدث حزمة مخطط بحيث يتطابق الإصدار مع `expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` فارغ أو ليس في النموذج `AC-YYYY-###`. | قم بملء معرف فريد باتباع التنسيق الموثق قبل إعادة الإرسال. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` صفر أو مفقود. | قم بتسجيل الطابع الزمني للإرسال بالميلي ثانية لنظام Unix. |
| `InvalidLanguageTag { value }` | `language` ليس علامة BCP-47 صالحة. | استخدم علامة قياسية مثل `en`، أو `ja-JP`، أو لغة أخرى يتعرف عليها BCP‑47. |
| `MissingSummaryField { field }` | أحد `summary.title`، أو `.motivation`، أو `.expected_impact` فارغ. | قم بتوفير نص غير فارغ لحقل الملخص المشار إليه. |
| `MissingSubmitterField { field }` | `submitter.name` أو `submitter.contact` مفقود. | قم بتوفير البيانات التعريفية المفقودة للمرسل حتى يتمكن المراجعون من الاتصال بمقدم المقترح. |
| `InvalidTag { value }` | الإدخال `tags[]` غير موجود في القائمة المسموح بها. | قم بإزالة العلامة أو إعادة تسميتها إلى إحدى القيم الموثقة (`csam`، `malware`، وما إلى ذلك). |
| `MissingTargets` | صفيف `targets[]` فارغ. | قم بتوفير إدخال واحد على الأقل لعائلة التجزئة المستهدفة. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | الإدخال الهدف يفتقد الحقول `label` أو `reason`. | قم بملء الحقل المطلوب للإدخال المفهرس قبل إعادة الإرسال. |
| `InvalidHashFamily { index, value }` | تسمية `hash_family` غير مدعومة. | تقييد أسماء عائلة التجزئة على الحروف الأبجدية الرقمية ASCII بالإضافة إلى `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | الملخص ليس سداسيًا عشريًا صالحًا أو أن حجمه أقل من 16 بايت. | قم بتوفير ملخص سداسي عشري صغير (≥32 حرفًا سداسيًا عشريًا) للهدف المفهرس. |
| `DuplicateTarget { index, fingerprint }` | يقوم ملخص الهدف بتكرار إدخال سابق أو بصمة التسجيل. | قم بإزالة التكرارات أو دمج الأدلة الداعمة في هدف واحد. |
| `MissingEvidence` | لم يتم توفير أي مرفقات الأدلة. | إرفاق سجل أدلة واحد على الأقل مرتبط بمواد النسخ. |
| `MissingEvidenceUri { index }` | إدخال الدليل يفتقد الحقل `uri`. | قم بتوفير معرف URI القابل للجلب أو معرف الحالة لإدخال الأدلة المفهرسة. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | إدخال الدليل الذي يتطلب ملخصًا (SoraFS CID أو المرفق) مفقود أو يحتوي على `digest_blake3_hex` غير صالح. | قم بتوفير ملخص BLAKE3 صغير مكون من 64 حرفًا للإدخال المفهرس. |

## أمثلة

- `docs/examples/ministry/agenda_proposal_example.json` - الكنسي،
  حمولة اقتراح تنظيف الوبر مع مرفقين للأدلة.
- `docs/examples/ministry/agenda_duplicate_registry.json` — سجل البداية
  تحتوي على بصمة BLAKE3 واحدة ومبررها.

أعد استخدام هذه الملفات كقوالب عند دمج أدوات البوابة الإلكترونية أو كتابة CI
التحقق من التقديمات الآلية.