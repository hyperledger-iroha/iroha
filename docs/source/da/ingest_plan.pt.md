---
lang: pt
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plano de ingestão de disponibilidade de dados Sora Nexus

_Elaborado: 20/02/2026 - Proprietário: Core Protocol WG / Equipe de armazenamento / DA WG_

O fluxo de trabalho DA-2 estende Torii com uma API de ingestão de blob que emite Norito
metadados e sementes replicação SoraFS. Este documento captura a proposta
esquema, superfície da API e fluxo de validação para que a implementação possa prosseguir sem
bloqueio em simulações pendentes (acompanhamentos DA-1). Todos os formatos de carga útil DEVEM
use codecs Norito; nenhum substituto serde/JSON é permitido.

## Metas

- Aceitar grandes blobs (segmentos Taikai, sidecars de pista, artefatos de governança)
  determinísticamente sobre Torii.
- Produzir manifestos canônicos Norito descrevendo o blob, parâmetros de codec,
  perfil de eliminação e política de retenção.
- Persistir metadados de pedaços no armazenamento quente SoraFS e enfileirar trabalhos de replicação.
- Publicar intenções de pin + tags de política no registro e governança SoraFS
  observadores.
- Expor recibos de admissão para que os clientes recuperem provas determinísticas de publicação.

## Superfície de API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

A carga útil é um `DaIngestRequest` codificado em Norito. Uso de respostas
`application/norito+v1` e retorne `DaIngestReceipt`.

| Resposta | Significado |
| --- | --- |
| 202 Aceito | Blob na fila para agrupamento/replicação; recibo devolvido. |
| 400 Solicitação incorreta | Violação de esquema/tamanho (consulte verificações de validação). |
| 401 Não autorizado | Token de API ausente/inválido. |
| 409 Conflito | Duplicar `client_blob_id` com metadados incompatíveis. |
| 413 Carga útil muito grande | Excede o limite de comprimento do blob configurado. |
| 429 Muitas solicitações | Limite de taxa atingido. |
| 500 Erro interno | Falha inesperada (registrada + alerta). |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Retorna um `DaProofPolicyBundle` versionado derivado do catálogo de pistas atual.
O pacote anuncia `version` (atualmente `1`), um `policy_hash` (hash do
lista de políticas ordenada) e entradas `policies` contendo `lane_id`, `dataspace_id`,
`alias`, e o `proof_scheme` aplicado (`merkle_sha256` hoje; pistas KZG são
rejeitado por ingestão até que os compromissos KZG estejam disponíveis). O cabeçalho do bloco agora
confirma o pacote via `da_proof_policies_hash`, para que os clientes possam fixar o
política ativa definida ao verificar compromissos ou provas do DA. Buscar este endpoint
antes de construir provas para garantir que correspondam à política da pista e ao atual
pacote de hash. Os endpoints de lista de compromissos/provas carregam o mesmo pacote para que os SDKs
não precisa de uma viagem de ida e volta extra para vincular uma prova ao conjunto de políticas ativas.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Retorna um `DaProofPolicyBundle` contendo a lista de políticas ordenadas mais um
`policy_hash` para que os SDKs possam fixar a versão usada quando um bloco foi produzido. O
hash é calculado sobre a matriz de política codificada em Norito e muda sempre que um
O `proof_scheme` da pista é atualizado, permitindo que os clientes detectem desvios entre
provas armazenadas em cache e a configuração da cadeia.

