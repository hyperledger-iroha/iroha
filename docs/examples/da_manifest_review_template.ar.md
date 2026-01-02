---
lang: ar
direction: rtl
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/da_manifest_review_template.md -->

# حزمة حوكمة بيان توفر البيانات (قالب)

استخدم هذا القالب عندما تراجع لجان البرلمان بيانات توفر البيانات (DA) للمنح، او
takedown، او تغييرات الاحتفاظ (خارطة الطريق DA-10). انسخ Markdown الى تذكرة الحوكمة،
املأ القوالب، وارفق الملف المكتمل مع حمولة Norito الموقعة وملفات CI المشار اليها ادناه.

```markdown
## بيانات البيان الوصفية
- اسم / اصدار البيان: <نص>
- فئة blob ووسم الحوكمة: <taikai_segment / da.taikai.live>
- ملخص BLAKE3 (hex): `<digest>`
- تجزئة حمولة Norito (اختياري): `<digest>`
- مغلف المصدر / URL: <https://.../manifest_signatures.json>
- معرف لقطة سياسة Torii: `<unix timestamp or git sha>`

## التحقق من التواقيع
- مصدر جلب البيان / تذكرة التخزين: `<hex>`
- امر/ناتج التحقق: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (هل تم ارفاق مقتطف سجل؟)
- `manifest_blake3` الذي ابلغت عنه الاداة: `<digest>`
- `chunk_digest_sha3_256` الذي ابلغت عنه الاداة: `<digest>`
- تواقيع مجلس الموقعين (multihash):
  - `<did:...>` / `<ed25519 multihash>`
- طابع وقت التحقق (UTC): `<2026-02-20T11:04:33Z>`

## التحقق من الاحتفاظ
| الحقل | المتوقع (السياسة) | المشاهد (البيان) | الدليل |
|-------|-------------------|------------------|--------|
| الاحتفاظ الحار (ثوان) | <مثلا، 86400> | <قيمة> | `<torii.da_ingest.replication_policy dump | CI link>` |
| الاحتفاظ البارد (ثوان) | <مثلا، 1209600> | <قيمة> |  |
| النسخ المطلوبة | <قيمة> | <قيمة> |  |
| فئة التخزين | <hot / warm / cold> | <قيمة> |  |
| وسم الحوكمة | <da.taikai.live> | <قيمة> |  |

## السياق
- نوع الطلب: <منحة | Takedown | تدوير البيان | تجميد طارئ>
- تذكرة الاصل / مرجع الامتثال: <رابط او معرف>
- اثر المنحة / الايجار: <تغير XOR المتوقع او "n/a">
- رابط استئناف الاشراف (ان وجد): <case_id او رابط>

## ملخص القرار
- اللجنة: <البنية التحتية | الاشراف | الخزانة>
- نتيجة التصويت: `<for>/<against>/<abstain>` (هل تحقق النصاب `<threshold>`؟)
- ارتفاع/نافذة التفعيل او التراجع: `<block/slot range>`
- اجراءات المتابعة:
  - [ ] ابلاغ الخزانة / عمليات الايجار
  - [ ] تحديث تقرير الشفافية (`TransparencyReportV1`)
  - [ ] جدولة تدقيق المخزن المؤقت

## التصعيد والتقارير
- مسار التصعيد: <منحة | امتثال | تجميد طارئ>
- رابط/معرف تقرير الشفافية (اذا تم تحديثه): <`TransparencyReportV1` CID>
- حزمة proof-token او مرجع ComplianceUpdate: <مسار او معرف تذكرة>
- دلتا دفتر الايجار / الاحتياطي (ان وجد): <`ReserveSummaryV1` snapshot link>
- روابط لقطات التليمترية: <Grafana permalink او معرف artefact>
- ملاحظات لمحضر البرلمان: <ملخص المواعيد النهائية / الالتزامات>

## المرفقات
- [ ] بيان Norito موقع (`.to`)
- [ ] ملخص JSON / artefact من CI يثبت قيم الاحتفاظ
- [ ] proof token او حزمة امتثال (لعمليات الحجب)
- [ ] لقطة تليمترية المخزن المؤقت (`iroha_settlement_buffer_xor`)
```

ارشِف كل حزمة مكتملة تحت مدخل Governance DAG الخاص بالتصويت حتى تتمكن المراجعات اللاحقة
من الرجوع الى ملخص البيان دون تكرار المراسم بالكامل.

</div>
