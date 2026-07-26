---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-primitives
عنوان: سورانیٹ پوسٹ کوانٹم قدیم
سائڈبار_لیبل: پی کیو قدیم
تفصیل: `soranet_pq` کریٹ کا جائزہ اور کس طرح سورانیٹ ہینڈ شیک ML-KEM/ML-DSA مددگار استعمال کرتا ہے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_primitives.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

`soranet_pq` کریٹ میں پوسٹ کوانٹم بلاکس پر مشتمل ہے جس پر ہر سورنیٹ ریلے ، کلائنٹ اور ٹولنگ جزو پر انحصار کرتا ہے۔ یہ پی کیو کلین کے تعاون سے کیبر (ایم ایل کے ای ایم) اور دلیتھیم (ایم ایل-ڈی ایس اے) سوٹ کو لپیٹتا ہے اور صارف دوست HKDF اور ہیجڈ RNG مددگاروں کو پروٹوکول میں شامل کرتا ہے تاکہ تمام سطحیں ایک جیسی نفاذ کا اشتراک کریں۔

## `soranet_pq` میں کیا آتا ہے

۔
-** ML-DSA-44/65/87: ** ڈومین سے الگ شدہ نقلوں کے لئے الگ الگ دستخط/توثیق۔
۔
۔

تمام راز `Zeroizing` کنٹینرز کے اندر ہیں اور CI تمام تائید شدہ پلیٹ فارمز پر PQClean پابندیوں کی مشق کرتا ہے۔

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

## کس طرح استعمال کریں

1. ** انحصار ** کو ورک اسپیس روٹ سے باہر کریٹوں میں شامل کریں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** کال پوائنٹس پر صحیح سویٹ ** منتخب کریں۔ ابتدائی ہائبرڈ ہینڈ شیک کام کے لئے ، `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. ** لیبلوں کے ساتھ چابیاں اخذ کریں۔

4. ** فال بیک بیک راز کے نمونے لینے پر ہیجڈ آر این جی ** استعمال کریں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

سورانیٹ کے بنیادی مصافحہ اور سی آئی ڈی بلائنڈنگ مددگار (`iroha_crypto::soranet`) ان افادیت کو براہ راست کھینچیں ، اس کا مطلب یہ ہے کہ بہاو کریٹ پی کیو کلین پابندیوں کو اپنے ہی جوڑنے کے بغیر ایک ہی عمل درآمد کا وارث ہیں۔

## توثیق چیک لسٹ

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README (`crates/soranet_pq/README.md`) میں استعمال کی مثالوں کا آڈٹ کریں
- جب ہائبرڈ پہنچیں تو سورانیٹ ہینڈ شیک ڈیزائن دستاویز کو اپ ڈیٹ کریں