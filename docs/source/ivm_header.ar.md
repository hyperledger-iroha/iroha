---
lang: ar
direction: rtl
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T10:30:30.084677+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM رأس الرمز الثانوي


سحر
- 4 بايت: ASCII `IVM\0` عند الإزاحة 0.

التخطيط (الحالي)
- الإزاحات والأحجام (إجمالي 17 بايت):
  - 0..4: سحري `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (بتات الميزات، انظر أدناه)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (النهاية الصغيرة)
  - 16: `abi_version: u8`

بتات الوضع
- `ZK = 0x01`، `VECTOR = 0x02`، `HTM = 0x04` (محجوز/مزود ببوابات مميزة).

الحقول (المعنى)
- `abi_version`: إصدار مخطط syscall ومؤشر ABI.
- `mode`: البتات المميزة لتتبع ZK/VECTOR/HTM.
- `vector_length`: طول المتجه المنطقي لعمليات المتجه (0 → غير محدد).
- `max_cycles`: ربط التنفيذ المستخدم في وضع ZK والقبول.

ملاحظات
- يتم تحديد Endianness والتخطيط من خلال التنفيذ وربطهما بـ `version`. يعكس التخطيط المتصل بالسلك أعلاه التنفيذ الحالي في `crates/ivm_abi/src/metadata.rs`.
- يمكن للقارئ البسيط الاعتماد على هذا التخطيط للعناصر الحالية ويجب عليه التعامل مع التغييرات المستقبلية عبر بوابة `version`.
- يتم تفعيل تسريع الأجهزة (SIMD/Metal/CUDA) لكل مضيف. يقرأ وقت التشغيل قيم `AccelerationConfig` من `iroha_config`: يفرض `enable_simd` عمليات احتياطية عددية عندما تكون خاطئة، بينما يقوم `enable_metal` و`enable_cuda` ببوابة واجهاتهم الخلفية حتى عند تجميعها. يتم تطبيق عمليات التبديل هذه من خلال `ivm::set_acceleration_config` قبل إنشاء VM.
- تظهر حزم SDK للأجهزة المحمولة (Android/Swift) على نفس المقابض؛ `IrohaSwift.AccelerationSettings`
  يستدعي `connect_norito_set_acceleration_config` حتى تتمكن إصدارات macOS/iOS من الاشتراك في Metal /
  النيون مع الحفاظ على الاحتياطيات الحتمية.
- يمكن للمشغلين أيضًا فرض تعطيل الواجهات الخلفية المحددة للتشخيصات عن طريق تصدير `IVM_DISABLE_METAL=1` أو `IVM_DISABLE_CUDA=1`. تتمتع تجاوزات البيئة هذه بالأولوية على التكوين وتحافظ على الجهاز الافتراضي على المسار المحدد لوحدة المعالجة المركزية.

مساعدات الحالة المتينة وسطح ABI
- تعد استدعاءات النظام المساعدة للحالة الدائمة (0x50–0x5A: STATE_{GET,SET,DEL} وENCODE/DECODE_INT وBUILD_PATH_* وتشفير/فك تشفير JSON/SCHEMA) جزءًا من V1 ABI ويتم تضمينها في حساب `abi_hash`.
- يقوم CoreHost بتوصيل STATE_{GET,SET,DEL} إلى حالة العقد الذكي المتين المدعومة من WSV؛ قد يستخدم مضيفو dev/test التراكبات أو الثبات المحلي ولكن يجب عليهم الحفاظ على نفس السلوك الملحوظ.

التحقق من الصحة
- يقبل قبول العقدة رؤوس `version_major = 1` و`version_minor = 0` فقط.
- يجب أن يحتوي `mode` على البتات المعروفة فقط: `ZK`، `VECTOR`، `HTM` (يتم رفض البتات غير المعروفة).
- `vector_length` هو استشاري وقد يكون غير صفري حتى إذا لم يتم تعيين البت `VECTOR`؛ القبول يفرض الحد الأعلى فقط.
- قيم `abi_version` المدعومة: الإصدار الأول يقبل فقط `1` (V1)؛ يتم رفض القيم الأخرى عند القبول.

### السياسة (التي تم إنشاؤها)
يتم إنشاء ملخص السياسة التالي من التنفيذ ولا يجب تحريره يدويًا.<!-- BEGIN GENERATED HEADER POLICY -->
| المجال | سياسة |
|---|---|
| version_major | 1 |
| version_minor | 0 |
| الوضع (البتات المعروفة) | 0x07 (ZK=0x01، VECTOR=0x02، HTM=0x04) |
| أبي_النسخة | 1 |
| Vector_length | 0 أو 1..=64 (استشاري؛ مستقل عن بت VECTOR) |
<!-- END GENERATED HEADER POLICY -->

### تجزئة ABI (تم إنشاؤها)
يتم إنشاء الجدول التالي من التنفيذ ويسرد قيم `abi_hash` الأساسية للسياسات المدعومة.

<!-- BEGIN GENERATED ABI HASHES -->
| سياسة | أبي_هاش (ست عشري) |
|---|---|
| أبي v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- قد تضيف التحديثات البسيطة تعليمات خلف `feature_bits` ومساحة كود التشغيل المحجوزة؛ قد تؤدي التحديثات الرئيسية إلى تغيير الترميزات أو إزالتها/إعادة توظيفها فقط مع ترقية البروتوكول.
- نطاقات Syscall مستقرة؛ غير معروف بالنسبة لـ `abi_version` النشط ينتج `E_SCALL_UNKNOWN`.
- ترتبط جداول الغاز بـ `version` وتتطلب ناقلات ذهبية عند التغيير.

فحص القطع الأثرية
- استخدم `ivm_tool inspect <file.to>` للحصول على عرض ثابت لحقول الرأس.
- للتطوير، تتضمن الأمثلة/ هدف Makefile صغير `examples-inspect` الذي يقوم بفحص القطع الأثرية المبنية.

مثال (الصدأ): الحد الأدنى من السحر + التحقق من الحجم

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

ملاحظة: تم إصدار تخطيط الرأس الدقيق الذي يتجاوز السحر وتحديد التنفيذ؛ تفضل `ivm_tool inspect` لأسماء وقيم الحقول الثابتة.