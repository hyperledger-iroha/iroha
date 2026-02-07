---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: Postagens de propriedade SoraNet
sidebar_label: PQ примитивы
description: Abra a caixa `soranet_pq` e também, como o SoraNet usa ajudantes ML-KEM/ML-DSA.
---

:::nota História Canônica
Esta página está configurada para `docs/source/soranet/pq_primitives.md`. Faça cópias na sincronização, mas a documentação não será exibida na tela.
:::

Crate `soranet_pq` contém blocos de construção que permitem a operação de seus relés, clientes e componentes de ferramentas SoraNet. Ao usar Kyber (ML-KEM) e Dilithium (ML-DSA) em PQClean e usar ajudantes de protocolo HKDF e RNG protegido, você também pode usá-los поверхности разделяли идентичные реализации.

## O que você precisa em `soranet_pq`

- **ML-KEM-512/768/1024:** детерминированная генерация ключей, ajudantes инкапсуляции и декапсуляции с распространением ошибок no período constante.
- **ML-DSA-44/65/87:** отделенная подпись/проверка, привязанная к транскриптам с разделением домена.
- **Помеченный HKDF:** `derive_labeled_hkdf` добавляет namespace para каждого вывода с указанием стадии рукопожатия (`DH/es`, `KEM/1`, ...), esta transcrição não é estável.
- **Conclusão coberta:** `hedged_chacha20_rng` смешивает детерминированные sementes com живой энтропией ОС и обнуляет promежуточное состояние при освобождении.

Seus segredos são encontrados no contêiner `Zeroizing`, e o CI armazena ligações PQClean em sua plataforma mais recente.

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

## Como usar

1. **Fazer a configuração** em caixas na área de trabalho:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Você pode usar o recurso** no seu computador. Para o aperto de mão dinâmico, o aperto de mão usa `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Vыводите ключи с метками.** Use `HkdfDomain::soranet("KEM/1")` (e родственные), чтобы цепочка транскриптов оставалась детерминированной между узлами.

4. **Gerar RNG protegido** para abrir segredos:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро рукопожатия SoraNet e helpers ослепления CID (`iroha_crypto::soranet`) используют эти утилиты напрямую, поэтому downstream crates наследуют те Não há necessidade de vincular ligações PQClean.

## Verifique as receitas

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Verifique os primeiros exemplos usados no README (`crates/soranet_pq/README.md`)
- Обновите документ дизайна рукопожатия SoraNet после появления гибридов