## Esquema Norito proposto

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```> Nota de implementação: as representações canônicas do Rust para essas cargas agora estão em
> `iroha_data_model::da::types`, com wrappers de solicitação/recebimento em `iroha_data_model::da::ingest`
> e a estrutura do manifesto em `iroha_data_model::da::manifest`.

O campo `compression` anuncia como os chamadores prepararam a carga útil. Torii aceita
`identity`, `gzip`, `deflate` e `zstd`, descompactando transparentemente os bytes antes
hash, fragmentação e verificação de manifestos opcionais.

### Lista de verificação de validação

1. Verifique se o cabeçalho da solicitação Norito corresponde a `DaIngestRequest`.
2. Falha se `total_size` diferir do comprimento da carga útil canônica (descompactada) ou exceder o comprimento máximo configurado.
3. Aplique o alinhamento `chunk_size` (potência de dois, = 2.
5. `retention_policy.required_replica_count` deve respeitar a linha de base de governança.
6. Verificação de assinatura contra hash canônico (excluindo campo de assinatura).
7. Rejeite `client_blob_id` duplicado, a menos que hash de carga útil + metadados sejam idênticos.
8. Quando `norito_manifest` for fornecido, verifique as correspondências de esquema + hash recalculadas
   manifestar-se após a fragmentação; caso contrário, o nó gera o manifesto e o armazena.
9. Aplique a política de replicação configurada: Torii reescreve o enviado
   `RetentionPolicy` com `torii.da_ingest.replication_policy` (ver
   `replication_policy.md`) e rejeita manifestos pré-construídos cuja retenção
   os metadados não correspondem ao perfil imposto.

### Fluxo de fragmentação e replicação1. Bloco de carga útil em `chunk_size`, calcule BLAKE3 por bloco + raiz Merkle.
2. Construa Norito `DaManifestV1` (nova estrutura) capturando compromissos de bloco (role/group_id),
   layout de eliminação (contagens de paridade de linhas e colunas mais `ipa_commitment`), política de retenção,
   e metadados.
3. Enfileire os bytes do manifesto canônico em `config.da_ingest.manifest_store_dir`
   (Torii grava arquivos `manifest.encoded` codificados por faixa/época/sequência/ticket/impressão digital) então SoraFS
   a orquestração pode ingeri-los e vincular o tíquete de armazenamento aos dados persistentes.
4. Publique intenções de pin via `sorafs_car::PinIntent` com tag de governança + política.
5. Emita o evento Norito `DaIngestPublished` para notificar os observadores (clientes leves,
   governança, análise).
6. Retorne `DaIngestReceipt` (assinado pela chave de serviço DA Torii) e adicione o
   Cabeçalho de resposta `Sora-PDP-Commitment` contendo a codificação base64 Norito
   do compromisso derivado para que os SDKs possam armazenar a semente de amostragem imediatamente.
   O recibo agora incorpora `rent_quote` (um `DaRentQuote`) e `stripe_layout`
   para que os remetentes possam revelar as obrigações XOR, participação de reserva, expectativas de bônus PDP/PoTR,
   e as dimensões da matriz de eliminação 2D juntamente com os metadados dos bilhetes de armazenamento antes de comprometer os fundos.
7. Metadados de registro opcionais:
   - `da.registry.alias` — string de alias UTF-8 pública e não criptografada para propagar a entrada de registro do PIN.
   - `da.registry.owner` — string `AccountId` pública e não criptografada para registrar a propriedade do registro.
   Torii copia-os no `DaPinIntent` gerado para que o processamento de pinos downstream possa vincular aliases
   e proprietários sem analisar novamente o mapa de metadados brutos; valores malformados ou vazios são rejeitados durante
   validação de ingestão.

## Atualizações de armazenamento/registro

- Estenda `sorafs_manifest` com `DaManifestV1`, permitindo análise determinística.
- Adicionar novo fluxo de registro `da.pin_intent` com referência de carga útil versionada
  hash de manifesto + id do ticket.
- Atualizar pipelines de observabilidade para rastrear a latência de ingestão, fragmentação da taxa de transferência,
  backlog de replicação e contagens de falhas.
- As respostas Torii `/status` agora incluem uma matriz `taikai_ingest` que apresenta as mais recentes
  latência do codificador para ingestão, desvio de borda ao vivo e contadores de erros por (cluster, stream), habilitando DA-9
  painéis para ingerir instantâneos de integridade diretamente dos nós sem raspar Prometheus.

## Estratégia de teste- Testes unitários para validação de esquemas, verificações de assinaturas, detecção de duplicatas.
- Testes Golden verificando a codificação Norito de `DaIngestRequest`, manifesto e recebimento.
- Chicote de integração girando o registro SoraFS + simulado, afirmando fluxos de pedaços + pinos.
- Testes de propriedades cobrindo perfis de apagamento aleatórios e combinações de retenção.
- Fuzzing de cargas úteis Norito para proteção contra metadados malformados.
- Luminárias douradas para cada classe de blob ao vivo
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` com um pedaço complementar
  listagem em `fixtures/da/ingest/sample_chunk_records.txt`. O teste ignorado
  `regenerate_da_ingest_fixtures` atualiza os fixtures, enquanto
  `manifest_fixtures_cover_all_blob_classes` falha assim que uma nova variante `BlobClass` é adicionada
  sem atualizar o pacote Norito/JSON. Isso mantém Torii, SDKs e documentos honestos sempre que DA-2
  aceita uma nova superfície de blob.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## Ferramentas CLI e SDK (DA-8)- `iroha app da submit` (novo ponto de entrada CLI) agora agrupa o construtor/editor de ingestão compartilhada para que os operadores
  pode ingerir blobs arbitrários fora do fluxo do pacote Taikai. O comando mora em
  `crates/iroha_cli/src/commands/da.rs:1` e consome uma carga útil, perfil de eliminação/retenção e
  arquivos opcionais de metadados/manifesto antes de assinar o `DaIngestRequest` canônico com a CLI
  chave de configuração. As execuções bem-sucedidas persistem `da_request.{norito,json}` e `da_receipt.{norito,json}` em
  `artifacts/da/submission_<timestamp>/` (substituir via `--artifact-dir`) para que os artefatos de liberação possam
  registre os bytes Norito exatos usados durante a ingestão.
