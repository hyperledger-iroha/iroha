---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: Primitivos pós-quânticos SoraNet
sidebar_label: Primitivos PQ
description: Caixa `soranet_pq` کا visão geral اور یہ کہ Handshake SoraNet Ajudantes ML-KEM/ML-DSA کو کیسے استعمال کرتا ہے۔
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentação retirar نہ ہو، دونوں کاپیاں sincronização رکھیں۔
:::

Caixa `soranet_pq` ان blocos de construção pós-quânticos پر مشتمل ہے جن پر ہر Relé SoraNet, cliente اور componente de ferramentas انحصار کرتا ہے۔ یہ Suítes Kyber (ML-KEM) e Dilithium (ML-DSA) apoiadas por PQClean کو wrap کرتا ہے اور HKDF compatível com protocolo اور ajudantes RNG protegidos فراہم کرتا ہے تاکہ تمام superfícies ایک جیسی implementações compartilham کریں۔

## `soranet_pq` میں کیا شامل ہے

- **ML-KEM-512/768/1024:** geração de chave determinística, encapsulamento e auxiliares de desencapsulação e propagação de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificação separada e transcrições separadas por domínio کے لئے com fio ہے.
- **Rotulado HKDF:** `derive_labeled_hkdf` ہر derivação کو estágio de handshake (`DH/es`, `KEM/1`, ...) کے ساتھ namespace دیتا ہے تاکہ transcrições híbridas livres de colisão رہیں.
- **Aleatoriedade protegida:** `hedged_chacha20_rng` sementes determinísticas کو entropia de sistema operacional ao vivo کے ساتھ mistura کرتا ہے اور queda پر estado intermediário کو zerar کرتا ہے.

تمام segredos `Zeroizing` contêineres کے اندر رہتے ہیں اور CI تمام plataformas suportadas پر ligações PQClean کو exercício کرتی ہے۔

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

## استعمال کیسے کریں

1. **Dependência شامل کریں** ان crates میں جو raiz do espaço de trabalho سے باہر ہوں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **درست suite منتخب کریں** chamar sites پر۔ O handshake híbrido é o `MlKemSuite::MlKem768` e o `MlDsaSuite::MlDsa65`.

3. **Rótulos کے ساتھ chaves derivam کریں۔** `HkdfDomain::soranet("KEM/1")` (اور اس جیسے) استعمال کریں تاکہ nós de encadeamento de transcrição کے درمیان determinístico رہے۔

4. **Hedged RNG استعمال کریں** جب amostra de segredos alternativos کر رہے ہوں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet کا core handshake اور CID blinding helpers (`iroha_crypto::soranet`) ان utilitários کو براہ راست استعمال کرتے ہیں, جس کا مطلب ہے کہ caixas downstream بغیر PQClean vinculações link کئے وہی implementações herdam کرتے ہیں۔

## Lista de verificação de validação

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Amostras de uso README کا auditoria کریں (`crates/soranet_pq/README.md`)
- híbridos آنے کے بعد Documento de design de handshake SoraNet اپ ڈیٹ کریں