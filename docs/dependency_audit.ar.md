---
lang: ar
direction: rtl
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/dependency_audit.md (Dependency Audit Summary) -->

//! ملخص تدقيق الاعتمادات (Dependencies)

التاريخ: 2025‑09‑01

النطاق: مراجعة على مستوى الـ workspace لكل الـ crates المعرَّفة في ملفات
Cargo.toml والمحلولة في Cargo.lock. أُجري التدقيق باستخدام `cargo-audit`
ضد قاعدة بيانات RustSec، إضافةً إلى مراجعة يدوية لشرعية crates واختيار
“الـ crates الرئيسية” للخوارزميات.

الأدوات/الأوامر المستخدمة:
- `cargo tree -d --workspace --locked --offline` – فحص الإصدارات المكررة.
- `cargo audit` – فحص Cargo.lock بحثًا عن الثغرات المعروفة والـ crates
  المسحوبة (yanked).

التنبيهات الأمنية التي تم العثور عليها (حاليًا 0 ثغرات؛ تحذيران):
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - تم الإصلاح: التحديث إلى `0.5.15` في `crates/ivm/Cargo.toml`.

- كودك pprof مهجور — RUSTSEC‑2024‑0437  
  - تم الإصلاح: تحويل `pprof` إلى `prost-codec` في
    `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC‑2025‑0009  
  - تم الإصلاح: ترقية مكدّس QUIC/TLS (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) وتحديث مكدّس WebSocket إلى
    `tungstenite/tokio-tungstenite 0.24`. تم تثبيت `ring` على
    `0.17.12` باستخدام:
    `cargo update -p ring --precise 0.17.12`.

التنبيهات المتبقية: لا يوجد. التحذيرات المتبقية: `backoff` (غير مدعوم)،
`derivative` (غير مدعوم).

تقييم الشرعية و“crates الرئيسية” (ملخّص):
- الـ hashing: الحزم `sha2` (RustCrypto)، و`blake2` (RustCrypto)،
  و`tiny-keccak` (مستخدمة على نطاق واسع) — خيارات معيارية.
- التماثل/AEAD: `aes-gcm`، `chacha20poly1305`، وـ traits `aead`
  (RustCrypto) — معيارية.
- التواقيع/ECC: `ed25519-dalek`، `x25519-dalek` (مشروع dalek)،
  `k256` (RustCrypto)، `secp256k1` (روابط libsecp) — جميعها شرعية؛ من
  المفضّل توحيد الاعتماد على مكدّس واحد لـ secp256k1 (`k256` بـ Rust
  خالص أو `secp256k1` مع libsecp) لتقليل سطح الهجوم.
- BLS12‑381/ZK: الحزم `blstrs` وعائلة `halo2_*` — مستخدمة على نطاق واسع في
  بيئات ZK الإنتاجية؛ شرعية.
- PQ: الحزم `pqcrypto-mldsa` و`pqcrypto-traits` — حزم مرجعية شرعية.
- TLS: `rustls`، `tokio-rustls`، `hyper-rustls` — مكدّس TLS حديث ومعياري
  في Rust.
- Noise: `snow` — تطبيق معياري لبروتوكول Noise.
- التسييل (Serialization): الحزمة `parity-scale-codec` هي الكودك المعياري
  لـ SCALE. تم إزالة Serde من اعتمادات الإنتاج في كامل الـ workspace؛
  يغطّي Norito (مشتقات/كاتبو التسييل) كل المسارات التنفيذية. أي إشارات
  متبقية لـ Serde موجودة في وثائق تاريخية، أو سكربتات حراسة، أو
  allowlists خاصة بالاختبارات.
- FFI/libs: `libsodium-sys-stable`، `openssl` — حزم شرعية؛ يُفضّل Rustls
  بدل OpenSSL في مسارات الإنتاج (الكود الحالي يلتزم بذلك).
- pprof 0.13.0 (من crates.io) — تم دمج الإصلاح upstream؛ يجب استخدام
  الإصدار الرسمي مع `prost-codec` + frame‑pointer لتجنّب الكودك المهجور.

التوصيات:
- معالجة التحذيرات:
  - النظر في استبدال `backoff` بـ `retry` أو `futures-retry`، أو كتابة
    helper محلي لآلية backoff أسّي.
  - استبدال الـ derives الخاصة بـ `derivative` بتنفيذات (impls) يدوية أو
    باستخدام `derive_more` حيث يكون ذلك مناسبًا.
- على المدى المتوسط: توحيد الاعتماد على `k256` أو `secp256k1` حيثما
  أمكن لتقليل تعدد التطبّقات (والإبقاء على كليهما فقط عند الضرورة).
- على المدى المتوسط: مراجعة مصدر `poseidon-primitives 0.2.0` لاستخدامه في
  سياقات ZK؛ وإذا أمكن، التوجّه نحو تطبيق Poseidon أصيل من Arkworks/Halo2
  لتقليل تعدد الأنظمة المتوازية.

ملاحظات:
- يُظهِر `cargo tree -d` تكرارًا متوقَّعًا في الإصدارات الكبرى
  (`bitflags` 1/2 ونسخًا متعددة من `ring`)؛ هذا ليس خطرًا أمنيًا بحد
  ذاته، لكنه يزيد من سطح الـ build.
- لم تُرصد crates مشتبه بها من نوع typosquat؛ جميع الأسماء والمصادر تعود
  إلى حزم معروفة في منظومة Rust أو إلى أعضاء داخليين في الـ workspace.
- تجريبيًا: تمت إضافة الـ feature `bls-backend-blstrs` إلى
  `iroha_crypto` للبدء في نقل BLS إلى backend يعتمد فقط على `blstrs`
  (يزيل الاعتماد على arkworks عند تفعيله). تظل القيمة الافتراضية `w3f-bls`
  لتجنّب تغييرات في السلوك/التشفير. خطة التوافق:
  - توحيد تسييل المفتاح السري إلى مخرجات معيارية بطول 32 بايت بالـ
    little‑endian، بحيث يفهمها كل من `w3f-bls` و`blstrs`
    (`Scalar::to_bytes_le`)، مع التخلّي عن الـ helper القديم ذي الـ endianness
    المختلط.
  - تقديم wrapper صريح لضغط المفتاح العام باستخدام
    `blstrs::G1Affine::to_compressed`، مع إضافة فحص توافق مع ترميز w3f
    لضمان تطابق البايتات على السلك (on‑wire).
  - إضافة fixtures لاختبارات round‑trip في
    `crates/iroha_crypto/tests/bls_backend_compat.rs`، بحيث تُشتق المفاتيح
    مرة واحدة، وتُقارن نواتج كلا الـ backends لـ `SecretKey`, `PublicKey`
    وتجميع التواقيع.
  - حماية الـ backend الجديد عبر feature `bls-backend-blstrs` في CI، مع
    الاستمرار في تشغيل اختبارات التوافق للbackend الافتراضي لرصد
    الارتدادات قبل أي انتقال.

خطوات لاحقة (بنود عمل مقترحة):
- الإبقاء على سكربتات الحماية من Serde في CI
  (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) حتى لا
  يُسمح بإضافة استخدامات جديدة لـ Serde في مسارات الإنتاج.

الاختبارات التي أُجريت لهذا التدقيق:
- 

</div>