- O comando é padronizado como `client_blob_id = blake3(payload)`, mas aceita substituições via
  `--client-blob-id`, respeita mapas JSON de metadados (`--metadata-json`) e manifestos pré-gerados
  (`--manifest`) e suporta `--no-submit` para preparação off-line e `--endpoint` para preparação personalizada
  Anfitriões Torii. O JSON do recibo é impresso no stdout além de ser gravado no disco, fechando o
  Requisito de ferramentas DA-8 “submit_blob” e trabalho de paridade de desbloqueio do SDK.
- `iroha app da get` adiciona um alias focado em DA para o orquestrador multi-fonte que já alimenta
  `iroha app sorafs fetch`. Os operadores podem apontar para artefatos de manifesto + plano de bloco (`--manifest`,
  `--plan`, `--manifest-id`) **ou** simplesmente passe um tíquete de armazenamento Torii via `--storage-ticket`. Quando o
  o caminho do ticket é usado, a CLI extrai o manifesto de `/v1/da/manifests/<ticket>`, persiste o pacote
  sob `artifacts/da/fetch_<timestamp>/` (substituir por `--manifest-cache-dir`), deriva o **manifesto
  hash** para `--manifest-id` e, em seguida, executa o orquestrador com o `--gateway-provider` fornecido
  lista. A verificação da carga útil ainda depende do resumo CAR/`blob_hash` incorporado enquanto o ID do gateway é
  agora o hash do manifesto para que clientes e validadores compartilhem um único identificador de blob. Todos os botões avançados de
  a superfície do buscador SoraFS intacta (envelopes de manifesto, etiquetas de cliente, caches de proteção, transporte de anonimato
  substituições, exportação de placar e caminhos `--output`), e o endpoint do manifesto pode ser substituído por meio de
  `--manifest-endpoint` para hosts Torii personalizados, portanto, as verificações de disponibilidade ponta a ponta ficam inteiramente sob o
  Namespace `da` sem duplicar a lógica do orquestrador.
