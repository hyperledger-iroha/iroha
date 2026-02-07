---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitives
כותרת: Primitivos pos-quanticos do SoraNet
sidebar_label: Primitivos PQ
תיאור: Visao geral do crate `soranet_pq` e de como o לחיצת יד לעשות SoraNet consome helpers ML-KEM/ML-DSA.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas as copias sincronizadas.
:::

O ארגז `soranet_pq` מציין את קובצי ה-Pos-Quanticos Nos Quais Todo Relay, הלקוח ורכיבי הכלים של SoraNet. Eleveve as suites Kyber (ML-KEM) e Dilithium (ML-DSA) suportadas by PQClean e adiciona helpers de HKDF e RNG hedged amigaveis ao protocolo para que todas as superficies compartilhem executives identicas.

## O que vem em `soranet_pq`

- **ML-KEM-512/768/1024:** גרסאו דטרמיניסטיקה דה צ'אבס, עוזרי אנקפסולציה ו דה-קפסולציה com propagacao de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificacao separadas para transcricoes com separacao de dominio.
- **HKDF com rotulo:** `derive_labeled_hkdf` adiciona namespace a cada derivacao com o estagio do לחיצת יד (`DH/es`, `KEM/1`, ...) para que transcricoes hibridas fiquem sem colisa.
- **Aleatoriedade גידור:** `hedged_chacha20_rng` זרעי מיסטורה deterministicas com entropia do SO e zera o estado intermediario ao descartar.

Todos os segredos ficam dentro de contenedores `Zeroizing` e a CI exercita os bindings PQClean em todas as plataformas suportadas.

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

## Como Consumir

1. **Adicione a dependencia** a crates fora da raiz do space work:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **בחירת סוויטת קורטה** nos pontos de chamada. עבור ההתחלה של לחיצת יד, השתמש ב-`MlKemSuite::MlKem768` וב-`MlDsaSuite::MlDsa65`.

3. **מגזר chaves com rotulos.** השתמש ב-`HkdfDomain::soranet("KEM/1")` (e similares) para que o encadeamento de transcricoes fique deterministico entre nodes.

4. **השתמש ב-RNG hedged** ובין השאר ב-Sigredos de Fallback:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

O לחיצת יד המרכזית לעשות SoraNet e OS עוזרי עיוורון של CID (`iroha_crypto::soranet`) פעולות שימושיות, או que que crates downstream herdam as mesmas implementacoes Sem linkar bindings PQClean por conta propria.

## רשימת תאימות

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- ביקורת על דוגמאות לשימוש ללא README (`crates/soranet_pq/README.md`)
- להגדיר את המסמך לעיצוב לעשות לחיצת יד לעשות SoraNet quando os hybrids chegarem