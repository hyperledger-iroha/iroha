---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-примитивы
Название: Постквантовые примитивы в SoraNet
Sidebar_label: примитивы PQ
описание: Обзор ящика `soranet_pq` и того, как подтверждение связи SoraNet использует средства ML-KEM/ML-DSA.
---

:::note Стандартный источник
Эта страница отражает `docs/source/soranet/pq_primitives.md`. Сохраняйте две копии идентичными до тех пор, пока старый комплект документов не будет удален.
:::

Ящик `soranet_pq` содержит постквантовые строительные блоки, от которых зависит каждое реле, клиент и инструментарий в SoraNet. Он включает в себя колоды Kyber (ML-KEM) и Dilithium (ML-DSA), поддерживаемые PQClean, и добавляет хелперы для HKDF и RNG, хеджированные в протокол, так что все скины имеют одну и ту же реализацию.

## Что включено в `soranet_pq`

- **ML-KEM-512/768/1024:** Детерминированная генерация ключей, помощники инкапсуляции и декапсуляции с распространением ошибок с фиксированным временем.
- **ML-DSA-44/65/87:** Отдельная подпись/проверка, связанная со сценариями с разделенной областью действия.
- **Тег HKDF:** `derive_labeled_hkdf` добавляет пространство имен к каждому производному с фазой установления связи (`DH/es`, `KEM/1`, ...), чтобы гибридные строки оставались без коллизий.
- **Хеджированная случайность:** `hedged_chacha20_rng` смешивает детерминированные начальные значения с энтропией системы и обнуляет промежуточное состояние при выпуске.

Все секреты хранятся внутри контейнеров `Zeroizing`, а CI тестирует привязки PQClean на всех поддерживаемых платформах.

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

1. **Добавьте учетные данные** в ящики за пределами корня рабочей области:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильную группу** на точках вызова. Для первоначальной работы над гибридным подтверждением используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65`.

3. **Получайте ключи с помощью тегов.** Используйте `HkdfDomain::soranet("KEM/1")` (и его аналоги), чтобы текстовые последовательности оставались детерминированными между узлами.

4. **Используйте хеджированный RNG** при выборке альтернативных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро SoraNet для установления связи и помощники шифрования CID (`iroha_crypto::soranet`) извлекают их напрямую, а это означает, что дочерние ящики наследуют одни и те же реализации без привязки самих PQClean.

## Контрольный список проверки

- `cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- См. примеры использования в README (`crates/soranet_pq/README.md`).
- Обновлен документ по проектированию рукопожатия SoraNet при появлении гибридов.