- `iroha app da get-blob` extrai manifestos canônicos diretamente de Torii via `GET /v1/da/manifests/{storage_ticket}`.
  O comando agora rotula os artefatos com o hash do manifesto (blob id), escrevendo
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` e `chunk_plan_{manifest_hash}.json`
  sob `artifacts/da/fetch_<timestamp>/` (ou um `--output-dir` fornecido pelo usuário) enquanto ecoa o exato
  Invocação `iroha app da get` (incluindo `--manifest-id`) necessária para a busca do orquestrador de acompanhamento.
  Isso mantém os operadores fora dos diretórios de spool de manifesto e garante que o buscador sempre use o
  artefatos assinados emitidos por Torii. O cliente JavaScript Torii espelha esse fluxo por meio de
  `ToriiClient.getDaManifest(storageTicketHex)` enquanto o Swift SDK agora expõe
  `ToriiClient.getDaManifestBundle(...)`. Ambos retornam os bytes Norito decodificados, manifesto JSON, manifesto hash,e plano de bloco para que os chamadores do SDK possam hidratar as sessões do orquestrador sem gastar dinheiro com a CLI e o Swift
  os clientes também podem chamar `fetchDaPayloadViaGateway(...)` para canalizar esses pacotes através do nativo
  Wrapper do orquestrador SoraFS.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- As respostas `/v1/da/manifests` agora aparecem `manifest_hash` e ambos os auxiliares CLI + SDK (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` e os wrappers de gateway Swift/JS) tratam esse resumo como o
  identificador de manifesto canônico enquanto continua a verificar cargas em relação ao hash CAR/blob incorporado.
