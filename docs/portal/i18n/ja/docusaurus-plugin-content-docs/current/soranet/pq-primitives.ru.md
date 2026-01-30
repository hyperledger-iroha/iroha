---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-primitives
title: Постквантовые примитивы SoraNet
sidebar_label: PQ примитивы
description: Обзор crate `soranet_pq` и того, как рукопожатие SoraNet использует helpers ML-KEM/ML-DSA.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_primitives.md`. Держите обе копии в синхронизации, пока набор устаревшей документации не будет выведен из эксплуатации.
:::

Crate `soranet_pq` содержит постквантовые строительные блоки, на которые опираются все relays, clients и tooling компоненты SoraNet. Он оборачивает наборы Kyber (ML-KEM) и Dilithium (ML-DSA) на базе PQClean и добавляет дружественные протоколу helpers HKDF и hedged RNG, чтобы все поверхности разделяли идентичные реализации.

## Что входит в `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, helpers инкапсуляции и декапсуляции с распространением ошибок за константное время.
- **ML-DSA-44/65/87:** отделенная подпись/проверка, привязанная к транскриптам с разделением домена.
- **Помеченный HKDF:** `derive_labeled_hkdf` добавляет namespace для каждого вывода с указанием стадии рукопожатия (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипты не сталкивались.
- **Hedged случайность:** `hedged_chacha20_rng` смешивает детерминированные seeds с живой энтропией ОС и обнуляет промежуточное состояние при освобождении.

Все секреты находятся в контейнерах `Zeroizing`, а CI запускает bindings PQClean на всех поддерживаемых платформах.

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

1. **Добавьте зависимость** в crates вне корня workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный набор** в местах вызова. Для начальной гибридной handshaking работы используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65`.

3. **Выводите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и родственные), чтобы цепочка транскриптов оставалась детерминированной между узлами.

4. **Используйте hedged RNG** при выборке запасных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро рукопожатия SoraNet и helpers ослепления CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, поэтому downstream crates наследуют те же реализации без необходимости линковать bindings PQClean самостоятельно.

## Чеклист проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Проверьте примеры использования в README (`crates/soranet_pq/README.md`)
- Обновите документ дизайна рукопожатия SoraNet после появления гибридов
