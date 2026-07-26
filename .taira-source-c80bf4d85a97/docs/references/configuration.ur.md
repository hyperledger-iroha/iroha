---
lang: ur
direction: rtl
source: docs/references/configuration.md
status: complete
translator: manual
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-11-02T04:40:39.795595+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/references/configuration.md (Acceleration) کا اردو ترجمہ -->

# Acceleration (ہاردویئر ایکسلریشن)

سیٹنگ سیکشن `[accel]`، IVM اور اس کے helpers کے لیے اختیاری ہارڈویئر
acceleration کو کنٹرول کرتا ہے۔ ہر accelerated path کے لیے deterministic
CPU fallbacks موجود ہیں؛ اگر کوئی backend runtime پر golden self‑test میں
ناکام ہو جائے تو اسے خودکار طور پر disable کر دیا جاتا ہے اور execution
CPU پر جاری رہتا ہے۔

- `enable_cuda` (ڈیفالٹ: `true`) – جب CUDA compile اور available ہو تو اسے
  استعمال کریں۔
- `enable_metal` (ڈیفالٹ: `true`) – macOS پر Metal available ہونے کی صورت
  میں اسے استعمال کریں۔
- `max_gpus` (ڈیفالٹ: `0`) – initialize ہونے والی GPUs کی زیادہ سے زیادہ
  تعداد؛ `0` کا مطلب auto/کوئی واضح حد نہیں۔
- `merkle_min_leaves_gpu` (ڈیفالٹ: `8192`) – Merkle leaves کے hashing کو GPU
  پر offload کرنے کے لیے کم از کم leaves کی تعداد۔ اسے صرف غیر معمولی
  تیز GPUs کے لیے کم کریں۔
- Advanced (اختیاری؛ عموماً مناسب defaults inherit کرتے ہیں):
  - `merkle_min_leaves_metal` (ڈیفالٹ: `merkle_min_leaves_gpu` کو inherit
    کرتا ہے)۔
  - `merkle_min_leaves_cuda` (ڈیفالٹ: `merkle_min_leaves_gpu` کو inherit
    کرتا ہے)۔
  - `prefer_cpu_sha2_max_leaves_aarch64` (ڈیفالٹ: `32768`) – ARMv8 + SHA2
    پر اس حد تک leaves کے لیے SHA‑2 کو CPU پر ترجیح دیں۔
  - `prefer_cpu_sha2_max_leaves_x86` (ڈیفالٹ: `32768`) – x86/x86_64 پر اس
    حد تک leaves کے لیے SHA‑NI کو CPU پر ترجیح دیں۔

نوٹس
- determinism سب سے اہم ہے: acceleration، observable outputs کو کبھی تبدیل
  نہیں کرتا؛ backends init کے وقت golden tests چلاتے ہیں، اور mismatch
  ہونے پر scalar/SIMD paths پر fallback کر جاتے ہیں۔
- کنفیگریشن ہمیشہ `iroha_config` کے ذریعے کریں؛ production میں environment
  variables پر انحصار کرنے سے گریز کریں۔

</div>