- `iroha app da rent-quote` calcula aluguel determinístico e detalhamentos de incentivo para um tamanho de armazenamento fornecido
  e janela de retenção. O auxiliar consome o `DaRentPolicyV1` ativo (bytes JSON ou Norito) ou
  o padrão integrado, valida a política e imprime um resumo JSON (`gib`, `months`, metadados da política,
  e os campos `DaRentQuote`) para que os auditores possam citar cobranças XOR exatas dentro de minutos de governança sem
  escrever scripts ad hoc. O comando agora também emite um resumo `rent_quote ...` de uma linha antes do JSON
  carga útil para facilitar a verificação de logs e runbooks do console quando cotações são geradas durante incidentes.
  Passe `--quote-out artifacts/da/rent_quotes/<stamp>.json` (ou qualquer outro caminho)
  persistir o resumo bem impresso e usar `--policy-label "governance ticket #..."` quando o
  o artefato precisa citar um pacote específico de votação/configuração; a CLI corta rótulos personalizados e rejeita espaços em branco
  strings para manter os valores `policy_source` significativos em pacotes de evidências. Veja
  `crates/iroha_cli/src/commands/da.rs` para o subcomando e `docs/source/da/rent_policy.md`
  para o esquema de política.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- A paridade do registro de pinos agora se estende aos SDKs: `ToriiClient.registerSorafsPinManifest(...)` no
  O JavaScript SDK cria a carga exata usada por `iroha app sorafs pin register`, impondo
  metadados do chunker, políticas de pin, provas de alias e resumos de sucessores antes do POST para
  `/v1/sorafs/pin/register`. Isso evita que os bots de CI e a automação paguem pela CLI quando
  gravando registros de manifesto, e o auxiliar vem com cobertura TypeScript/README para que DA-8's
  A paridade de ferramentas “enviar/obter/provar” é totalmente satisfeita em JS junto com Rust/Swift.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` encadeia todos os itens acima: pega um ticket de armazenamento, baixa o
  pacote de manifesto canônico, executa o orquestrador de várias fontes (`iroha app sorafs fetch`) no
  lista `--gateway-provider` fornecida, persiste a carga útil baixada + placar em
  `artifacts/da/prove_availability_<timestamp>/` e invoca imediatamente o auxiliar PoR existente
  (`iroha app da prove`) usando os bytes buscados. Os operadores podem ajustar os botões do orquestrador
  (`--max-peers`, `--scoreboard-out`, substituições de endpoint de manifesto) e o amostrador de prova
  (`--sample-count`, `--leaf-index`, `--sample-seed`) enquanto um único comando produz os artefatos
  esperado pelas auditorias DA-5/DA-9: cópia da carga útil, evidências de placar e resumos de provas JSON.- `da_reconstruct` (novo no DA-6) lê um manifesto canônico mais o diretório de pedaços emitido pelo pedaço
  store (layout `chunk_{index:05}.bin`) e remonta deterministicamente a carga útil enquanto verifica
  cada compromisso Blake3. A CLI reside sob `crates/sorafs_car/src/bin/da_reconstruct.rs` e é enviada como
  parte do pacote de ferramentas SoraFS. Fluxo típico:
  1. `iroha app da get-blob --storage-ticket <ticket>` para baixar `manifest_<manifest_hash>.norito` e o plano de blocos.
  2.`iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (ou `iroha app da prove-availability`, que grava os artefatos de busca em
     `artifacts/da/prove_availability_<ts>/` e persiste arquivos por bloco dentro do diretório `chunks/`).
  3.`cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Um acessório de regressão reside em `fixtures/da/reconstruct/rs_parity_v1/` e captura o manifesto completo
  e matriz de blocos (dados + paridade) usada por `tests::reconstructs_fixture_with_parity_chunks`. Regenere-o com

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  O aparelho emite:

  - `manifest.{norito.hex,json}` — codificações canônicas `DaManifestV1`.
  - `chunk_matrix.json` — linhas ordenadas de índice/deslocamento/comprimento/resumo/paridade para referências de documentos/testes.
  - `chunks/` — Fatias de carga útil `chunk_{index:05}.bin` para dados e fragmentos de paridade.
  - `payload.bin` — carga útil determinística usada pelo teste de chicote com reconhecimento de paridade.
  - `commitment_bundle.{json,norito.hex}` — amostra `DaCommitmentBundle` com um compromisso KZG determinístico para documentos/testes.

  O chicote recusa pedaços ausentes ou truncados, verifica o hash final da carga útil Blake3 em relação a `blob_hash`,
  e emite um blob JSON resumido (bytes de carga útil, contagem de blocos, tíquete de armazenamento) para que o CI possa afirmar a reconstrução
  evidência. Isso encerra o requisito DA-6 para uma ferramenta de reconstrução determinística que os operadores e o controle de qualidade
  jobs podem ser invocados sem conectar scripts personalizados.

## Resumo da resolução TODO

Todos os TODOs de ingestão bloqueados anteriormente foram implementados e verificados:- **Dicas de compactação** — Torii aceita rótulos fornecidos pelo chamador (`identity`, `gzip`, `deflate`,
  `zstd`) e normaliza as cargas úteis antes da validação para que o hash do manifesto canônico corresponda ao
  bytes descompactados.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Criptografia de metadados somente de governança** — Torii agora criptografa metadados de governança com o
  chave ChaCha20-Poly1305 configurada, rejeita rótulos incompatíveis e apresenta dois explícitos
  botões de configuração (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) para manter a rotação determinística.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Streaming de grande carga útil** — a ingestão de várias partes é ao vivo. Fluxo de clientes determinístico
  Envelopes `DaIngestChunk` codificados por `client_blob_id`, Torii valida cada fatia, organiza-as
  sob `manifest_store_dir` e reconstrói atomicamente o manifesto assim que a bandeira `is_last` pousar,
  eliminando os picos de RAM observados em uploads de chamada única.【crates/iroha_torii/src/da/ingest.rs:392】
- **Controle de versão do manifesto** — `DaManifestV1` carrega um campo `version` explícito e Torii recusa
  versões desconhecidas, garantindo atualizações determinísticas quando novos layouts de manifesto forem lançados.【crates/iroha_data_model/src/da/types.rs:308】
- **Ganchos PDP/PoTR** — Os compromissos PDP derivam diretamente do armazenamento de blocos e são persistidos
  além de manifestos para que os agendadores DA-5 possam lançar desafios de amostragem a partir de dados canônicos; o
  O cabeçalho `Sora-PDP-Commitment` agora é fornecido com `/v1/da/ingest` e `/v1/da/manifests/{ticket}`
  respostas para que os SDKs aprendam imediatamente o compromisso assinado que as futuras investigações farão referência.
- **Diário do cursor de fragmento** — os metadados da pista podem especificar `da_shard_id` (padrão para `lane_id`) e
  Sumeragi agora persiste o `(epoch, sequence)` mais alto por `(shard_id, lane_id)` em
  `da-shard-cursors.norito` ao lado do spool DA, então as reinicializações descartam pistas refragmentadas/desconhecidas e mantêm
  repetição determinística. O índice do cursor de fragmento na memória agora falha rapidamente em compromissos para
  pistas não mapeadas em vez de usar como padrão o ID da pista, fazendo com que o avanço do cursor e erros de repetição
  explícito e a validação de bloco rejeita regressões de cursor de fragmento com um dedicado
  `DaShardCursorViolation` razão + etiquetas de telemetria para operadores. Inicialização/atualização agora interrompe o DA
  indexe a hidratação se Kura contiver uma pista desconhecida ou cursor regressivo e registre a infração
  altura do bloco para que os operadores possam remediar antes de servir o DA estado.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Telemetria de atraso do cursor de fragmento** — o medidor `da_shard_cursor_lag_blocks{lane,shard}` relata comoaté onde um fragmento segue a altura que está sendo validada. Pistas ausentes/obsoletas/desconhecidas definem o atraso para
  altura necessária (ou delta) e avanços bem-sucedidos a redefinem para zero para que o estado estacionário permaneça estável.
  Os operadores devem alarmar-se com atrasos diferentes de zero, inspecionar o carretel/diário DA para a pista infratora,
  e verifique o catálogo de pistas para refragmentação acidental antes de reproduzir o bloco para limpar o
  lacuna.
- **Faixas de computação confidenciais** — pistas marcadas com
  `metadata.confidential_compute=true` e um `confidential_key_version` são tratados como
  Caminhos SMPC/DA criptografados: Sumeragi impõe resumos de carga/manifesto diferentes de zero e tíquetes de armazenamento,
  rejeita perfis de armazenamento de réplica completa e indexa o ticket SoraFS + versão da política sem
  expondo bytes de carga útil. Os recibos são hidratados de Kura durante o replay para que os validadores os recuperem
  metadados de confidencialidade após reinicializações.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## Notas de implementação- O endpoint `/v1/da/ingest` do Torii agora normaliza a compactação de carga útil, impõe o cache de repetição,
  divide deterministicamente os bytes canônicos, reconstrói `DaManifestV1` e descarta a carga útil codificada
  em `config.da_ingest.manifest_store_dir` para orquestração SoraFS antes de emitir o recibo; o
  manipulador também anexa um cabeçalho `Sora-PDP-Commitment` para que os clientes possam capturar o compromisso codificado
  imediatamente.【crates/iroha_torii/src/da/ingest.rs:220】
- Depois de persistir o `DaCommitmentRecord` canônico, Torii agora emite um
  Arquivo `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ao lado do spool de manifesto.
  Cada entrada agrupa o registro com os bytes brutos Norito `PdpCommitment` para que os construtores de pacotes DA-3 e
  Os agendadores DA-5 ingerem entradas idênticas sem reler manifestos ou armazenamentos de blocos.【crates/iroha_torii/src/da/ingest.rs:1814】
