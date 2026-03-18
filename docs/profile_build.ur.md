---
lang: ur
direction: rtl
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/profile_build.md (Profiling iroha_data_model Build) -->

# `iroha_data_model` بلڈ کا پروفائلنگ

`iroha_data_model` کی بلڈ میں سست مراحل تلاش کرنے کے لیے یہ helper اسکرپٹ چلائیں:

```sh
./scripts/profile_build.sh
```

یہ اسکرپٹ `cargo build -p iroha_data_model --timings` چلاتا ہے اور ٹائمنگ رپورٹس کو
`target/cargo-timings/` میں لکھتا ہے۔
براؤزر میں `cargo-timing.html` کھولیں اور ٹاسکس کو مدت کے لحاظ سے sort کریں تاکہ یہ
دیکھ سکیں کہ کن crates یا بلڈ کے مراحل پر سب سے زیادہ وقت صرف ہو رہا ہے۔

ان timings کی مدد سے آپ اپنی optimisation کی کوششیں سب سے سست ٹاسکس پر فوکس کر سکتے
ہیں۔

</div>

