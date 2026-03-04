---
lang: pt
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Instantâneo de proveniência de Tongsuo
% Gerado: 30/01/2026

# Resumo do ambiente

-`pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: relata `LibreSSL 3.3.6` (kit de ferramentas TLS fornecido pelo sistema no macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: consulte `sm_iroha_crypto_tree.txt` para obter a pilha de dependência Rust exata (caixa `openssl` v0.10.74, `openssl-sys` v0.9.x, fontes OpenSSL 3.x fornecidas disponíveis via caixa `openssl-src`; recurso `vendored` habilitado em `crates/iroha_crypto/Cargo.toml` para compilações de visualização determinísticas).

# Notas

- Links do ambiente de desenvolvimento local contra cabeçalhos/bibliotecas LibreSSL; as compilações de visualização de produção devem usar OpenSSL >= 3.0.0 ou Tongsuo 8.x. Substitua o kit de ferramentas do sistema ou configure `OPENSSL_DIR`/`PKG_CONFIG_PATH` ao gerar o pacote de artefatos final.
- Gere novamente esse instantâneo dentro do ambiente de compilação de lançamento para capturar o hash exato do tarball OpenSSL/Tongsuo (`openssl version -v`, `openssl version -b`, `openssl version -f`) e anexe o script/soma de verificação de compilação reproduzível. Para compilações de fornecedores, registre a versão/commit da caixa `openssl-src` usada pelo Cargo (visível em `target/debug/build/openssl-sys-*/output`).
- Os hosts Apple Silicon exigem `RUSTFLAGS=-Aunsafe-code` ao executar o chicote de fumaça OpenSSL para que os stubs de aceleração AArch64 SM3/SM4 sejam compilados (os intrínsecos não estão disponíveis no macOS). O script `scripts/sm_openssl_smoke.sh` exporta esse sinalizador antes de invocar `cargo` para manter a consistência das execuções CI e locais.
- Anexe a origem da fonte upstream (por exemplo, `openssl-src-<ver>.tar.gz` SHA256) assim que o pipeline de embalagem estiver fixado; use o mesmo hash em artefatos de CI.