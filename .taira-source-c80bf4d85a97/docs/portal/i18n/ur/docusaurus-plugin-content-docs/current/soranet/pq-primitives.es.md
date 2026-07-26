---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-primitives
عنوان: سورانیٹ پوسٹ کوینٹم قدیم
سائڈبار_لیبل: پی کیو قدیم
تفصیل: کریٹ `soranet_pq` کا خلاصہ اور کس طرح سورنیٹ ہینڈ شیک ML-KEM/ML-DSA مددگار استعمال کرتا ہے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ جب تک میراثی دستاویزات کا سیٹ ریٹائر نہ ہو تب تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

`soranet_pq` کریٹ میں پوسٹ کے بعد کے بلاکس پر مشتمل ہے جس پر تمام سورانیٹ ریلے ، مؤکلوں اور ٹولنگ اجزاء کے ذریعہ انحصار کیا جاتا ہے۔ اس نے پی کیو کلین کی حمایت یافتہ کیبر (ایم ایل-کے ای ایم) اور دلیتھیم (ایم ایل-ڈی ایس اے) سوٹ کو لپیٹ لیا ہے اور پروٹوکول دوستانہ ہیجڈ ایچ کے ڈی ایف اور آر این جی مددگاروں کو شامل کیا ہے تاکہ تمام سطحیں یکساں نفاذ کا اشتراک کریں۔

## جس میں `soranet_pq` شامل ہے

۔
-** ML-DSA-44/65/87: ** ڈومین علیحدگی کے ساتھ نقلوں کے لئے الگ الگ دستخط/توثیق۔
۔
۔

تمام راز `Zeroizing` کنٹینرز کے اندر رہتے ہیں اور CI تمام معاون پلیٹ فارمز پر PQCLEAN پابندیوں کی مشق کرتے ہیں۔

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

1. ** انحصار ** ان خانوں میں شامل کریں جو جڑ کے کام سے باہر ہیں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** کال پوائنٹس پر صحیح سویٹ ** منتخب کریں۔ ہائبرڈ ہینڈ شیک کے ابتدائی کام کے لئے ، `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. ** لیبلوں کے ساتھ چابیاں اخذ کریں۔

4. ** بیک اپ راز کے نمونے لینے پر ہیجڈ آر این جی ** استعمال کریں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

سورنیٹ کور ہینڈ شیک اور سی آئی ڈی شیلڈنگ مددگار (`iroha_crypto::soranet`) ان افادیت کو براہ راست استعمال کرتے ہیں ، اس کا مطلب ہے کہ بہاو کریٹ خود پی کیو کلین پابند ہونے کے بغیر ایک ہی عمل درآمد کا وارث ہوتے ہیں۔

## توثیق چیک لسٹ

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- آڈٹ ریڈم (`crates/soranet_pq/README.md`) میں استعمال کی مثالوں کا آڈٹ کریں
- جب ہائبرڈس آتے ہیں تو سورانیٹ ہینڈ شیک ڈیزائن دستاویز کو اپ ڈیٹ کریں