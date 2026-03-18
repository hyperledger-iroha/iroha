---
lang: ar
direction: rtl
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#تصديقات المجال

تسمح موافقات النطاق للمشغلين بإنشاء النطاق وإعادة استخدامه بموجب بيان موقع من اللجنة. حمولة التصديق هي كائن Norito مسجل على السلسلة حتى يتمكن العملاء من تدقيق من قام بالتصديق على أي مجال ومتى.

## شكل الحمولة

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: معرف المجال الأساسي
- `committee_id`: تسمية اللجنة التي يمكن قراءتها بواسطة الإنسان
-`statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: ارتفاعات الكتلة المحيطة بالصلاحية
- `scope`: مساحة بيانات اختيارية بالإضافة إلى نافذة `[block_start, block_end]` اختيارية (شاملة) والتي **يجب** أن تغطي ارتفاع الكتلة المقبولة
- `signatures`: التوقيعات عبر `body_hash()` (المصادقة بـ `signatures = []`)
- `metadata`: بيانات تعريف Norito الاختيارية (معرفات الاقتراح، وارتباطات التدقيق، وما إلى ذلك)

## التنفيذ

- التصديقات مطلوبة عند تمكين Nexus و`nexus.endorsement.quorum > 0`، أو عندما تحدد سياسة لكل مجال المجال على أنه مطلوب.
- يفرض التحقق من الصحة ربط تجزئة المجال/البيان، والإصدار، ونافذة الحظر، وعضوية مساحة البيانات، وانتهاء الصلاحية/العمر، والنصاب القانوني للجنة. يجب أن يكون لدى الموقعين مفاتيح إجماع مباشرة مع الدور `Endorsement`. تم رفض عمليات الإعادة بواسطة `body_hash`.
- تستخدم التصديقات المرتبطة بتسجيل النطاق مفتاح البيانات التعريفية `endorsement`. يتم استخدام نفس مسار التحقق من الصحة بواسطة تعليمات `SubmitDomainEndorsement`، التي تسجل الموافقات للتدقيق دون تسجيل مجال جديد.

## اللجان والسياسات

- يمكن تسجيل اللجان على السلسلة (`RegisterDomainCommittee`) أو مشتقة من إعدادات التكوين الافتراضية (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`، المعرف = `default`).
- يتم تكوين السياسات لكل مجال عبر `SetDomainEndorsementPolicy` (معرف اللجنة، علامة `max_endorsement_age`، `required`). في حالة الغياب، يتم استخدام الإعدادات الافتراضية Nexus.

## مساعدي CLI

- إنشاء/توقيع المصادقة (إخراج Norito JSON إلى stdout):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- تقديم تأييد:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- إدارة الحوكمة:
  -`iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  -`iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  -`iroha endorsement policy --domain wonderland`
  -`iroha endorsement committee --committee-id jdga`
  -`iroha endorsement list --domain wonderland`

تؤدي حالات فشل التحقق من الصحة إلى إرجاع سلاسل أخطاء مستقرة (عدم تطابق النصاب القانوني، والمصادقة التي لا معنى لها/منتهي الصلاحية، وعدم تطابق النطاق، ومساحة البيانات غير المعروفة، واللجنة المفقودة).