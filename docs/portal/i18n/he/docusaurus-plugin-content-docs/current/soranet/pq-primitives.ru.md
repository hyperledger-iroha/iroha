---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitives
כותרת: Постквантовые примитивы SoraNet
sidebar_label: PQ примитивы
תיאור: Обзор crate `soranet_pq` и того, как рукопожатие SoraNet использует helpers ML-KEM/ML-DSA.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_primitives.md`. צור מסמכים נוספים בסין.
:::

ארגז `soranet_pq` содержит постквантовые строительные блоки, на которые опираются все ממסרים, לקוחות ומכשירי כלים של SoraNet. Он оборачивает наборы Kyber (ML-KEM) and Dilithium (ML-DSA) обазе PQClean и добавляет дружественные протоколу helpers HKDF и hedged Rg, разделяли идентичные реализации.

## Что входит ב-`soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, helpers инкапсуляции и декапсуляции с распроста константное время.
- **ML-DSA-44/65/87:** отделенная подпись/проверка, привязанная к транскриптам с разделением домена.
- **Помеченный HKDF:** `derive_labeled_hkdf` добавляет מרחב שמות для каждого вывода с указанием стадии рукопожатия,018NI080X (I018NI0080X, `KEM/1`, ...), чтобы гибридные транскрипты не сталкивались.
- **סלוטש מגודר:** `hedged_chacha20_rng` смешивает детерминированные זרעים עם живой энтропией при освобождении.

Все секреты находятся в контейнерах `Zeroizing`, а CI запускает bindings PQClean всех поддерживаемпорх.

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

## Как использовать

1. **Добавьте зависимость** в crates вне корня סביבת עבודה:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный набор** в местах вызова. Для начальной гибридной לחיצת יד работы используйте `MlKemSuite::MlKem768` ו `MlDsaSuite::MlDsa65`.

3. **Выводите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (ו родственные), чтобы цепочка транскриптов детерминированной между узлами.

4. **Используйте hedged RNG** при выборке запасных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро рукопожатия SoraNet и helpers ослепления CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, послепления במורד הזרם реализации без необходимости линковать bindings PQClean самостоятельно.

## Чеклист проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Проверьте примеры использования в README (`crates/soranet_pq/README.md`)
- אישור דוקומנט рукопожатия SoraNet после появления гибридов