---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-примитивы
Название: Постквантовые примитивы SoraNet
sidebar_label: Примитивы PQ
описание: Обзор крейта `soranet_pq` и того, как рукопожатие SoraNet использует помощники ML-KEM/ML-DSA.
---

:::примечание Канонический источник
Эта страница отражает `docs/source/soranet/pq_primitives.md`. Синхронизируйте обе копии.
:::

Ящик `soranet_pq` содержит постквантовые блоки, от которых зависит каждое реле, клиент и инструментальный компонент SoraNet. Он включает в себя пакеты Kyber (ML-KEM) и Dilithium (ML-DSA), поддерживаемые PQClean, и добавляет в протокол удобные для пользователя помощники HKDF и хеджированные RNG, чтобы все поверхности имели одинаковую реализацию.

## Что входит в `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, помощники инкапсуляции и декапсуляции с распространением ошибок за постоянное время.
- **ML-DSA-44/65/87:** отдельная подпись/проверка транскриптов, разделенных доменами.
- **HKDF с меткой:** `derive_labeled_hkdf` добавляет пространство имен к каждой ветке на этапе установления связи (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипции не допускали коллизий.
- **Хеджированная случайность:** `hedged_chacha20_rng` смешивает детерминированные начальные числа с энтропией ОС и сбрасывает промежуточное состояние при отбрасывании.

Все секреты находятся внутри контейнеров `Zeroizing`, а CI выполняет привязки PQClean на всех поддерживаемых платформах.

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

## Как употреблять

1. **Добавьте зависимость** к ящикам за пределами корня рабочей области:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный пакет** на точках вызова. Для первоначального гибридного подтверждения используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65`.

3. **Выведите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и аналогичный), чтобы цепочка транскрипции была детерминированной между узлами.

4. **Используйте хеджированный RNG** при выборке резервных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Основные помощники SoraNet по установлению связи и ослеплению CID (`iroha_crypto::soranet`) извлекают эти утилиты напрямую, а это означает, что нижестоящие крейты наследуют одни и те же реализации без связывания собственных привязок PQClean.

## Контрольный список проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Проверьте примеры использования в README (`crates/soranet_pq/README.md`).
- Обновление документа по проектированию рукопожатия SoraNet при появлении гибридов.