- Os auxiliares do SDK expõem os bytes do cabeçalho PDP sem forçar cada cliente a reimplementar a análise Norito:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` cobre Rust, o Python `ToriiClient`
  agora exporta `decode_pdp_commitment_header` e `IrohaSwift` envia ajudantes correspondentes tão móveis
  os clientes podem armazenar a programação de amostragem codificada imediatamente.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii também expõe `GET /v1/da/manifests/{storage_ticket}` para que SDKs e operadores possam buscar manifestos
  e pedaços de planos sem tocar no diretório de spool do nó. A resposta retorna os bytes Norito
  (base64), manifesto JSON renderizado, um blob JSON `chunk_plan` pronto para `sorafs fetch`, além do relevante
  resumos hexadecimais (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) para que as ferramentas posteriores possam
  alimenta o orquestrador sem recomputar resumos e emite o mesmo cabeçalho `Sora-PDP-Commitment` para
  espelhar respostas de ingestão. Passar `block_hash=<hex>` como parâmetro de consulta retorna um valor determinístico
  `sampling_plan` enraizado em `block_hash || client_blob_id` (compartilhado entre validadores) contendo o
  `assignment_hash`, o `sample_window` solicitado e as tuplas `(index, role, group)` amostradas abrangendo
  todo o layout de faixa 2D para que amostradores e validadores PoR possam reproduzir os mesmos índices. O amostrador
  mistura `client_blob_id`, `chunk_root` e `ipa_commitment` no hash de atribuição; `iroha app da get
  --block-hash ` now writes `sampling_plan_.json` próximo ao manifesto + plano de bloco com
  o hash preservado e os clientes JS/Swift Torii expõem o mesmo `assignment_hash_hex` para que os validadores
  e os provadores compartilham um único conjunto de sondagens determinísticas. Quando Torii retorna um plano de amostragem, `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) em vez disso
  de amostragem ad-hoc para que as testemunhas PoR se alinhem com as atribuições do validador, mesmo que o operador omita uma
  `--block-hash` substituir.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Grande fluxo de streaming de carga útilOs clientes que precisam ingerir ativos maiores que o limite de solicitação única configurado iniciam um
