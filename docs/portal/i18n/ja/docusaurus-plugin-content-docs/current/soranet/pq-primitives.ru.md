---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
タイトル: Постквантовые примитивы SoraNet
サイドバーラベル: PQ の表示
説明: クレート `soranet_pq` と、SoraNet ヘルパー ML-KEM/ML-DSA の説明。
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_primitives.md`. Собе копии в синхронизации, пока набор устаревлей документации не будет выведен из эксплуатации.
:::

クレート `soranet_pq` は、SoraNet のリレー、クライアント、ツールを提供します。 Kyber (ML-KEM) と Dilithium (ML-DSA) の組み合わせ、PQClean と добавляет дружественные протоколу ヘルパー HKDF とヘッジ RNG、чтобы最高です。

## Что входит в `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей、ヘルパー инкапсуляции и декапсуляции с распространением овибок за константное время.
- **ML-DSA-44/65/87:** отделенная подпись/проверка、привязанная к транскриптам с разделением домена.
- **Помеченный HKDF:** `derive_labeled_hkdf` добавляет 名前空間 для каждого вывода с указанием стадии рукопожатия (`DH/es`, `KEM/1`, ...)、чтобы гибридные транскрипты не сталкивались。
- **ヘッジ済み случайность:** `hedged_chacha20_rng` смезивает детерминированные 種子 с живой энтропией ОС и обнуляет промежуточное состояние при освобождении.

`Zeroizing`、CI とバインディング PQClean の組み合わせが、PQClean の機能をサポートします。

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

1. **ワークスペースのクレート**:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный набор** в местах вызова.ハンドシェイクが `MlKemSuite::MlKem768` および `MlDsaSuite::MlDsa65` で行われます。

3. **Выводите ключи с метками.** Используйте `HkdfDomain::soranet("KEM/1")` (и родственные)、чтобы цепочка транскриптов оставалась детерминированной между узлами。

4. **ヘッジ RNG** の詳細:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet とヘルパーの CID (`iroha_crypto::soranet`) を使用して、ダウンストリーム クレートの機能を確認します。バインディング PQClean を使用してください。

## Чеклист проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README (`crates/soranet_pq/README.md`) の説明
- Обновите документ дизайна рукопожатия SoraNet после появления гибридов