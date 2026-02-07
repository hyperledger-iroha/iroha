---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-primitives
عنوان: سورانیٹ میں پوسٹ کوانٹم کے قدیم
سائڈبار_لیبل: پی کیو قدیم
تفصیل: کریٹ `soranet_pq` کا جائزہ اور کس طرح سورانیٹ ہینڈ شیک ML-KEM/ML-DSA ایڈز کو استعمال کرتا ہے۔
---

::: نوٹ معیاری ماخذ
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ دستاویزات کا پرانا سیٹ ریٹائر ہونے تک دونوں کاپیاں ایک جیسے رکھیں۔
:::

کریٹ `soranet_pq` میں پوسٹ کوانٹم بلڈنگ بلاکس پر مشتمل ہے جس پر ہر ریلے ، مؤکل اور سورنیٹ میں ٹولنگ کا انحصار ہوتا ہے۔ اس نے پی کیو کلین کے تعاون سے کیبر (ایم ایل-کے ای ایم) اور دلیتھیم (ایم ایل-ڈی ایس اے) ڈیکوں کو لپیٹ لیا ہے اور ایچ کے ڈی ایف اور آر این جی کے مددگاروں کو پروٹوکول میں شامل کیا ہے تاکہ تمام کھالیں ایک ہی عمل میں شریک ہوں۔

## `soranet_pq` کے ساتھ کیا شامل ہے

۔
-** ML-DSA-44/65/87: ** دائرہ کار سے الگ ہونے والے اسکرپٹس سے منسلک علیحدہ دستخط/توثیق۔
۔
۔

تمام راز `Zeroizing` کنٹینرز اور CI ٹیسٹ کے اندر تمام معاون پلیٹ فارمز پر PQCLEAN پابندیوں کے اندر رہتے ہیں۔

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

1. ** ورک اسپیس کی جڑ سے باہر کے کریٹوں میں اسناد شامل کریں **:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** کال پوائنٹس پر صحیح گروپ ** کا انتخاب کریں۔ ہائبرڈ ہینڈ شیک پر ابتدائی کام کے لئے ، `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. ** ٹیگز کے ساتھ چابیاں اخذ کریں۔

4. ** متبادل رازوں کے نمونے لینے پر RNG ہیجڈ ** استعمال کریں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

سورانیٹ کور ہینڈ شیک اور سی آئی ڈی انکرپشن مددگار (`iroha_crypto::soranet`) ان کو براہ راست کھینچتے ہیں ، اس کا مطلب ہے کہ بچوں کے کریٹ خود پی کیو کلین کو پابند کیے بغیر ایک ہی عمل درآمد کا وارث ہوتے ہیں۔

## توثیق چیک لسٹ

- `cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- README (`crates/soranet_pq/README.md`) میں استعمال کی مثالیں دیکھیں
- جب ہائبرڈز پہنچے تو سورانیٹ ہینڈ شیک ڈیزائن دستاویز کو اپ ڈیٹ کریں