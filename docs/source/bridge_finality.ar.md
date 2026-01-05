---
lang: ar
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7236dfe86175ff89f660be4cb4dd2c90df20a05f9606d2707407465f639b1a1c
source_last_modified: "2025-12-05T06:21:36.529838+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/bridge_finality.md -->

<!--
SPDX-License-Identifier: Apache-2.0
-->

# اثباتات نهائية Bridge

يصف هذا المستند السطح الاولي لاثباتات نهائية Bridge في Iroha.
الهدف هو تمكين السلاسل الخارجية او light clients من التحقق من ان كتلة Iroha
نهائية دون حسابات off-chain او مرحلات موثوقة.

## تنسيق الاثبات

`BridgeFinalityProof` (Norito/JSON) يحتوي على:

- `height`: ارتفاع الكتلة.
- `chain_id`: معرف سلسلة Iroha لمنع اعادة التشغيل عبر السلاسل.
- `block_header`: `BlockHeader` كانوني.
- `block_hash`: hash للـ header (يعيد العملاء حسابه للتحقق).
- `commit_certificate`: مجموعة المدققين + التواقيع التي انهت الكتلة.

الاثبات مكتف ذاتيا؛ لا حاجة الى manifests خارجية او blobs مبهمة.
الاحتفاظ: يقدم Torii اثباتات نهائية ضمن نافذة commit-certificate الاخيرة
(محدودة بواسطة cap التاريخ المضبوط؛ الافتراضي 512 ادخالا عبر
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). ينبغي على العملاء
تخزين الاثباتات او تثبيتها اذا احتاجوا افقا اطول.
الثلاثي الكانوني هو `(block_header, block_hash, commit_certificate)`: يجب ان يطابق hash
الـ header الـ hash داخل commit certificate، ويربط chain id الاثبات بدفتر واحد. ترفض
الخوادم وتسجل `CommitCertificateHashMismatch` عندما يشير certificate الى hash كتلة مختلف.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) يوسع الاثبات الاساسي مع commitment وتبرير صريحين:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: تواقيع authority set على payload الـ commitment
  (يعيد استخدام تواقيع commit certificate).
- `block_header`, `commit_certificate`: نفس الاثبات الاساسي.

Placeholder الحالي: يتم اشتقاق `mmr_root`/`mmr_peaks` عبر اعادة حساب MMR لِـ block-hash في الذاكرة؛
اثباتات الادراج لم تعد بعد. ما زال بإمكان العملاء التحقق من نفس الـ hash عبر payload الـ commitment حاليا.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

التحقق مماثل للاثبات الاساسي: اعادة حساب `block_hash` من الـ header، التحقق من تواقيع commit
certificate، والتحقق من ان حقول commitment تطابق الشهادة وhash الكتلة. يضيف الـ bundle غلاف
commitment/justification لبروتوكولات bridge التي تفضل الفصل.

## خطوات التحقق

1. اعادة حساب `block_hash` من `block_header`؛ ارفض عند عدم التطابق.
2. تحقق من ان `commit_certificate.block_hash` يطابق `block_hash` المعاد حسابه؛
   ارفض ازواج header/commit certificate غير المتطابقة.
3. تحقق من ان `chain_id` يطابق سلسلة Iroha المتوقعة.
4. اعادة حساب `validator_set_hash` من `commit_certificate.validator_set` والتحقق من
   مطابقته للـ hash/النسخة المسجلة.
5. تحقق من التواقيع في commit certificate مقابل hash الـ header باستخدام المفاتيح
   العامة والمؤشرات المعلنة؛ طبق quorum (`2f+1` عندما `n>3`، والا `n`) وارفض
   المؤشرات المكررة/خارج النطاق.
6. اختياريا اربط بـ checkpoint موثوق بمقارنة hash مجموعة المدققين بقيمة مثبتة
   (weak-subjectivity anchor).
7. اختياريا اربط بـ anchor متوقع للـ epoch بحيث ترفض اثباتات epochs الاقدم/الاحدث
   حتى يتم تدوير الـ anchor عمدا.

`BridgeFinalityVerifier` (ضمن `iroha_data_model::bridge`) يطبق هذه الفحوصات، ويرفض
انحراف chain-id/height، عدم تطابق hash/نسخة validator set، الموقّعين المكررين/خارج
النطاق، التواقيع غير الصالحة، وepochs غير المتوقعة قبل احتساب quorum كي يتمكن
light clients من اعادة استخدام verifier واحد.

## مرجع التحقق

`BridgeFinalityVerifier` يقبل `chain_id` متوقعا مع anchors اختيارية لمجموعة المدققين والـ epoch.
يفرض ثلاثي header/block-hash/commit-certificate، يتحقق من hash/نسخة validator set، ويتحقق من
التواقيع/quorum مقابل roster المدققين المعلن، ويتتبع احدث ارتفاع لرفض الاثباتات
القديمة/المتخطاة. عند توفير anchors يرفض replays بين epochs/rosters مع اخطاء
`UnexpectedEpoch`/`UnexpectedValidatorSet`; بدون anchors يعتمد hash مجموعة المدققين والـ epoch
من اول اثبات قبل الاستمرار في فرض اخطاء حتمية للتواقيع المكررة/خارج النطاق/غير الكافية.

## سطح API

- `GET /v1/bridge/finality/{height}` - يعيد `BridgeFinalityProof` لارتفاع الكتلة المطلوب.
  تفاوض المحتوى عبر `Accept` يدعم Norito او JSON.
- `GET /v1/bridge/finality/bundle/{height}` - يعيد `BridgeFinalityBundle`
  (commitment + justification + header/certificate) للارتفاع المطلوب.

## ملاحظات ومتابعات

- الاثباتات مشتقة حاليا من commit certificates المخزنة. التاريخ المحدود يتبع نافذة
  الاحتفاظ لـ commit certificate؛ يجب على العملاء تخزين اثباتات الارتكاز اذا احتاجوا
  افقا اطول. الطلبات خارج النافذة تعيد `CommitCertificateNotFound(height)`؛ اظهر الخطا
  وارجع الى checkpoint مثبت.
- اثبات معاد تشغيله او مزور مع `block_hash` غير متطابق (header مقابل certificate) يتم رفضه
  بـ `CommitCertificateHashMismatch`؛ يجب على العملاء اجراء نفس فحص الثلاثي قبل التحقق
  من التواقيع ورفض payloads غير المتطابقة.
- العمل المستقبلي يمكن ان يضيف سلاسل commitment من MMR/authority-set لتقليل حجم الاثباتات
  للتواريخ الطويلة جدا. يبقى التنسيق متوافقا للخلف عبر تغليف commit certificate داخل
  envelopes اغنى للـ commitment.

</div>
