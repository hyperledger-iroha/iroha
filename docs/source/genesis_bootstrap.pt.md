---
lang: pt
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap de pares confiáveis

Pares Iroha sem um `genesis.file` local podem buscar um bloco de gênese assinado de pares confiáveis
usando o protocolo de inicialização codificado Norito.

- **Protocolo:** peers trocam `GenesisRequest` (`Preflight` para metadados, `Fetch` para carga útil) e
  Quadros `GenesisResponse` codificados por `request_id`. Os respondentes incluem o ID da cadeia, signatário pubkey,
  hash e uma dica de tamanho opcional; cargas úteis são retornadas apenas em `Fetch` e IDs de solicitação duplicados
  receber `DuplicateRequest`.
- **Guardas:** os respondentes impõem uma lista de permissões (`genesis.bootstrap_allowlist` ou os pares confiáveis
  conjunto), correspondência de ID de cadeia/pubkey/hash, limites de taxa (`genesis.bootstrap_response_throttle`) e um
  tampa de tamanho (`genesis.bootstrap_max_bytes`). Solicitações fora da lista de permissões recebem `NotAllowed` e
  cargas assinadas pela chave errada recebem `MismatchedPubkey`.
- **Fluxo do solicitante:** quando o armazenamento está vazio e `genesis.file` não está definido (e
  `genesis.bootstrap_enabled=true`), o nó simula peers confiáveis com o opcional
  `genesis.expected_hash`, em seguida, busca a carga útil, valida assinaturas via `validate_genesis_block`,
  e persiste `genesis.bootstrap.nrt` ao lado de Kura antes de aplicar o bloqueio. Novas tentativas de bootstrap
  honra `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` e
  `genesis.bootstrap_max_attempts`.
- **Modos de falha:** solicitações são rejeitadas por falhas na lista de permissões, incompatibilidades de cadeia/pubkey/hash, tamanho
  violações de limite, limites de taxa, gênese local ausente ou IDs de solicitação duplicados. Hashes conflitantes
  entre pares aborta a busca; nenhum respondedor/tempo limite retorna à configuração local.
- **Etapas do operador:** garantir que pelo menos um peer confiável seja acessível com uma gênese válida, configure
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` e os botões de nova tentativa, e
  opcionalmente, fixe `expected_hash` para evitar aceitar cargas incompatíveis. Cargas persistentes podem ser
  reutilizado em inicializações subsequentes apontando `genesis.file` para `genesis.bootstrap.nrt`.