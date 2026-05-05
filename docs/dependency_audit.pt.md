---
lang: pt
direction: ltr
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/dependency_audit.md (Dependency Audit Summary) -->

//! Resumo de Auditoria de Dependências

Data: 2025‑09‑01

Escopo: revisão em nível de workspace de todos os crates declarados em
Cargo.toml e resolvidos em Cargo.lock. Execução de `cargo-audit` contra o
RustSec advisory DB e revisão manual de legitimidade de crates e da escolha
dos “crates principais” para algoritmos.

Ferramentas/comandos executados:
- `cargo tree -d --workspace --locked --offline` – inspeção de versões
  duplicadas.
- `cargo audit` – varreu o Cargo.lock em busca de vulnerabilidades conhecidas
  e crates marcados como yanked.

Advisories de segurança encontrados (agora 0 vulns; 2 avisos):
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - Corrigido: atualização para `0.5.15` em `crates/ivm/Cargo.toml`.

- codec obsoleto em pprof — RUSTSEC‑2024‑0437  
  - Corrigido: `pprof` alterado para `prost-codec` em
    `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC‑2025‑0009  
  - Corrigido: atualização da stack QUIC/TLS (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) e atualização da stack WS para
    `tungstenite/tokio-tungstenite 0.24`. Lock de `ring` fixado em
    `0.17.12` via `cargo update -p ring --precise 0.17.12`.

Advisories restantes: nenhum. Avisos restantes: `backoff` (não mantido),
`derivative` (não mantido).

Avaliação de legitimidade e “crate principal” (destaques):
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak`
  (amplamente utilizado) — escolhas canônicas.
- AEAD/simétrico: `aes-gcm`, `chacha20poly1305`, traits `aead` (RustCrypto) —
  canônicos.
- Assinaturas/ECC: `ed25519-dalek`, `x25519-dalek` (projeto dalek), `k256`
  (RustCrypto), `secp256k1` (bindings libsecp) — todos legítimos; recomenda‑se
  preferir uma única stack secp256k1 (`k256` puro Rust ou `secp256k1` sobre
  libsecp) para reduzir a superfície.
- BLS12‑381/ZK: `blstrs`, família `halo2_*` — usados em produção em diversos
  ecossistemas ZK; legítimos.
- PQ: `pqcrypto-mldsa`, `pqcrypto-mlkem`, `pqcrypto-traits` — crates de referência
  legítimos.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — stack TLS moderna e
  canônica em Rust.
- Noise: `snow` — implementação canônica.
- Serialização: `parity-scale-codec` é o codec canônico para SCALE. Serde foi
  removido das dependências de produção em todo o workspace; derives/writers
  de Norito cobrem todos os caminhos de runtime. Qualquer referência residual
  a Serde está confinada a documentação histórica, scripts de proteção ou
  allowlists de testes.
- FFI/libs: `libsodium-sys-stable`, `openssl` — legítimos; em caminhos de
  produção, preferir Rustls a OpenSSL (como o código atual já faz).
- pprof 0.13.0 (crates.io) — fix upstream integrado; usar o release oficial
  com `prost-codec` + frame‑pointer para evitar o codec obsoleto.

Recomendações:
- Tratar os avisos:
  - Considerar substituir `backoff` por `retry`/`futures-retry` ou por um
    helper local de backoff exponencial.
  - Trocar derives de `derivative` por impls manuais ou `derive_more`, quando
    apropriado.
- Médio prazo: unificar o uso em torno de `k256` ou `secp256k1` onde for
  possível, a fim de reduzir implementações duplicadas (mantendo ambos apenas
  se realmente necessário).
- Médio prazo: revisar a proveniência de `poseidon-primitives 0.2.0` para uso
  em ZK; se possível, alinhar com uma implementação Poseidon nativa de
  Arkworks/Halo2 para evitar ecossistemas paralelos.

Notas:
- `cargo tree -d` mostra as versões maiores duplicadas esperadas
  (`bitflags` 1/2, múltiplos `ring`); isso não é um risco de segurança em si,
  mas aumenta a superfície de build.
- Não foram observados crates que aparentem typosquatting; todos os nomes e
  fontes remetem a crates bem conhecidos do ecossistema ou membros internos
  do workspace.
- Experimental: foi adicionada a feature `iroha_crypto`
  `bls-backend-blstrs` para iniciar a migração de BLS para um backend apenas
  `blstrs` (removendo a dependência de arkworks quando ativada). O padrão
  continua sendo `w3f-bls` para evitar mudanças de comportamento/codificação.
  Plano de alinhamento:
  - Normalizar a serialização da chave secreta para a saída canônica de
    32 bytes em little‑endian que tanto `w3f-bls` quanto `blstrs` entendem
    (`Scalar::to_bytes_le`), removendo o helper de endianness misto.
  - Expor um wrapper explícito para compressão de chave pública reutilizando
    `blstrs::G1Affine::to_compressed` e adicionando um check de
    consistencia com a codificação w3f, garantindo bytes idênticos “on wire”.
  - Adicionar fixtures de round‑trip em
    `crates/iroha_crypto/tests/bls_backend_compat.rs` que derivam chaves uma
    única vez e verificam igualdade entre ambos backends (`SecretKey`,
    `PublicKey` e agregação de assinaturas).
  - Proteger o novo backend com a feature `bls-backend-blstrs` em CI, mantendo
    ao mesmo tempo os testes de consistencia rodando para o backend
    padrão, para capturar regressões antes de qualquer migração.

Itens de follow‑up (trabalho proposto):
- Manter os guardrails de Serde em CI
  (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) para
  impedir que novos usos de Serde em código de produção sejam introduzidos.

Testes executados para esta auditoria:
- 
