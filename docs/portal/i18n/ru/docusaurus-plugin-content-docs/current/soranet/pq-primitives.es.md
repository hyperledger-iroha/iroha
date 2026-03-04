---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-примитивы
Название: Постквантовые примитивы SoraNet
Sidebar_label: примитивы PQ
описание: Краткое описание ящика `soranet_pq` и того, как рукопожатие SoraNet использует помощники ML-KEM/ML-DSA.
---

:::примечание Канонический источник
Эта страница отражает `docs/source/soranet/pq_primitives.md`. Синхронизируйте обе версии до тех пор, пока устаревший набор документации не будет удален.
:::

Ящик `soranet_pq` содержит постквантовые блоки, используемые всеми реле, клиентами и инструментальными компонентами SoraNet. Он включает в себя пакеты Kyber (ML-KEM) и Dilithium (ML-DSA) на базе PQClean и добавляет удобные для протоколов хеджированные помощники HKDF и RNG, так что все поверхности имеют одинаковую реализацию.

## Что включает в себя `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, инкапсуляция и декапсуляция с распространением ошибок за постоянное время.
- **ML-DSA-44/65/87:** Отдельное подписание/проверка транскриптов с разделением доменов.
- **HKDF с тегами:** `derive_labeled_hkdf` добавляет пространство имен к каждой ветке на этапе установления связи (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипты не конфликтовали.
- **Хеджированная случайность:** `hedged_chacha20_rng` смешивает детерминированные начальные значения с энтропией ОС и сбрасывает промежуточное состояние в ноль при освобождении ресурсов.

Все секреты хранятся внутри контейнеров `Zeroizing`, а CI выполняет привязки PQClean на всех поддерживаемых платформах.

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

## Как его употреблять

1. **Добавьте зависимость** к ящикам, находящимся за пределами корневой рабочей области:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный пакет** на точках вызова. Для первоначальной работы гибридного рукопожатия используйте `MlKemSuite::MlKem768` и `MlDsaSuite::MlDsa65`.

3. **Выведите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и эквиваленты), чтобы цепочка транскриптов оставалась детерминированной между узлами.

4. **Используйте хеджированный RNG** при выборке секретов резервной копии:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро SoraNet для установления связи и помощники по экранированию CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, а это означает, что нижестоящие крейты наследуют одни и те же реализации без привязки самих привязок PQClean.

## Контрольный список проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Проверьте примеры использования в README (`crates/soranet_pq/README.md`)
- Обновите проектный документ квитирования SoraNet по прибытии гибридов.