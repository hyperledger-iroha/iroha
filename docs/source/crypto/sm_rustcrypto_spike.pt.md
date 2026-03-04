---
lang: pt
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2026-01-03T18:07:57.103009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notas para o pico de integração do RustCrypto SM.

# Notas de pico RustCrypto SM

## Objetivo
Valide que a introdução das caixas `sm2`, `sm3` e `sm4` do RustCrypto (mais `rfc6979`, `ccm`, `gcm`) como dependências opcionais são compiladas de forma limpa no `iroha_crypto` e produz tempos de construção aceitáveis antes de conectar o sinalizador de recurso ao espaço de trabalho mais amplo.

## Mapa de dependência proposto

| Caixa | Versão sugerida | Recursos | Notas |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/assinaturas) | `std` | Depende de `elliptic-curve`; verifique se o MSRV corresponde ao espaço de trabalho. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hashes) | padrão | A API é paralela ao `sha2` e integra-se às características `digest` existentes. |
| `sm4` | `0.5.1` (RustCrypto/cifras de bloco) | padrão | Funciona com características de cifra; Wrappers AEAD adiados para pico posterior. |
| `rfc6979` | `0.4` | padrão | Reutilização para derivação determinística de nonce. |

*As versões refletem os lançamentos atuais de 2024-12; confirme com `cargo search` antes de pousar.*

## Mudanças no Manifesto (rascunho)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Acompanhamento: fixe `elliptic-curve` para corresponder às versões já em `iroha_crypto` (atualmente `0.13.8`).

## Lista de verificação de picos
- [x] Adicione dependências e recursos opcionais ao `crates/iroha_crypto/Cargo.toml`.
- [x] Crie o módulo `signature::sm` atrás de `cfg(feature = "sm")` com estruturas de espaço reservado para confirmar a fiação.
- [x] Execute `cargo check -p iroha_crypto --features sm` para confirmar a compilação; registrar o tempo de construção e a nova contagem de dependências (`cargo tree --features sm`).
- [x] Confirme a postura somente padrão com `cargo check -p iroha_crypto --features sm --locked`; As compilações `no_std` não são mais suportadas.
- [x] Resultados do arquivo (tempos, delta da árvore de dependência) em `docs/source/crypto/sm_program.md`.

## Observações para capturar
- Tempo de compilação adicional versus linha de base.
- Impacto de tamanho binário (se mensurável) com `cargo builtinsize`.
- Qualquer MSRV ou conflito de recursos (por exemplo, com versões secundárias `elliptic-curve`).
- Avisos emitidos (código inseguro, controle const-fn) que podem exigir patches upstream.

## Itens Pendentes
- Aguarde a aprovação do Crypto WG antes de aumentar o gráfico de dependência do espaço de trabalho.
- Confirme se deseja vender caixas para revisão ou confiar em crates.io (espelhos podem ser necessários).
- Coordene a atualização `Cargo.lock` por `sm_lock_refresh_plan.md` antes de marcar a lista de verificação como concluída.
- Use `scripts/sm_lock_refresh.sh` assim que a aprovação for concedida para regenerar o arquivo de bloqueio e a árvore de dependências.

## 2025-01-19 Registro de picos
- Adicionadas dependências opcionais (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) e sinalizador de recurso `sm` em `iroha_crypto`.
- Módulo `signature::sm` stub para exercitar APIs de criptografia de hash/bloqueio durante a compilação.
- `cargo check -p iroha_crypto --features sm --locked` agora resolve o gráfico de dependência, mas aborta com o requisito de atualização `Cargo.lock`; a política do repositório proíbe edições de lockfile, portanto a execução da compilação permanece pendente até coordenarmos uma atualização de bloqueio permitida.## 2026-02-12 Registro de picos
- Resolvido o bloqueador de arquivo de bloqueio anterior - as dependências já foram capturadas - para que `cargo check -p iroha_crypto --features sm --locked` seja bem-sucedido (compilação a frio 7,9s no dev Mac; nova execução incremental 0,23s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` passa em 1.0s, confirmando que o recurso opcional é compilado em configurações somente `std` (nenhum caminho `no_std` permanece).
- Delta de dependência com o recurso `sm` habilitado apresenta 11 caixas: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` e `sm4-gcm`. (`rfc6979` já fazia parte do gráfico de linha de base.)
- Os avisos de compilação persistem para auxiliares de política NEON não utilizados; deixe como está até que o tempo de execução de suavização de medição reative esses caminhos de código.