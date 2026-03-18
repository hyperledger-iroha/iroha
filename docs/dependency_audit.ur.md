---
lang: ur
direction: rtl
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/dependency_audit.md (Dependency Audit Summary) کا اردو ترجمہ -->

//! ڈیپینڈنسی آڈٹ کا خلاصہ

تاریخ: 2025‑09‑01

دائرۂ کار: workspace کی سطح پر تمام crates کا جائزہ، جو Cargo.toml میں
declare اور Cargo.lock میں resolve ہوئے ہیں۔ `cargo-audit` کو RustSec
advisory DB کے ساتھ چلا کر، اور الگ سے crates کی legitimacy اور الگورتھم کے
لیے “main crates” کے انتخاب کی manual review کی گئی۔

استعمال شدہ ٹولز/کمانڈز:
- `cargo tree -d --workspace --locked --offline` – ڈپلیکیٹ ورژنز کی
  جانچ۔
- `cargo audit` – Cargo.lock کو known vulnerabilities اور yanked crates کے
  لیے scan کیا۔

درج ذیل سیکیورٹی advisories ملے (اب 0 vulns؛ 2 warnings باقی):
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - فِکس: `crates/ivm/Cargo.toml` میں ورژن `0.5.15` پر اپ ڈیٹ کیا گیا۔

  - فِکس: `crates/iroha_torii/Cargo.toml` میں `pprof` کو `prost-codec` پر
    سوئچ کیا گیا۔

- ring — RUSTSEC‑2025‑0009  
  - فِکس: QUIC/TLS stack (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) اپ ڈیٹ کی گئی، اور WS stack کو
    `tungstenite/tokio-tungstenite 0.24` پر اپ ڈیٹ کیا گیا۔ `cargo update -p ring --precise 0.17.12`
    کی مدد سے lock کو `ring 0.17.12` پر فِکس کیا گیا۔

باقی advisories: کوئی نہیں۔ باقی warnings: `backoff` (unmaintained)،
`derivative` (unmaintained)۔

legitimacy اور “main crate” assessment (اہم نکات):
- Hashing: `sha2` (RustCrypto)، `blake2` (RustCrypto)،
  `tiny-keccak` (وسیع پیمانے پر استعمال) — canonical انتخاب۔
- AEAD/سمیٹرک: `aes-gcm`, `chacha20poly1305`, `aead` traits (RustCrypto) —
  canonical۔
- Signatures/ECC: `ed25519-dalek`, `x25519-dalek` (dalek project),
  `k256` (RustCrypto), `secp256k1` (libsecp bindings) — سب legitimate؛
  بہتر ہے کہ secp256k1 کے لیے صرف ایک stack (`k256` خالص Rust یا
  `secp256k1` + libsecp) پر converge کیا جائے، تاکہ surface area کم رہے۔
- BLS12‑381/ZK: `blstrs`, `halo2_*` family — مختلف ZK پروڈکشن
  ecosystem میں عام، اور legitimate۔
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` — معتبر reference crates۔
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — Rust میں جدید اور
  canonical TLS stack۔
- Noise: `snow` — canonical Noise implementation۔
- Serialization: `parity-scale-codec`، SCALE کے لیے canonical codec ہے۔
  پورے workspace میں Serde کو production dependencies سے ہٹا دیا گیا ہے؛
  Norito کے derives/writers تمام runtime paths کو cover کرتے ہیں۔ Serde کی
  کوئی بھی residual references فقط تاریخی documentation، guard scripts یا
  test‑only allowlists تک محدود ہیں۔
- FFI/libs: `libsodium-sys-stable`, `openssl` — legitimate؛ production
  paths میں OpenSSL کے بجائے Rustls کو ترجیح دی جاتی ہے (جیسا کہ
  موجودہ کوڈ پہلے ہی کرتا ہے)۔
  بچنے کے لیے official release کے `prost-codec` + frame‑pointer configuration
  کو استعمال کیا جا رہا ہے۔

سفارشات:
- warnings کو address کریں:
  - `backoff` کو `retry`/`futures-retry` یا کسی local exponential backoff
    helper سے بدلنے پر غور کریں۔
  - جہاں مناسب ہو، `derivative` derives کی جگہ manual impls یا
    `derive_more` اختیار کریں۔
- Medium سطح پر: جہاں ممکن ہو `k256` یا `secp256k1` پر unify کریں، تاکہ
  duplicate implementations کم ہوں (اگر واقعی ضرورت ہو تو دونوں رکھے جا
  سکتے ہیں)۔
- Medium سطح پر: ZK استعمال کی صحبت میں `poseidon-primitives 0.2.0` کی
  provenance کا جائزہ لیں؛ اگر ممکن ہو تو کسی Arkworks/Halo2‑native
  Poseidon implementation کے ساتھ align کریں تاکہ parallel ecosystems کم
  ہوں۔

نوٹس:
- `cargo tree -d` میں متوقع duplicate major versions (`bitflags` 1/2، اور
  متعدد `ring`) ظاہر ہوتی ہیں؛ یہ بذات خود کوئی security risk نہیں،
  البتہ build surface بڑھی ہوئی دکھاتی ہیں۔
- typosquat قسم کے crates نہیں ملے؛ تمام نام اور sources یا تو well‑known
  ecosystem crates یا internal workspace members کی طرف resolve ہوتے ہیں۔
- Experimental: `iroha_crypto` میں feature `bls-backend-blstrs` شامل کیا
  گیا ہے، تاکہ BLS کو صرف `blstrs`‑based backend کی طرف migrate کرنے کا
  سلسلہ شروع کیا جا سکے (feature enable ہونے پر arkworks پر انحصار ختم
  ہو جاتا ہے)۔ ڈیفالٹ فی الحال `w3f-bls` ہی ہے، تاکہ behaviour/encoding
  میں تبدیلی نہ آئے۔ alignment پلان:
  - secret key serialization کو ایک canonical 32‑byte little‑endian output
    پر normalize کریں جو `w3f-bls` اور `blstrs` دونوں کے لیے
    ختم کریں۔
  - public key compression کے لیے ایک explicit wrapper فراہم کریں جو
    `blstrs::G1Affine::to_compressed` استعمال کرے، اور w3f encoding کے خلاف
  - `crates/iroha_crypto/tests/bls_backend_compat.rs` میں round‑trip fixtures
    شامل کریں، جو ایک بار keys derive کر کے دونوں backends (`SecretKey`,
    `PublicKey`, signature aggregation) میں equality assert کریں۔
  - نئے backend کو CI میں feature `bls-backend-blstrs` کے پیچھے gate کریں،
    کسی regression کو backend flip سے پہلے ہی پکڑا جا سکے۔

Follow‑ups (مجوزہ آئٹمز):
- CI میں Serde guardrails (`scripts/check_no_direct_serde.sh`,
  `scripts/deny_serde_json.sh`) برقرار رکھیں، تاکہ production code میں نئے
  Serde استعمالات شامل نہ ہو سکیں۔

اس آڈٹ کے لیے کیے گئے ٹیسٹس:
- 

</div>

