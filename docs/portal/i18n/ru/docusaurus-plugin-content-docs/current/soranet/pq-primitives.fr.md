---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-примитивы
Название: Постквантовые примитивы SoraNet
Sidebar_label: примитивы PQ
описание: Обзор ящика `soranet_pq` и того, как подтверждение связи SoraNet использует помощники ML-KEM/ML-DSA.
---

:::примечание Канонический источник
:::

В ящике `soranet_pq` содержатся постквантовые блоки, на которых основаны все реле, клиенты и инструментальные компоненты SoraNet. Он инкапсулирует пакеты Kyber (ML-KEM) и Dilithium (ML-DSA), поддерживаемые PQClean, и добавляет HKDF и хеджированные помощники RNG, адаптированные к протоколу, так что все поверхности имеют одинаковую реализацию.

## Что поставляется в `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, инкапсуляция и декапсуляция с постоянным распространением ошибок во времени.
- **ML-DSA-44/65/87:** отдельная подпись/проверка с транскрипцией, разделенной доменом.
- **HKDF с маркировкой:** `derive_labeled_hkdf` применяет пространство имен к каждому производному посредством этапа установления связи (`DH/es`, `KEM/1`, ...), так что гибридные транскрипции остаются без коллизий.
- **Случайное хеджирование:** `hedged_chacha20_rng` объединяет детерминированные начальные значения с энтропией системы и обнуляет промежуточное состояние при разрушении.

Все секреты хранятся в контейнерах `Zeroizing`, а CI выполняет привязки PQClean на всех поддерживаемых платформах.

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

## Как это использовать

1. **Добавьте зависимость** к ящикам за пределами корня рабочей области:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильную последовательность** в точках вызова. Для первоначального гибридного подтверждения используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65`.

3. **Выведите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и эквиваленты), чтобы сохранить детерминированную последовательность транскрипций между узлами.

4. **Используйте хеджированный ГСЧ** для получения запасных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Центральное рукопожатие SoraNet и помощники по экранированию CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, а это означает, что нижестоящие крейты наследуют одни и те же реализации без привязки самих привязок PQClean.

## Контрольный список проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Проверьте примеры использования README (`crates/soranet_pq/README.md`).
- Обновите проектный документ квитирования SoraNet по прибытии гибридов.