---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: Постквантовые примитивы SoraNet
sidebar_label: PQ примитивы
descripción: Обзор crate `soranet_pq` y того, как рукопожатие SoraNet utiliza los ayudantes ML-KEM/ML-DSA.
---

:::nota Канонический источник
Esta página contiene la letra `docs/source/soranet/pq_primitives.md`. Deje copias de las sincronizaciones, pero no guarde documentos exclusivos.
:::

Crate `soranet_pq` содержит постквантовые строительные блоки, на которые опираются все relés, clientes y componentes de herramientas SoraNet. Utilizando Kyber (ML-KEM) y Dilithium (ML-DSA) en base a PQClean y añadiendo protocolos de ayuda HKDF y RNG con cobertura, entre otros. разделяли идентичные реализации.

## Что входит в `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, helpers инкапсуляции и декапсуляции с распространением ошибок за константное время.
- **ML-DSA-44/65/87:** отделенная подпись/проверка, привязанная к транскриптам с разделением domена.
- **Помеченный HKDF:** `derive_labeled_hkdf` incluye el espacio de nombres para el archivo de ubicación de las estaciones de almacenamiento (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипты не сталкивались.
- **Hedged случайность:** `hedged_chacha20_rng` combina semillas determinadas con un sistema operativo autónomo y está disponible para su instalación. освобождении.Все секреты находятся в контейнерах `Zeroizing`, а CI запускает ПQClean на всех поддерживаемых платформах.

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

2. **Выберите правильный набор** в местах вызова. Para los robots de protocolo de enlace inalámbricos utilizados se utilizan `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Выводите ключи с метками.** Utilice `HkdfDomain::soranet("KEM/1")` (y родственные), чтобы цепочка транскриптов оставалась. детерминированной между узлами.

4. **Utilice RNG cubierto** para crear secretos:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

La configuración de SoraNet y los asistentes de configuración CID (`iroha_crypto::soranet`) implementan estas utilidades, las cajas descendentes utilizadas en estos casos Realice las tareas de limpieza con las fijaciones PQClean.

## Чеклист проверки

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Primeros archivos implementados en README (`crates/soranet_pq/README.md`)
- Обновите документ дизайна рукопожатия SoraNet после появления гибридов