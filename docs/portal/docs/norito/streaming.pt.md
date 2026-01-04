---
lang: pt
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

Norito Streaming define o formato on-wire, os frames de controle e o codec de referencia usados para fluxos de midia ao vivo entre Torii e SoraNet. A especificacao canonica fica em `norito_streaming.md` na raiz do workspace; esta pagina resume as partes que operadores e autores de SDK precisam junto aos pontos de configuracao.

## Formato on-wire e plano de controle

- **Manifests e frames.** `ManifestV1` e `PrivacyRoute*` descrevem a linha do tempo de segmentos, descritores de chunks e dicas de rota. Frames de controle (`KeyUpdate`, `ContentKeyUpdate` e feedback de cadencia) ficam junto ao manifest para que viewers validem commitments antes de decodificar.
- **Codec baseline.** `BaselineEncoder`/`BaselineDecoder` impoem ids de chunk monotonic, aritmetica de timestamps e verificacao de commitments. Hosts devem chamar `EncodedSegment::verify_manifest` antes de servir viewers ou relays.
- **Bits de feature.** A negociacao de capacidades anuncia `streaming.feature_bits` (padrao `0b11` = baseline feedback + privacy route provider) para que relays e clientes rejeitem peers incompativeis de forma deterministica.

## Chaves, suites e cadencia

- **Requisitos de identidade.** Frames de controle de streaming sao sempre assinados com Ed25519. Chaves dedicadas podem ser fornecidas via `streaming.identity_public_key`/`streaming.identity_private_key`; caso contrario a identidade do nodo e reutilizada.
- **Suites HPKE.** `KeyUpdate` seleciona a suite comum mais baixa; a suite #1 e obrigatoria (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), com caminho opcional de upgrade para `Kyber1024`. A selecao da suite fica armazenada na sessao e e validada em cada update.
- **Rotacao.** Publishers emitem um `KeyUpdate` assinado a cada 64 MiB ou 5 minutos. `key_counter` deve aumentar de forma estrita; regressao e erro critico. `ContentKeyUpdate` distribui a Group Content Key rotativa, envolta na suite HPKE negociada, e limita a descriptografia de segmentos por ID + janela de validade.
- **Snapshots.** `StreamingSession::snapshot_state` e `restore_from_snapshot` persistem `{session_id, key_counter, suite, sts_root, cadence state}` em `streaming.session_store_dir` (padrao `./storage/streaming`). As chaves de transporte sao re-derivadas na restauracao para que falhas nao vazem segredos da sessao.

## Configuracao de runtime

- **Material de chave.** Forneca chaves dedicadas com `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) e material Kyber opcional via `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Os quatro devem estar presentes ao sobrescrever os padroes; `streaming.kyber_suite` aceita `mlkem512|mlkem768|mlkem1024` (aliases `kyber512/768/1024`, padrao `mlkem768`).
- **Rotas SoraNet.** `streaming.soranet.*` controla transporte anonimo: `exit_multiaddr` (padrao `/dns/torii/udp/9443/quic`), `padding_budget_ms` (padrao 25 ms), `access_kind` (`authenticated` vs `read-only`), `channel_salt` opcional e `provision_spool_dir` (padrao `./storage/streaming/soranet_routes`).
- **Gate de sincronizacao.** `streaming.sync` alterna a aplicacao de drift para streams audiovisuais: `enabled`, `observe_only`, `ewma_threshold_ms` e `hard_cap_ms` governam quando segmentos sao rejeitados por drift de timing.

## Validacao e fixtures

- As definicoes canonicas de tipos e helpers ficam em `crates/iroha_crypto/src/streaming.rs`.
- A cobertura de integracao exercita o handshake HPKE, a distribuicao de content-key e o ciclo de vida de snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). Rode `cargo test -p iroha_crypto streaming_handshake` para verificar a superficie de streaming localmente.
- Para um mergulho profundo em layout, tratamento de erros e futuras upgrades, leia `norito_streaming.md` na raiz do repositorio.
