---
lang: pt
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Faixa de hospedagem de conteúdo
% Núcleo Iroha

# Faixa de hospedagem de conteúdo

A faixa de conteúdo armazena pequenos pacotes estáticos (arquivos tar) na cadeia e serve
arquivos individuais diretamente de Torii.

- **Publicar**: envie `PublishContentBundle` com um arquivo tar, expiração opcional
  altura e um manifesto opcional. O ID do pacote é o hash blake2b do
  tarball. As entradas tar devem ser arquivos regulares; os nomes são caminhos UTF-8 normalizados.
  Os limites de tamanho/caminho/contagem de arquivos vêm da configuração `content` (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Os manifestos incluem o hash do índice Norito, espaço de dados/pista, política de cache
  (`max_age_seconds`, `immutable`), modo de autenticação (`public` / `role:<role>` /
  `sponsor:<uaid>`), espaço reservado para política de retenção e substituições MIME.
- **Desduplicação**: as cargas tar são fragmentadas (padrão 64 KB) e armazenadas uma vez por
  hash com contagens de referência; aposentar um pacote diminui e remove pedaços.
- **Servir**: Torii expõe `GET /v1/content/{bundle}/{path}`. Fluxo de respostas
  diretamente do armazenamento de blocos com `ETag` = hash de arquivo, `Accept-Ranges: bytes`,
  Suporte de intervalo e Cache-Control derivado do manifesto. Lê honra o
  modo de autenticação de manifesto: respostas controladas por função e por patrocinador exigem canônico
  cabeçalhos de solicitação (`X-Iroha-Account`, `X-Iroha-Signature`) para o assinado
  conta; pacotes ausentes/expirados retornam 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (ou `--root <dir>`) agora
  gera automaticamente um manifesto, emite `--manifest-out/--bundle-out` opcional e
  aceita `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  e substituições `--expires-at-height`. Construções `iroha content pack --root <dir>`
  um tarball + manifesto determinístico sem enviar nada.
- **Config**: botões de cache/autenticação ficam em `content.*` em `iroha_config`
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) e são aplicados no momento da publicação.
- **SLO + limites**: `content.max_requests_per_second` / `request_burst` e
  Tampa `content.max_egress_bytes_per_second` / `egress_burst_bytes` lado de leitura
  rendimento; Torii impõe ambos antes de servir bytes e exportar
  `torii_content_requests_total`, `torii_content_request_duration_seconds` e
  Métricas `torii_content_response_bytes_total` com rótulos de resultados. Latência
  alvos vivem sob `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Controles de abuso**: os intervalos de taxas são codificados por token UAID/API/IP remoto e um
  protetor PoW opcional (`content.pow_difficulty_bits`, `content.pow_header`) pode
  ser obrigatório antes das leituras. Os padrões de layout de faixa DA vêm de
  `content.stripe_layout` e são ecoados em hashes de recibos/manifestos.
- **Recibos e evidências de DA**: respostas bem-sucedidas anexadas
  `sora-content-receipt` (bytes `ContentDaReceipt` com estrutura base64 Norito) carregando
  `bundle_id`, `path`, `file_hash`, `served_bytes`, o intervalo de bytes servidos,
  `chunk_root` / `stripe_layout`, compromisso PDP opcional e um carimbo de data/hora para
  os clientes podem fixar o que foi buscado sem reler o corpo.

Principais referências:- Modelo de dados: `crates/iroha_data_model/src/content.rs`
- Execução: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Manipulador Torii: `crates/iroha_torii/src/content.rs`
- Auxiliar CLI: `crates/iroha_cli/src/content.rs`