---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-примитивы
Название: Пост-квантовые примитивы SoraNet
sidebar_label: Примитивы PQ
описание: Обзор крейта `soranet_pq` и того, как при подтверждении связи SoraNet используются помощники ML-KEM/ML-DSA.
---

:::обратите внимание на канонический источник
Эта страница отражает `docs/source/soranet/pq_primitives.md`. Синхронизируйте обе копии до тех пор, пока старый набор документации не будет удален.
:::

Ящик `soranet_pq` содержит постквантовые строительные блоки, на которых основано каждое реле, клиент и инструментальный компонент SoraNet. Он включает в себя пакеты Kyber (ML-KEM) и Dilithium (ML-DSA) на базе PQClean и предоставляет удобные для протоколов HKDF и хеджированные помощники RNG, так что все поверхности используют одни и те же реализации.

## Что включено в `soranet_pq`

- **ML-KEM-512/768/1024:** Постоянное распространение ошибок с детерминированной генерацией ключей, помощниками инкапсуляции и декапсуляции.
- **ML-DSA-44/65/87:** Отдельная подпись/проверка, подключенная для транскриптов, разделенных доменами.
- **Помечено как HKDF:** `derive_labeled_hkdf` помещает каждое производное пространство имен на этап подтверждения (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипты не допускали коллизий.
- **Хеджированная случайность:** `hedged_chacha20_rng` смешивает детерминированные начальные значения с энтропией активной ОС и обнуляет промежуточное состояние при удалении.

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

## Как использовать

1. **Добавьте зависимости** в ящики за пределами корня рабочей области:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный пакет** на сайтах вызовов. Используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65` для начального гибридного подтверждения.

3. **Выведите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и аналогичный), чтобы цепочка транскриптов была детерминированной между узлами.

4. **Используйте хеджированный RNG** при выборке резервных секретов:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Основные помощники SoraNet по установлению связи и ослеплению CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, что означает, что нижестоящие крейты наследуют одни и те же реализации без связывания привязок PQClean.

## Контрольный список проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Аудит примеров использования README (`crates/soranet_pq/README.md`)
- Обновить документацию по проектированию рукопожатия SoraNet после появления гибридов.