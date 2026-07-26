---
lang: ar
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
---

<div dir="rtl">

<!--
SPDX-License-Identifier: Apache-2.0
-->

# براهين نهائية الجسر

تحدد هذه الوثيقة صيغة الإصدار الأول لنهائية الجسر. يحمل البرهان دليل النهائية الدائم والدقيق
الذي ينتجه Sumeragi v2. إصدار مخطط غلاف البرهان هو `1`، بينما إصدار بروتوكول الإجماع
داخله هو `2`. لا يوجد إسقاط لشهادة Sumeragi v1 ولا decoder ولا مسار fallback.

## صيغة البرهان الدقيقة

يحتوي `BridgeFinalityProof` المشفر بـ Norito أو Norito JSON على ثلاثة حقول فقط:

```text
{ version, block_header, finality_artifact }
```

- يجب أن تكون `version` مساوية لـ `1`؛
- `block_header` هو `BlockHeader` القانوني للارتفاع المطلوب؛
- `finality_artifact` هو `V2FinalityArtifact` الدقيق المحفوظ للكتلة. وهو يضم بصورة
  دائمة PoP من نوع BLS-normal لكل مدقق وبالترتيب نفسه في roster الخاص بسياق الارتفاع
  (`validator_set_pops`).

يحتوي الـ artifact على `HeightContext` الكامل وغير القابل للتغيير، و`BlockSubject`
الدقيق، وhash الكتلة، وCommitQC، وPoP المرتبة مع roster. يجمّد سياق الارتفاع السلسلة
والحقبة وroster و`DualQuorum` وتخطيط DA وleader seed وغيرها من بيانات الإجماع.
ويتضمن سياق الكتلة الأب التي تنهي الحقبة أيضا `next_epoch_snapshot` اختياريا؛ ولأن هذا
الحقل يدخل في context id فإن CommitQC للأب يوثقه قبل أن يستطيع تخويل roster الابن.
كما يوثق snapshot النهائي `epoch_end_height` و`validator_set_pops` المرتبة مع roster التالي
إضافة إلى معاملات الحقبة التالية.

## الحفظ والتحقق

قبل نشر finality أو إخلاء body الكتلة، يحفظ Kura سجلا ثابتا يحتوي الـheader القانوني الدقيق
وأرشيف SCCP بترتيب `commitment_index`. ثم يحفظ artifact النهائي في سجل ثابت منفصل مرتبط
بالـheader نفسه. يقرأ منشئ البرهان الـheader المحتفظ به وسجل finality فقط؛ ولا يحتاج إلى
body كتلة تاريخية أو payload قابل للتغيير في WSV. يؤدي فقد أي سجل أو تلفه أو تعارضه أو
فشل التحقق منه إلى الرفض المغلق.

يطابق المدقق عديم الحالة الإصدار والسلسلة والارتفاع وhash الـ header والـ predecessor
القانوني وview والسياق والـ subject
وCommitQC بدقة، ويتحقق من جميع PoP المضمنة في الـ artifact. يجب أن تكون فهارس الموقعين
متزايدة تماما وضمن المجال، وأن يحقق CommitQC حدّي quorum: عدد المدققين وقوة التصويت،
وأن تكون BLS aggregate signature على Sumeragi v2 vote preimage الدقيق صحيحة.

## مرساة الثقة والتحقق من الخلف

يثبت البرهان المنفرد اتساقه الداخلي تحت roster الذي يحمله فقط. لذلك يتطلب
`BridgeFinalityVerifier` قيمة `HeightContextId` موثوقة صراحة قبل قبول أول برهان. بعد ذلك
لا يقبل إلا الارتفاع التالي مباشرة، ويتحقق من parent CommitQC في سياق الابن باستخدام
roster وPoP المجمدين السابقين. داخل الحقبة ينسخ artifact الابن PoP الخاصة بالـ artifact
السابق؛ وعند الحد يجب أن تطابق الحقبة وroster وquorum وseed وPoP قيمة
`next_epoch_snapshot` في سياق الأب، بما فيها `epoch_end_height` الموثقة، وكلها موثقة
بـ CommitQC الخاص بالأب. ترفض
الارتفاعات القديمة أو المتخطاة والخلف غير المرتبط.

يستخدم SCCP النوع نفسه `BridgeFinalityProof`. لا تكفي الثقة بتوقيع تحت roster يقدمه
الرسالة؛ يجب التحقق من كل خلف مباشر بدءا من context/artifact لنقطة تحقق مثبتة بالحوكمة
وصولا إلى artifact الرسالة.

## Bundle وواجهة API

يحتوي `BridgeFinalityBundle` بالضبط على `{ commitment, finality_proof }`. ويكون
commitment هو بالضبط
`{ chain_id, height_context_id, block_height, block_hash }`.

- يعيد `GET /v1/bridge/finality/{height}` قيمة `BridgeFinalityProof`؛
- يعيد `GET /v1/bridge/finality/bundle/{height}` قيمة `BridgeFinalityBundle`.

يفشل المساران بصورة مغلقة إذا غاب الـheader القانوني المحتفظ به أو الـartifact الدائم الدقيق
لإصدار v2 أو كان غير صالح. لا يؤدي إخلاء body الكتلة إلى فقد برهان صحيح. يجب رفض الحقول
والإصدارات وأشكال البرهان المتقاعدة غير المعروفة.

</div>
