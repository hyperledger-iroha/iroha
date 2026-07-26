---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-primitives
عنوان: سورانیٹ پوسٹ کوانٹم قدیم
سائڈبار_لیبل: پی کیو قدیم
تفصیل: کریٹ `soranet_pq` کا ایک جائزہ اور کس طرح سورانیٹ ہینڈ شیک ML-KEM/ML-DSA مددگار استعمال کرتا ہے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_primitives.md` کا آئینہ دار ہے۔ جب تک میراثی دستاویزات کا سیٹ ریٹائر نہ ہو تب تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

کریٹ `soranet_pq` میں پوسٹ کوانٹم بلڈنگ بلاکس شامل ہیں جس پر سورنیٹ کے تمام ریلے ، کلائنٹ اور ٹولنگ اجزاء انحصار کرتے ہیں۔ اس نے پی کیو کلین میں مقیم کیبر (ایم ایل کے ای ایم) اور دلیتھیم (ایم ایل-ڈی ایس اے) کٹس کو لپیٹا ہے اور پروٹوکول دوستانہ ایچ کے ڈی ایف اور ہیجڈ آر این جی مددگاروں کو شامل کیا ہے تاکہ تمام سطحیں یکساں نفاذ کا اشتراک کریں۔

## `soranet_pq` میں کیا شامل ہے

۔
۔
۔
۔

تمام راز `Zeroizing` کنٹینرز میں ہیں ، اور CI تمام معاون پلیٹ فارمز پر PQClean پابند چلاتا ہے۔

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

## استعمال کرنے کا طریقہ

1. ** ورک اسپیس روٹ سے باہر کے کریٹوں میں انحصار ** شامل کریں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** کالنگ مقامات میں صحیح ڈائلنگ ** منتخب کریں۔ ابتدائی ہائبرڈ ہینڈ شیکنگ کام کے لئے ، `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. ** آؤٹ پٹ لیبل والی چابیاں۔

4. ** اسپیئر راز لانے کے وقت ہیجڈ آر این جی ** استعمال کریں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

سورانیٹ ہینڈ شیک کور اور سی آئی ڈی بلائنڈنگ مددگار (`iroha_crypto::soranet`) ان افادیت کو براہ راست استعمال کرتے ہیں ، لہذا بہاو کریٹ پی کیو کلین پابندیوں کو خود سے جوڑنے کے بغیر ایک ہی عمل درآمد کا وارث ہوتے ہیں۔

## چیک لسٹ

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README (`crates/soranet_pq/README.md`) میں استعمال کی مثالوں کو چیک کریں
- ایک بار ہائبرڈ متعارف کرانے کے بعد سورانیٹ ہینڈ شیک ڈیزائن دستاویز کو اپ ڈیٹ کریں