sessão de streaming ligando para `POST /v1/da/ingest/chunk/start`. Torii responde com um
`ChunkSessionId` (BLAKE3 derivado dos metadados de blob solicitados) e o tamanho do bloco negociado.
Cada solicitação `DaIngestChunk` subsequente carrega:

- `client_blob_id` — idêntico ao `DaIngestRequest` final.
- `chunk_session_id` — vincula fatias à sessão em execução.
- `chunk_index` e `offset` — impõem ordenação determinística.
- `payload` — até o tamanho do pedaço negociado.
- `payload_hash` — hash BLAKE3 da fatia para que Torii possa validar sem armazenar em buffer o blob inteiro.
- `is_last` — indica a fatia terminal.

Torii persiste fatias validadas em `config.da_ingest.manifest_store_dir/chunks/<session>/` e
registra o progresso dentro do cache de repetição para honrar a idempotência. Quando a fatia final chegar, Torii
remonta a carga útil no disco (streaming através do diretório de partes para evitar picos de memória),
calcula o manifesto/recibo canônico exatamente como acontece com uploads únicos e, finalmente, responde a
`POST /v1/da/ingest` consumindo o artefato preparado. Sessões com falha podem ser abortadas explicitamente ou
são coletados como lixo após `config.da_ingest.replay_cache_ttl`. Este design mantém o formato da rede
Compatível com Norito, evita protocolos recuperáveis específicos do cliente e reutiliza o pipeline de manifesto existente
inalterado.

**Status de implementação.** Os tipos canônicos Norito agora residem em
`crates/iroha_data_model/src/da/`:

- `ingest.rs` define `DaIngestRequest`/`DaIngestReceipt`, junto com o
  Recipiente `ExtraMetadata` usado por Torii.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` hospeda `DaManifestV1` e `ChunkCommitment`, que Torii emite após
  fragmentação concluída.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` fornece aliases compartilhados (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, etc.) e codifica os valores de política padrão documentados abaixo.【crates/iroha_data_model/src/da/types.rs:240】
- Os arquivos de spool de manifesto chegam em `config.da_ingest.manifest_store_dir`, prontos para a orquestração SoraFS
  observador para entrar na admissão de armazenamento.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi impõe a disponibilidade do manifesto ao selar ou validar pacotes DA:
  os blocos falham na validação se o spool não tiver o manifesto ou o hash for diferente
  do compromisso.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

A cobertura de ida e volta para as cargas úteis de solicitação, manifesto e recebimento é rastreada em
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, garantindo o codec Norito
permanece estável durante as atualizações.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Padrões de retenção.** A governança ratificou a política de retenção inicial durante
SF-6; os padrões aplicados por `RetentionPolicy::default()` são:- camada quente: 7 dias (segundos `604_800`)
- camada fria: 90 dias (segundos `7_776_000`)
- réplicas necessárias: `3`
classe de armazenamento: `StorageClass::Hot`
- etiqueta de governança: `"da.default"`

Os operadores downstream devem substituir esses valores explicitamente quando uma pista adota
requisitos mais rigorosos.

## Artefatos à prova de ferrugem do cliente

SDKs que incorporam o cliente Rust não precisam mais pagar pela CLI para
produza o pacote JSON PoR canônico. O `Client` expõe dois auxiliares:

