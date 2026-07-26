---
lang: ar
direction: rtl
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# عمليات تدقيق تبعية التشفير

## ستريبوج (صندوق `streebog`)

- **الإصدار الموجود في الشجرة:** `0.11.0-rc.2` يتم بيعه ضمن `vendor/streebog` (يُستخدم عند تمكين ميزة `gost`).
- **المستهلك:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + تجزئة الرسالة).
- **الحالة:** مرشح الإصدار فقط. لا يوجد صندوق غير RC يوفر حاليًا سطح واجهة برمجة التطبيقات (API) المطلوب،
  لذلك نقوم بعكس الصندوق داخل الشجرة من أجل إمكانية التدقيق بينما نتتبع المنبع للحصول على الإصدار النهائي.
- **مراجعة نقاط التفتيش:**
  - تم التحقق من إخراج التجزئة مقابل مجموعة Wycheproof وتركيبات TC26 عبر
    `cargo test -p iroha_crypto --features gost` (راجع `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  -`cargo bench -p iroha_crypto --bench gost_sign --features gost`
    يمارس Ed25519/Secp256k1 جنبًا إلى جنب مع كل منحنى TC26 مع التبعية الحالية.
  -`cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    يقارن القياسات الأحدث مع المتوسطات المسجلة (استخدم `--summary-only` في CI، وأضف
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` عند إعادة تحديد الأساس).
  - `scripts/gost_bench.sh` يغلف المقعد + فحص التدفق؛ قم بتمرير `--write-baseline` لتحديث JSON.
    راجع `docs/source/crypto/gost_performance.md` للتعرف على سير العمل الشامل.
- **عمليات التخفيف:** لا يتم استدعاء `streebog` إلا ​​من خلال الأغلفة الحتمية التي تعمل على صفر المفاتيح؛
  يقوم الموقع بالتحوط من عدم وجود إنتروبيا نظام التشغيل لتجنب فشل RNG الكارثي.
- **الإجراءات التالية:** اتبع إصدار RustCrypto's streebog `0.11.x`؛ بمجرد هبوط العلامة، تعامل مع
  الترقية باعتبارها نتوء تبعية قياسي (التحقق من المجموع الاختباري، ومراجعة الفرق، وتسجيل المصدر، و
  أسقط المرآة المباعة).