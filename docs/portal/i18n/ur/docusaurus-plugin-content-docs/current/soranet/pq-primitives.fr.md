---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-primitives
عنوان: سورانیٹ پوسٹ کوانٹم قدیم
سائڈبار_لیبل: پی کیو قدیم
تفصیل: کریٹ `soranet_pq` کا جائزہ اور کس طرح سورانیٹ ہینڈ شیک ML-KEM/ML-DSA مددگاروں کو استعمال کرتا ہے۔
---

::: نوٹ کینونیکل ماخذ
:::

کریٹ `soranet_pq` میں پوسٹ کوانٹم اینٹوں پر مشتمل ہے جس پر تمام سورانیٹ ریلے ، کلائنٹ اور ٹولنگ اجزاء پر مبنی ہیں۔ اس میں پی کیو کلین کے تعاون سے کیبر (ایم ایل-کے ای ایم) اور دلیتھیم (ایم ایل-ڈی ایس اے) سوٹ کو شامل کیا گیا ہے اور اس میں ایچ کے ڈی ایف اور ہیجڈ آر این جی مددگار شامل ہیں جو پروٹوکول کے مطابق ڈھال گئے ہیں تاکہ تمام سطحیں ایک جیسی نفاذ کا اشتراک کریں۔

## `soranet_pq` میں کیا فراہم کیا جاتا ہے

۔
-** ML-DSA-44/65/87: ** ڈومین سے الگ ٹرانسکرپشن کے ساتھ الگ الگ دستخط/توثیق۔
۔
۔

تمام راز `Zeroizing` کنٹینرز میں رہتے ہیں اور CI تمام معاون پلیٹ فارمز پر PQCLEAN پابندیوں کی مشق کرتا ہے۔

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## اسے کیسے استعمال کریں

1. ** ورک اسپیس روٹ کے باہر کریٹس میں انحصار ** شامل کریں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** کال پوائنٹس پر صحیح ترتیب ** منتخب کریں۔ ابتدائی ہائبرڈ ہینڈ شیک کام کے لئے ، `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. ** لیبلوں کے ساتھ چابیاں اخذ کریں۔

4. ** نمونے کے لئے ہیجڈ آر این جی ** کا استعمال کریں فال بیک بیک راز:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

سورانیٹ کے مرکزی مصافحہ اور سی آئی ڈی شیلڈنگ مددگار (`iroha_crypto::soranet`) ان افادیت کو براہ راست استعمال کرتے ہیں ، اس کا مطلب یہ ہے کہ بہاو کریٹ پی کیو کلین پابندیاں خود کو پابند کیے بغیر ایک ہی عمل درآمد کا وارث ہیں۔

## توثیق چیک لسٹ

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- آڈٹ کے استعمال کی مثالوں (`crates/soranet_pq/README.md`)
- جب ہائبرڈس آتے ہیں تو سورانیٹ ہینڈ شیک ڈیزائن دستاویز کو اپ ڈیٹ کریں