- `build_da_proof_artifact` retorna a estrutura exata gerada por
  `iroha app da prove --json-out`, incluindo as anotações de manifesto/carga útil fornecidas
  via [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` envolve o construtor e persiste o artefato no disco
  (bastante JSON + nova linha final por padrão) para que a automação possa anexar o arquivo
  para lançamentos ou pacotes de evidências de governança.【crates/iroha/src/client.rs:3653】

### Exemplo

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

A carga JSON que sai do auxiliar corresponde à CLI até os nomes dos campos
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, etc.), tão existentes
a automação pode diferenciar/parquet/carregar o arquivo sem ramificações específicas do formato.

## Referência de verificação de prova

Use o equipamento de benchmark à prova de DA para validar orçamentos de verificador em cargas representativas antes
apertando as tampas de nível de bloco:

- `cargo xtask da-proof-bench` reconstrói o armazenamento de blocos a partir do par manifesto/carga útil, amostras de PoR
  folhas e verificação de tempos em relação ao orçamento configurado. Os metadados do Taikai são preenchidos automaticamente e o
  O chicote volta para um manifesto sintético se o par de fixtures for inconsistente. Quando `--payload-bytes`
  é definido sem um `--payload` explícito, o blob gerado é gravado em
  `artifacts/da/proof_bench/payload.bin` para que os fixtures permaneçam intactos.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Os relatórios são padronizados como `artifacts/da/proof_bench/benchmark.{json,md}` e incluem provas/execução, total e
  tempos por prova, taxa de aprovação do orçamento e um orçamento recomendado (110% da iteração mais lenta) para
  alinhe-se com `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- Execução mais recente (carga útil sintética de 1 MiB, blocos de 64 KiB, 32 provas/execução, 10 iterações, orçamento de 250 ms)
  recomendou um orçamento de verificador de 3 ms com 100% de iterações dentro do limite.【artifacts/da/proof_bench/benchmark.md:1】
- Exemplo (gera uma carga determinística e escreve ambos os relatórios):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

A montagem do bloco impõe os mesmos orçamentos: `sumeragi.da_max_commitments_per_block` e
`sumeragi.da_max_proof_openings_per_block` bloqueia o pacote DA antes de ser incorporado em um bloco e
cada compromisso deve conter um `proof_digest` diferente de zero. O guarda trata o comprimento do feixe como o
contagem de abertura de provas até que resumos de provas explícitas sejam encadeados por consenso, mantendo o
≤128 alvo de abertura executável no limite do bloco.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## Tratamento e redução de falhas PoROs trabalhadores de armazenamento agora apresentam faixas de falhas de PoR e recomendações de barras vinculadas ao lado de cada uma
veredicto. Falhas consecutivas acima do limite de ataque configurado emitem uma recomendação de que
inclui o par provedor/manifesto, o comprimento da sequência que acionou a barra e a proposta
multa calculada a partir do título de provedor e `penalty_bond_bps`; janelas de resfriamento (segundos) mantêm
barras duplicadas de disparo no mesmo incidente.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- Configurar limites/resfriamento por meio do construtor do trabalhador de armazenamento (os padrões refletem a governança
  política de penalidades).
- As recomendações de barra são registradas no JSON do resumo do veredicto para que a governança/auditores possam anexar
  para pacotes de evidências.
- Layout de distribuição + funções por bloco agora são encadeados através do ponto final do pino de armazenamento do Torii
  (campos `stripe_layout` + `chunk_roles`) e persistiu no trabalhador de armazenamento para
  auditores/ferramentas de reparo podem planejar reparos de linha/coluna sem derivar novamente o layout do upstream

### Colocação + chicote de reparo

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` agora
calcula um hash de posicionamento sobre `(index, role, stripe/column, offsets)` e executa primeiro a linha e depois
reparo da coluna RS (16) antes de reconstruir a carga útil:

- O posicionamento é padronizado como `total_stripes`/`shards_per_stripe` quando presente e retorna ao bloco
- Pedaços ausentes/corrompidos são reconstruídos primeiro com paridade de linha; as lacunas restantes são reparadas com
  paridade de faixa (coluna). Os pedaços reparados são gravados de volta no diretório de pedaços e o JSON
  summary captura o hash de posicionamento mais contadores de reparo de linha/coluna.
- Se a paridade linha+coluna não puder satisfazer o conjunto ausente, o chicote falha rapidamente com o conjunto irrecuperável
  índices para que os auditores possam sinalizar manifestos irreparáveis.