---
lang: pt
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-04T13:46:50.705991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Registro de alterações

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

Todas as alterações notáveis neste projeto serão documentadas neste arquivo.

## [Não lançado]

- Soltar o calço ESCALA; `norito::codec` agora é implementado com serialização Norito nativa.
- Substitua os usos de `parity_scale_codec` por `norito::codec` nas caixas.
- Comece a migrar ferramentas para serialização Norito nativa.
- Remova a dependência `parity-scale-codec` restante do espaço de trabalho em favor da serialização Norito nativa.
- Substitua as derivações residuais de características SCALE por implementações nativas Norito e renomeie o módulo de codec versionado.
- Mesclar `iroha_config_base_derive` e `iroha_futures_derive` em `iroha_derive` com macros controladas por recursos.
- *(multisig)* Rejeitar assinaturas diretas de autoridades multisig com um código/motivo de erro estável, aplicar limites TTL multisig em retransmissores aninhados e exibir limites TTL na CLI antes do envio (paridade do SDK pendente).
- Mova as macros procedimentais FFI para `iroha_ffi` e remova a caixa `iroha_ffi_derive`.
- *(schema_gen)* Remova o recurso `transparent_api` desnecessário da dependência `iroha_data_model`.
- *(data_model)* Armazene em cache o normalizador NFC da ICU para análise `Name` para reduzir a sobrecarga de inicialização repetida.
- 📚 Início rápido do documento JS, resolvedor de configuração, fluxo de trabalho de publicação e receita com reconhecimento de configuração para o cliente Torii.
- *(IrohaSwift)* Aumente as metas mínimas de implantação para iOS 15/macOS 12, adote a simultaneidade Swift nas APIs do cliente Torii e marque os modelos públicos como `Sendable`.
- *(IrohaSwift)* Adicionado `ToriiDaProofSummaryArtifact` e `DaProofSummaryArtifactEmitter.emit` para que aplicativos Swift possam construir/emitir pacotes de prova DA compatíveis com CLI sem gastar dinheiro com a CLI, completos com documentos e testes de regressão cobrindo tanto na memória quanto no disco fluxos de trabalho.【F:IrohaSwift/Fontes/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Testes/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* Corrija a serialização da opção Kaigi removendo o sinalizador de reutilização arquivada de `KaigiParticipantCommitment`, adicione testes de ida e volta nativos e elimine o substituto de decodificação JS para que as instruções Kaigi agora Norito ida e volta antes submissão.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* Permitir que os chamadores `ToriiClient` excluam os cabeçalhos padrão (passando `null`) para que `getMetrics` alterne perfeitamente entre o texto JSON e Prometheus Aceitar cabeçalhos.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* Adicionados auxiliares iteráveis para NFTs, saldos de ativos por conta e detentores de definição de ativos (com definições, documentos e testes TypeScript) para que a paginação Torii agora cubra o aplicativo restante pontos finais.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* Adicionados construtores de instruções/transações de governança, além de uma receita de governança para que os clientes JS possam preparar propostas de implantação, votações, promulgação e persistência do conselho de ponta a ponta fim.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* Adicionados auxiliares de envio/status ISO 20022 pacs.008 e uma receita correspondente, permitindo que os chamadores JS exercitem a ponte ISO Torii sem HTTP personalizado encanamento.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】
- *(javascript)* Adicionados ajudantes do construtor pacs.008/pacs.009, além de uma receita baseada em configuração para que os chamadores JS possam sintetizar cargas úteis ISO 20022 com metadados BIC/IBAN validados antes de acessar o ponte.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* Concluído o loop de ingestão/busca/prova do DA: `ToriiClient.fetchDaPayloadViaGateway` agora deriva automaticamente identificadores de chunker (por meio da nova ligação `deriveDaChunkerHandle`), resumos de prova opcionais reutilizam o `generateDaProofSummary` nativo e o README/digitações/testes foram atualizados para que os chamadores do SDK possam espelhar `iroha da get-blob/prove-availability` sem sob medida encanamento.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascript t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* Os metadados do placar `sorafsGatewayFetch` agora registram o ID/CID do manifesto do gateway sempre que os provedores de gateway são usados para que os artefatos de adoção se alinhem com as capturas CLI.
- *(torii/cli)* Aplicar cruzamentos ISO: Torii agora rejeita envios `pacs.008` com BICs de agentes desconhecidos e a visualização DvP CLI valida `--delivery-instrument-id` via `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* Adicione ingestão de dinheiro PvP via `POST /v1/iso20022/pacs009`, aplicando verificações de dados de referência `Purp=SECU` e BIC antes de construir transferências.
- *(ferramentas)* Adicionado `cargo xtask iso-bridge-lint` (mais `ci/check_iso_reference_data.sh`) para validar instantâneos ISIN/CUSIP, BIC↔LEI e MIC junto com acessórios de repositório.【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* Publicação npm reforçada declarando metadados de repositório, uma lista de permissões de arquivos explícita, `publishConfig` habilitado para proveniência, um changelog/proteção de teste `prepublishOnly` e um fluxo de trabalho GitHub Actions que exercita o Node 18/20 em CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* O campo add/sub/mul BN254 agora é executado nos novos kernels CUDA com lote do lado do host via `bn254_launch_kernel`, permitindo aceleração de hardware para gadgets Poseidon e ZK enquanto preserva a determinística substitutos.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 08/05/2025

### 🚀 Recursos

- *(cli)* Adicione `iroha transaction get` e outros comandos importantes (#5289)
- [**quebra**] Separar ativos fungíveis e não fungíveis (#5308)
- [**quebra**] Finalize blocos não vazios permitindo blocos vazios depois deles (#5320)
- Expor tipos de telemetria no esquema e no cliente (#5387)
- *(iroha_torii)* Stubs para endpoints controlados por recursos (#5385)
- Adicionar métricas de tempo de confirmação (#5380)

### 🐛 Correções de bugs

- Revisar Não-Zeros (#5278)
- Erros de digitação em arquivos de documentação (#5309)
- *(cripto)* Exponha o getter `Signature::payload` (#5302) (#5310)
- *(core)* Adicione verificações de presença de função antes de concedê-la (#5300)
- *(core)* Reconecte o peer desconectado (#5325)
- Corrigir testes de py relacionados a ativos de loja e NFT (#5341)
- *(CI)* Corrigir fluxo de trabalho de análise estática python para poesia v2 (#5374)
- O evento de transação expirada aparece após o commit (#5396)

### 💼 Outros

- Inclui `rust-toolchain.toml` (#5376)
- Avisar sobre `unused`, não `deny` (#5377)

### 🚜 Refatorar

- Guarda-chuva Iroha CLI (#5282)
- *(iroha_test_network)* Use formato bonito para logs (#5331)
- [**quebra**] Simplifique a serialização de `NumericSpec` em `genesis.json` (#5340)
- Melhorar o registro de falhas na conexão p2p (#5379)
- Reverter `logger.level`, adicionar `logger.filter`, estender rotas de configuração (#5384)

### 📚 Documentação

- Adicione `network.public_address` a `peer.template.toml` (#5321)

### ⚡ Desempenho

- *(kura)* Impedir gravações redundantes de blocos no disco (#5373)
- Implementado armazenamento personalizado para hashes de transações (#5405)

### ⚙️ Tarefas Diversas

- Corrigir o uso de poesia (#5285)
- Remover consts redundantes de `iroha_torii_const` (#5322)
- Remova `AssetEvent::Metadata*` não utilizado (#5339)
- Versão Bump Sonarqube Action (#5337)
- Remova permissões não utilizadas (#5346)
- Adicione o pacote de descompactação ao ci-image (#5347)
- Corrija alguns comentários (#5397)
- Mover testes de integração para fora da caixa `iroha` (#5393)
- Desativar trabalho defectdojo (#5406)
- Adicionar aprovação DCO para commits ausentes
- Reorganizar fluxos de trabalho (segunda tentativa) (#5399)
- Não execute Pull Request CI ao enviar para main (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 07/03/2025

### Adicionado

- finalizar blocos não vazios permitindo blocos vazios depois deles (#5320)

## [2.0.0-rc.1.2] - 25/02/2025

### Corrigido

- pares registrados novamente agora são refletidos corretamente na lista de pares (#5327)

## [2.0.0-rc.1.1] - 12/02/2025

### Adicionado

- adicione `iroha transaction get` e outros comandos importantes (#5289)

## [2.0.0-rc.1.0] - 06/12/2024

### Adicionado- implementar projeções de consulta (#5242)
- use executor persistente (#5082)
- adicionar tempos limite de escuta ao iroha cli (#5241)
- adicionar endpoint da API /peers ao torii (#5235)
- endereço p2p agnóstico (#5176)
- melhorar a utilidade e usabilidade multisig (#5027)
- protege `BasicAuth::password` de ser impresso (#5195)
- classificar em ordem decrescente na consulta `FindTransactions` (#5190)
- introduzir cabeçalho de bloco em cada contexto de execução de contrato inteligente (#5151)
- tempo de commit dinâmico baseado no índice de mudança de visualização (#4957)
- definir conjunto de permissões padrão (#5075)
- adicionar implementação de Niche para `Option<Box<R>>` (#5094)
- predicados de transação e bloco (#5025)
- reportar quantidade de itens restantes em consulta (#5016)
- tempo discreto limitado (#4928)
- adicione operações matemáticas ausentes a `Numeric` (#4976)
- validar mensagens de sincronização de bloco (#4965)
- filtros de consulta (#4833)

### Alterado

- simplificar a análise de ID de peer (#5228)
- mover erro de transação para fora da carga útil do bloco (#5118)
- renomeie JsonString para Json (#5154)
- adicionar entidade cliente a contratos inteligentes (#5073)
- líder como serviço de pedidos de transações (#4967)
- faça kura eliminar blocos antigos da memória (#5103)
- use `ConstVec` para obter instruções em `Executable` (#5096)
- fofocas txs no máximo uma vez (#5079)
- reduzir o uso de memória de `CommittedTransaction` (#5089)
- tornar os erros do cursor de consulta mais específicos (#5086)
- reorganizar caixas (#4970)
- introduzir a consulta `FindTriggers`, remover `FindTriggerById` (#5040)
- não dependa de assinaturas para atualização (#5039)
- alterar o formato dos parâmetros em genesis.json (#5020)
- enviar apenas prova de alteração da visualização atual e anterior (#4929)
- desabilitar o envio de mensagem quando não estiver pronto para evitar loop ocupado (#5032)
- mover a quantidade total de ativos para a definição de ativos (#5029)
- assine apenas o cabeçalho do bloco, não toda a carga útil (#5000)
- use `HashOf<BlockHeader>` como o tipo de hash do bloco (#4998)
- simplifique `/health` e `/api_version` (#4960)
- renomeie `configs` para `defaults`, remova `swarm` (#4862)

### Corrigido

- nivelar a função interna em json (#5198)
- corrigir avisos `cargo audit` (#5183)
- adicionar verificação de intervalo ao índice de assinatura (#5157)
- corrigir exemplo de macro de modelo em documentos (#5149)
- feche o ws corretamente no fluxo de blocos/eventos (#5101)
- verificação de pares confiáveis quebrados (#5121)
- verifique se o próximo bloco tem altura +1 (#5111)
- corrigir carimbo de data/hora do bloco genesis (#5098)
- corrigir a compilação `iroha_genesis` sem o recurso `transparent_api` (#5056)
- manusear corretamente `replace_top_block` (#4870)
- corrigir clonagem do executor (#4955)
- exibir mais detalhes do erro (#4973)
- use `GET` para fluxo de blocos (#4990)
- melhorar o tratamento de transações de fila (#4947)
- evita mensagens redundantes de bloqueio de sincronização de bloco (#4909)
- evita impasses no envio simultâneo de mensagens grandes (#4948)
- remover transação expirada do cache (#4922)
- corrigir URL do torii com caminho (#4903)

### Removido

- remover API baseada em módulo do cliente (#5184)
- remover `riffle_iter` (#5181)
- remover dependências não utilizadas (#5173)
- remova o prefixo `max` de `blocks_in_memory` (#5145)
- remover estimativa de consenso (#5116)
- remova `event_recommendations` do bloco (#4932)

### Segurança

## [2.0.0-pré-rc.22.1] - 30/07/2024

### Corrigido

- adicionado `jq` à imagem do docker

## [2.0.0-pré-rc.22.0] - 25/07/2024

### Adicionado

- especifique parâmetros on-chain explicitamente no genesis (#4812)
- permitir turbofish com vários `Instruction`s (#4805)
- reimplementar transações com múltiplas assinaturas (#4788)
- implementar parâmetros on-chain integrados versus personalizados (#4731)
- melhorar o uso de instruções personalizadas (#4778)
- tornar os metadados dinâmicos implementando JsonString (#4732)
- permitir que vários pares enviem bloco de gênese (#4775)
- forneça `SignedBlock` em vez de `SignedTransaction` para peer (#4739)
- instruções personalizadas no executor (#4645)
- estender o cli do cliente para solicitar consultas json (#4684)
 - adicionar suporte de detecção para `norito_decoder` (#4680)
- generalizar o esquema de permissões para o modelo de dados do executor (#4658)
- adicionadas permissões de gatilho de registro no executor padrão (#4616)
 - suporte JSON em `norito_cli`
- introduzir tempo limite de inatividade p2p

### Alterado

- substitua `lol_alloc` por `dlmalloc` (#4857)
- renomeie `type_` para `type` no esquema (#4855)
- substitua `Duration` por `u64` no esquema (#4841)
- use EnvFilter tipo `RUST_LOG` para registro (#4837)
- mantenha o bloco de votação quando possível (#4828)
- migrar de warp para axum (#4718)
- modelo de dados do executor dividido (#4791)
- modelo de dados raso (#4734) (#4792)
- não envie chave pública com assinatura (#4518)
- renomeie `--outfile` para `--out-file` (#4679)
- renomear servidor e cliente iroha (#4662)
- renomeie `PermissionToken` para `Permission` (#4635)
- rejeitar `BlockMessages` ansiosamente (#4606)
- tornar `SignedBlock` imutável (#4620)
- renomeie TransactionValue para CommittedTransaction (#4610)
- autenticar contas pessoais por ID (#4411)
- use o formato multihash para chaves privadas (#4541)
 - renomear `parity_scale_decoder` para `norito_cli`
- enviar blocos para validadores do Conjunto B
- tornar `Role` transparente (#4886)
- derivar hash de bloco do cabeçalho (#4890)

### Corrigido

- verifique se a autoridade possui o domínio para transferir (#4807)
- remover inicialização dupla do logger (#4800)
- corrigir convenção de nomenclatura para ativos e permissões (#4741)
- atualize o executor em transação separada no bloco genesis (#4757)
- valor padrão correto para `JsonString` (#4692)
- melhorar a mensagem de erro de desserialização (#4659)
- não entre em pânico se a chave pública Ed25519Sha512 passada tiver comprimento inválido (#4650)
- use o índice de alteração de visualização adequado no carregamento do bloco init (#4612)
- não execute gatilhos de tempo prematuramente antes do carimbo de data / hora `start` (#4333)
- suporte `https` para `torii_url` (#4601) (#4617)
- remova serde(flatten) de SetKeyValue/RemoveKeyValue (#4547)
- o conjunto de gatilhos está serializado corretamente
- revogar `PermissionToken`s removidos em `Upgrade<Executor>` (#4503)
- relatar o índice de alteração de visualização correto para a rodada atual
- remova os gatilhos correspondentes em `Unregister<Domain>` (#4461)
- verifique a chave do pub genesis na rodada genesis
- impedir o registro de domínio ou conta genesis
- remover permissões de funções no cancelamento do registro da entidade
- os metadados do gatilho estão acessíveis em contratos inteligentes
- use rw lock para evitar visualização de estado inconsistente (#4867)
- lidar com soft fork no snapshot (#4868)
- corrigir MinSize para ChaCha20Poly1305
- adicione limites ao LiveQueryStore para evitar alto uso de memória (#4893)

### Removido

- remover a chave pública da chave privada ed25519 (#4856)
- remover kura.lock (#4849)
- reverter os sufixos `_ms` e `_bytes` na configuração (#4667)
- remova o sufixo `_id` e `_file` dos campos genesis (#4724)
- remover ativos de índice em AssetsMap por AssetDefinitionId (#4701)
- remover domínio da identidade do gatilho (#4640)
- remover assinatura genesis de Iroha (#4673)
- remova `Visit` vinculado de `Validate` (#4642)
- remover `TriggeringEventFilterBox` (#4866)
- remova `garbage` no handshake p2p (#4889)
- remova `committed_topology` do bloco (#4880)

### Segurança

- proteja-se contra vazamento de segredos

## [2.0.0-pré-rc.21] - 19/04/2024

### Adicionado

- incluir o ID do gatilho no ponto de entrada do gatilho (#4391)
- expor o conjunto de eventos como campos de bits no esquema (#4381)
- introduzir o novo `wsv` com acesso granular (#2664)
- adicionar filtros de eventos para eventos `PermissionTokenSchemaUpdate`, `Configuration` e `Executor`
- introduzir o "modo" de instantâneo (#4365)
- permitir conceder/revogar permissões da função (#4244)
- introduzir tipo numérico de precisão arbitrária para ativos (remover todos os outros tipos numéricos) (#3660)
- limite de combustível diferente para o Executor (#3354)
- integrar o perfilador pprof (#4250)
- adicionar subcomando de ativos na CLI do cliente (#4200)
- Permissões `Register<AssetDefinition>` (#4049)
- adicione `chain_id` para evitar ataques de repetição (#4185)
- adicionar subcomandos para editar metadados de domínio na CLI do cliente (#4175)
- implementar operações de conjunto de armazenamento, remoção e obtenção na CLI do cliente (#4163)
- contar contratos inteligentes idênticos para gatilhos (#4133)
- adicione subcomando na CLI do cliente para transferir domínios (#3974)
- suporta fatias em caixa em FFI (#4062)
- git commit SHA para CLI do cliente (#4042)
- macro proc para padrão do validador padrão (#3856)
- introduziu o construtor de solicitação de consulta na API do cliente (#3124)
- consultas preguiçosas dentro de contratos inteligentes (#3929)
- Parâmetro de consulta `fetch_size` (#3900)
- instrução de transferência de armazenamento de ativos (#4258)
- proteja-se contra vazamento de segredos (#3240)
- desduplicar gatilhos com o mesmo código-fonte (#4419)

### Alterado- bata o conjunto de ferramentas de ferrugem todas as noites-2024-04-18
- enviar blocos para validadores do Conjunto B (#4387)
- dividir eventos de pipeline em eventos de bloco e transação (#4366)
- renomeie a seção de configuração `[telemetry.dev]` para `[dev_telemetry]` (#4377)
- crie tipos não genéricos `Action` e `Filter` (#4375)
- melhorar a API de filtragem de eventos com padrão de construtor (#3068)
- unificar várias APIs de filtro de eventos, introduzir uma API de construtor fluente
- renomeie `FilterBox` para `EventFilterBox`
- renomeie `TriggeringFilterBox` para `TriggeringEventFilterBox`
- melhorar a nomenclatura do filtro, por ex. `AccountFilter` -> `AccountEventFilter`
- reescrever a configuração de acordo com a configuração RFC (#4239)
- ocultar a estrutura interna das estruturas versionadas da API pública (#3887)
- introduzir temporariamente uma ordem previsível após muitas alterações de visualização com falha (#4263)
- use tipos de chave concretos em `iroha_crypto` (#4181)
- alterações na visualização dividida de mensagens normais (#4115)
- tornar `SignedTransaction` imutável (#4162)
- exportar `iroha_config` até `iroha_client` (#4147)
- exportar `iroha_crypto` até `iroha_client` (#4149)
- exportar `data_model` até `iroha_client` (#4081)
- remova a dependência `openssl-sys` de `iroha_crypto` e introduza backends tls configuráveis para `iroha_client` (#3422)
- substitua o EOF `hyperledger/ursa` não mantido pela solução interna `iroha_crypto` (#3422)
- otimizar o desempenho do executor (#4013)
- atualização de pares de topologia (#3995)

### Corrigido

- remova os gatilhos correspondentes em `Unregister<Domain>` (#4461)
- remover permissões de funções no cancelamento de registro de entidade (#4242)
- afirmar que a transação do genesis é assinada pela chave pub do genesis (#4253)
- introduza o tempo limite para pares que não respondem em p2p (#4267)
- impedir o registro de domínio ou conta genesis (#4226)
-`MinSize` para `ChaCha20Poly1305` (#4395)
- inicie o console quando `tokio-console` estiver habilitado (#4377)
- separe cada item com `\n` e crie recursivamente diretórios pai para logs de arquivo `dev-telemetry`
- impedir o registro de contas sem assinaturas (#4212)
- a geração de pares de chaves agora é infalível (#4283)
- pare de codificar chaves `X25519` como `Ed25519` (#4174)
- faça validação de assinatura em `no_std` (#4270)
- chamando métodos de bloqueio dentro do contexto assíncrono (#4211)
- revogar tokens associados no cancelamento do registro da entidade (#3962)
- bug de bloqueio assíncrono ao iniciar Sumeragi
- `(get|set)_config` 401 HTTP corrigido (#4177)
- Nome do arquivador `musl` em Docker (#4193)
- impressão de depuração de contrato inteligente (#4178)
- atualização da topologia na reinicialização (#4164)
- registro de novo par (#4142)
- ordem de iteração previsível na cadeia (#4130)
- re-arquitetar o registrador e a configuração dinâmica (#4100)
- disparar atomicidade (#4106)
- problema de ordenação de mensagens do armazenamento de consultas (#4057)
- defina `Content-Type: application/x-norito` para endpoints que respondem usando Norito

### Removido

- Parâmetro de configuração `logger.tokio_console_address` (#4377)
-`NotificationEvent` (#4377)
- enumeração `Value` (#4305)
- Agregação MST de iroha (#4229)
- clonagem para ISI e execução de consultas em contratos inteligentes (#4182)
- Recursos `bridge` e `dex` (#4152)
- eventos achatados (#3068)
- expressões (#4089)
- referência de configuração gerada automaticamente
- Ruído `warp` em registros (#4097)

### Segurança

- evitar falsificação de chave pública em p2p (#4065)
- garantir que as assinaturas `secp256k1` provenientes do OpenSSL sejam normalizadas (#4155)

## [2.0.0-pré-rc.20] - 17/10/2023

### Adicionado

- Transferir propriedade `Domain`
- Permissões do proprietário `Domain`
- Adicionar campo `owned_by` a `Domain`
- analisar filtro como JSON5 em `iroha_client_cli` (#3923)
- Adicionar suporte para uso de tipo próprio em enums parcialmente marcados com serde
- Padronizar API de bloco (#3884)
- Implementar o modo de inicialização `Fast` kura
- Adicionar cabeçalho de isenção de responsabilidade iroha_swarm
- suporte inicial para instantâneos WSV

### Corrigido

- Corrigido o download do executor em update_configs.sh (#3990)
- ferrugem adequada no devShell
- Corrigir reprições de gravação `Trigger`
- Correção de transferência `AssetDefinition`
- Corrigir `RemoveKeyValue` para `Domain`
- Corrigir o uso de `Span::join`
- Corrigir bug de incompatibilidade de topologia (#3903)
- Corrigir benchmark `apply_blocks` e `validate_blocks`
- `mkdir -r` com caminho de armazenamento, não caminho de bloqueio (#3908)
- Não falhe se dir existir em test_env.py
- Corrigir docstring de autenticação/autorização (#3876)
- Melhor mensagem de erro para erro de localização de consulta
- Adicionar chave pública da conta genesis ao dev docker compose
- Compare a carga útil do token de permissão como JSON (#3855)
- Corrigir `irrefutable_let_patterns` na macro `#[model]`
- Permitir que o genesis execute qualquer ISI (#3850)
- Corrigir validação de gênese (#3844)
- Corrigir topologia para 3 ou menos pares
- Corrija como o histograma tx_amounts é calculado.
- Teste de flacidez `genesis_transactions_are_validated()`
- Geração de validador padrão
- Corrigir desligamento normal de iroha

### Refatorar

- remover dependências não utilizadas (#3992)
- aumentar dependências (#3981)
- Renomear validador para executor (#3976)
- Remover `IsAssetDefinitionOwner` (#3979)
- Incluir código de contrato inteligente no espaço de trabalho (#3944)
- Mesclar endpoints de API e telemetria em um único servidor
- mover a expressão len da API pública para o núcleo (#3949)
- Evite clonar na pesquisa de funções
- Consultas de intervalo para funções
- Mover funções de conta para `WSV`
- Renomeie ISI de *Box para *Expr (#3930)
- Remover o prefixo 'Versionado' dos contêineres versionados (#3913)
- mova `commit_topology` para a carga útil do bloco (#3916)
- Migrar macro `telemetry_future` para syn 2.0
- Registrado com Identifiable nos limites ISI (#3925)
- Adicionar suporte genérico básico ao `derive(HasOrigin)`
- Limpe a documentação das APIs do Emissor para deixar o clippy feliz
- Adicionar testes para a macro deriva(HasOrigin), reduzir a repetição em derivar(IdEqOrdHash), corrigir relatórios de erros no estável
- Melhore a nomenclatura, simplifique .filter_maps repetidos e livre-se de .except desnecessários em derive(Filter)
- Faça uso de PartiallyTaggedSerialize/Deserialize, querido
- Faça derive(IdEqOrdHash) usar querido, adicione testes
- Faça derivar (Filtro) usar querido
- Atualize iroha_data_model_derive para usar syn 2.0
- Adicionar testes unitários de condição de verificação de assinatura
- Permitir apenas um conjunto fixo de condições de verificação de assinatura
- Generalize ConstBytes em um ConstVec que contém qualquer sequência const
- Use uma representação mais eficiente para valores de bytes que não estão mudando
- Armazene wsv finalizado em instantâneo
- Adicionar ator `SnapshotMaker`
- a limitação do documento de análise deriva em macros proc
- limpar comentários
- extrair um utilitário de teste comum para analisar atributos para lib.rs
- use parse_display e atualize Attr -> Nomeação de Attrs
- permitir o uso de correspondência de padrões em argumentos de função ffi
- reduza a repetição na análise de atributos getset
- renomear Emitter::into_token_stream para Emitter::finish_token_stream
- Use parse_display para analisar tokens getset
- Corrija erros de digitação e melhore mensagens de erro
- iroha_ffi_derive: use querido para analisar atributos e use syn 2.0
- iroha_ffi_derive: substitua proc-macro-error por manyhow
- Simplifique o código do arquivo de bloqueio kura
- faz com que todos os valores numéricos sejam serializados como literais de string
- Dividir Kagami (#3841)
- Reescrever `scripts/test-env.sh`
- Diferenciar entre contrato inteligente e pontos de entrada de gatilho
-Elide `.cloned()` em `data_model/src/block.rs`
- atualize `iroha_schema_derive` para usar syn 2.0

## [2.0.0-pré-rc.19] - 14/08/2023

### Adicionado

- tempo de execução hyperledger#3309 Bump IVM para melhorar
- hyperledger#3383 Implementar macro para analisar endereços de soquete em tempo de compilação
- hyperledger#2398 Adicionar testes de integração para filtros de consulta
- Incluir a mensagem de erro real em `InternalError`
- Uso de `nightly-2023-06-25` como cadeia de ferramentas padrão
- migração do validador hyperledger#3692
- [estágio DSL] hyperledger#3688: Implementar aritmética básica como macro proc
- hyperledger#3371 Divida o validador `entrypoint` para garantir que os validadores não sejam mais vistos como contratos inteligentes
- Instantâneos WSV do hyperledger#3651, que permitem ativar um nó Iroha rapidamente após uma falha
- hyperledger#3752 Substitua `MockValidator` por um validador `Initial` que aceita todas as transações
- hyperledger#3276 Adicionar instrução temporária chamada `Log` que registra uma string especificada no log principal do nó Iroha
- hyperledger#3641 Torne a carga útil do token de permissão legível por humanos
- hyperledger#3324 Adicionar verificações e refatorações `burn` relacionadas a `iroha_client_cli`
- hyperledger#3781 Validar transações genesis
- hyperledger#2885 Diferenciar entre eventos que podem e não podem ser usados para gatilhos
- compilação baseada em hyperledger#2245 `Nix` do binário do nó iroha como `AppImage`

### Corrigido- hyperledger#3613 Regressão que pode permitir a aceitação de transações assinadas incorretamente
- Rejeitar antecipadamente a topologia de configuração incorreta
- hyperledger#3445 Corrija a regressão e faça `POST` no endpoint `/configuration` funcionar novamente
- hyperledger#3654 Correção `iroha2` `glibc` `Dockerfiles` a ser implantado
- hyperledger#3451 Correção `docker` construída em Macs de silício da Apple
- hyperledger#3741 Corrigir erro `tempfile` em `kagami validator`
- hyperledger#3758 Corrige regressão onde caixas individuais não podiam ser construídas, mas poderiam ser construídas como parte do espaço de trabalho
- hyperledger#3777 Brecha de patch no registro de função não sendo validada
- hyperledger#3805 Correção de Iroha que não desliga após receber `SIGTERM`

### Outros

- hyperledger#3648 Incluir verificação `docker-compose.*.yml` nos processos de CI
- Mova a instrução `len()` de `iroha_data_model` para `iroha_core`
- hyperledger#3672 Substitua `HashMap` por `FxHashMap` em macros derivadas
- hyperledger#3374 Unificar comentários de documentos de erro e implementação `fmt::Display`
- hyperledger#3289 Use herança de espaço de trabalho Rust 1.70 em todo o projeto
- hyperledger#3654 Adicione `Dockerfiles` para construir iroha2 em `GNU libc <https://www.gnu.org/software/libc/>`_
- Introduzir `syn` 2.0, `manyhow` e `darling` para macros proc
- hyperledger#3802 Unicode `kagami crypto` semente

## [2.0.0-pré-rc.18]

### Adicionado

- hyperledger#3468: cursor do lado do servidor, que permite paginação reentrante avaliada preguiçosamente, o que deve ter implicações positivas importantes no desempenho da latência da consulta
- hyperledger#3624: Tokens de permissão de uso geral; especificamente
  - Os tokens de permissões podem ter qualquer estrutura
  - A estrutura do token é autodescrita no `iroha_schema` e serializada como uma string JSON
  - O valor do token é codificado em `Norito`
  - como consequência desta alteração, a convenção de nomenclatura do token de permissão foi movida de `snake_case` para `UpeerCamelCase`
- hyperledger#3615 Preservar wsv após validação

### Corrigido

- hyperledger#3627 A atomicidade da transação agora é aplicada por meio da clonagem do `WorlStateView`
- hyperledger#3195 Estende o comportamento de pânico ao receber uma transação genesis rejeitada
- hyperledger#3042 Corrigir mensagem de solicitação incorreta
- hyperledger#3352 Divida o fluxo de controle e a mensagem de dados em canais separados
- hyperledger#3543 Melhore a precisão das métricas

## 2.0.0-pré-rc.17

### Adicionado

- hyperledger#3330 Estende a desserialização `NumericValue`
- suporte hyperledger#2622 `u128`/`i128` em FFI
- hyperledger#3088 Introduzir otimização de fila, para evitar DoS
- variantes de comando hyperledger#2373 `kagami swarm file` e `kagami swarm dir` para gerar arquivos `docker-compose`
- Análise de token de permissão hyperledger#3597 (lado Iroha)
- hyperledger#3353 Remova `eyre` de `block.rs` enumerando condições de erro e usando erros de digitação forte
- hyperledger#3318 Intercalar transações rejeitadas e aceitas em blocos para preservar a ordem de processamento da transação

### Corrigido

- hyperledger#3075 Panic em transação inválida no `genesis.json` para evitar que transações inválidas sejam processadas
- hyperledger#3461 Tratamento adequado de valores padrão na configuração padrão
- hyperledger#3548 Corrigir atributo transparente `IntoSchema`
- hyperledger#3552 Corrige a representação do esquema do caminho do validador
- hyperledger#3546 Correção para gatilhos de tempo travados
- hyperledger#3162 Proibir altura 0 em solicitações de streaming de bloco
- Teste inicial da macro de configuração
- hyperledger#3592 Correção para arquivos de configuração sendo atualizados em `release`
- hyperledger#3246 Não envolva `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ sem `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_
- hyperledger#3570 Exibir corretamente erros de consulta de string do lado do cliente
- hyperledger#3596 `iroha_client_cli` mostra blocos/eventos
- hyperledger#3473 Faça `kagami validator` funcionar fora do diretório raiz do repositório iroha

### Outros

- hyperledger#3063 Mapeie a transação `hash` para bloquear a altura em `wsv`
- `HashOf<T>` fortemente digitado em `Value`

## [2.0.0-pré-rc.16]

### Adicionado

- subcomando hyperledger#2373 `kagami swarm` para gerar `docker-compose.yml`
- hyperledger#3525 Padronizar API de transação
- hyperledger#3376 Adicionar estrutura de automação Iroha CLI do cliente `pytest <https://docs.pytest.org/en/7.4.x/>`_
- hyperledger#3516 Reter o hash do blob original em `LoadedExecutable`

### Corrigido

- hyperledger#3462 Adicionar comando de ativo `burn` a `client_cli`
- hyperledger#3233 Tipos de erro de refatoração
- hyperledger#3330 Corrigir regressão, implementando manualmente `serde::de::Deserialize` para `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums`
- hyperledger#3487 Retorna tipos ausentes no esquema
- hyperledger#3444 Retorna discriminante no esquema
- hyperledger#3496 Corrigir análise de campo `SocketAddr`
- hyperledger#3498 Corrigir detecção de soft-fork
- hyperledger#3396 Armazena o bloco em `kura` antes de emitir um evento de bloco confirmado

### Outros

- hyperledger#2817 Remover mutabilidade interna de `WorldStateView`
- refatorador da API Genesis hyperledger#3363
- Refatorar os existentes e complementar com novos testes para topologia
- Mude de `Codecov <https://about.codecov.io/>`_ para `Coveralls <https://coveralls.io/>`_ para cobertura de teste
- hyperledger#3533 Renomeie `Bool` para `bool` no esquema

## [2.0.0-pré-rc.15]

### Adicionado

- validador monolítico hyperledger#3231
- hyperledger#3015 Suporte para otimização de nicho em FFI
- hyperledger#2547 Adicionar logotipo a `AssetDefinition`
- hyperledger#3274 Adicionar ao `kagami` um subcomando que gera exemplos (portados para LTS)
- hiperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ floco
- hyperledger#3412 Mova a fofoca da transação para um ator separado
- hyperledger#3435 Apresente o visitante `Expression`
- hyperledger#3168 Fornece o validador genesis como um arquivo separado
- hyperledger#3454 Torne o LTS o padrão para a maioria das operações e documentação do Docker
- hyperledger#3090 Propagar parâmetros on-chain do blockchain para `sumeragi`

### Corrigido

- hyperledger#3330 Corrigir a desserialização de enum não marcada com folhas `u128` (portadas para RC14)
- hyperledger#2581 reduziu o ruído nos logs
- Hyperledger#3360 Correção de benchmark `tx/s`
- hyperledger#3393 Interrompa o loop de impasse de comunicação em `actors`
- hyperledger#3402 Corrigir compilação `nightly`
- hyperledger#3411 Manipular adequadamente a conexão simultânea de pares
- hyperledger#3440 Descontinuar conversões de ativos durante a transferência, em vez disso tratadas por contratos inteligentes
- hyperledger#3408: Corrigir teste `public_keys_cannot_be_burned_to_nothing`

### Outros

- hyperledger#3362 Migrar para atores `tokio`
- hyperledger#3349 Remover `EvaluateOnHost` de contratos inteligentes
- hyperledger#1786 Adicionar tipos nativos `iroha` para endereços de soquete
- Desativar cache IVM
- Reativar o cache IVM
- Renomeie o validador de permissão para validador
- hyperledger#3388 Torne `model!` uma macro de atributo em nível de módulo
- hyperledger#3370 Serialize `hash` como string hexadecimal
- Mover `maximum_transactions_in_block` da configuração `queue` para `sumeragi`
- Descontinuar e remover o tipo `AssetDefinitionEntry`
- Renomeie `configs/client_cli` para `configs/client`
- Atualização `MAINTAINERS.md`

## [2.0.0-pré-rc.14]

### Adicionado

- modelo de dados hyperledger#3127 `structs` opaco por padrão
- hyperledger#3122 usa `Algorithm` para armazenar a função digest (colaborador da comunidade)
- A saída do hyperledger#3153 `iroha_client_cli` é legível por máquina
- hyperledger#3105 Implementar `Transfer` para `AssetDefinition`
- hyperledger#3010 `Transaction` evento de expiração de pipeline adicionado

### Corrigido

- revisão hyperledger#3113 de testes de rede instável
- hyperledger#3129 Corrigir des/serialização `Parameter`
- hyperledger#3141 Implementar manualmente `IntoSchema` para `Hash`
- hyperledger#3155 Corrige gancho de pânico em testes, evitando deadlock
- hyperledger#3166 Não visualiza alterações em modo inativo, melhorando o desempenho
- hyperledger#2123 Retornar à desserialização/desserialização de PublicKey do multihash
- hyperledger#3132 Adicionar validador NewParameter
- hyperledger#3249 Dividir hashes de bloco em versões parciais e completas
- hyperledger#3031 Corrija a UI/UX de parâmetros de configuração ausentes
- hyperledger#3247 Removida injeção de falha de `sumeragi`.

### Outros

- Adicione `#[cfg(debug_assertions)]` ausente para corrigir falhas espúrias
- hyperledger#2133 Reescreva a topologia para ficar mais próximo do white paper
- Remover dependência `iroha_client` de `iroha_core`
- hyperledger#2943 Derivar `HasOrigin`
- hyperledger#3232 Compartilhe metadados do espaço de trabalho
- hyperledger#3254 Refator `commit_block()` e `replace_top_block()`
- Use o manipulador de alocador padrão estável
- hyperledger#3183 Renomeie os arquivos `docker-compose.yml`
- Melhorou o formato de exibição `Multihash`
- hyperledger#3268 Identificadores de itens exclusivos globalmente
- Novo modelo de relações públicas

## [2.0.0-pré-rc.13]

### Adicionado- hyperledger#2399 Configuração de parâmetros como ISI.
- hyperledger#3119 Adicionar métrica `dropped_messages`.
- hyperledger#3094 Gere rede com pares `n`.
- hyperledger#3082 Fornece dados completos no evento `Created`.
- hyperledger#3021 Importação de ponteiro opaco.
- hyperledger#2794 Rejeite enumerações sem campo com discriminantes explícitos em FFI.
- hyperledger#2922 Adicione `Grant<Role>` ao genesis padrão.
- hyperledger#2922 Omita o campo `inner` na desserialização json `NewRole`.
- hyperledger#2922 Omita `object(_id)` na desserialização json.
- hyperledger#2922 Omita `Id` na desserialização json.
- hyperledger#2922 Omita `Identifiable` na desserialização json.
- hyperledger#2963 Adicione `queue_size` às métricas.
- hyperledger#3027 implementa lockfile para Kura.
- hyperledger#2813 Kagami gera configuração de peer padrão.
- hyperledger#3019 Suporte JSON5.
- hyperledger#2231 Gera API de wrapper FFI.
- hyperledger#2999 Acumule assinaturas de bloco.
- hyperledger#2995 Detecção de soft fork.
- hyperledger#2905 Estende operações aritméticas para suportar `NumericValue`
- hyperledger#2868 Emite a versão iroha e confirma o hash nos logs.
- hyperledger#2096 Consulta do valor total do ativo.
- hyperledger#2899 Adicionar subcomando de múltiplas instruções em 'client_cli'
- hyperledger#2247 Remova o ruído de comunicação do websocket.
- hyperledger#2889 Adicionar suporte de streaming de bloco em `iroha_client`
- hyperledger#2280 Produz eventos de permissão quando a função é concedida/revogada.
- hyperledger#2797 Enriquecer eventos.
- hyperledger#2725 Reintroduza o tempo limite em `submit_transaction_blocking`
- hyperledger#2712 Testes de configuração.
- suporte a enum hyperledger#2491 em FFi.
- hyperledger#2775 Gere diferentes chaves na gênese sintética.
- hyperledger#2627 Finalização de configuração, ponto de entrada de proxy, kagami docgen.
- hyperledger#2765 Gere gênese sintética em `kagami`
- hyperledger#2698 Corrigir mensagem de erro pouco clara em `iroha_client`
- hyperledger#2689 Adicione parâmetros de definição de token de permissão.
- hyperledger#2502 Armazena hash GIT de compilação.
- hyperledger#2672 Adicione `ipv4Addr`, variante `ipv6Addr` e predicados.
- hyperledger#2626 Implementar derivação `Combine`, dividir macros `config`.
- hyperledger#2586 `Builder` e `LoadFromEnv` para estruturas de proxy.
- hyperledger#2611 Derive `TryFromReprC` e `IntoFfi` para estruturas opacas genéricas.
- hyperledger#2587 Divida `Configurable` em duas características. #2587: Divida `Configurable` em duas características
- hyperledger#2488 Adicionar suporte para características impls em `ffi_export`
- hyperledger#2553 Adicione classificação às consultas de ativos.
- hyperledger#2407 Gatilhos de parametrização.
- hyperledger#2536 Introduzir `ffi_import` para clientes FFI.
- hyperledger#2338 Adicione instrumentação `cargo-all-features`.
- opções de algoritmo da ferramenta hyperledger#2564 Kagami.
- hyperledger#2490 Implemente ffi_export para funções independentes.
- hyperledger#1891 Valida a execução do gatilho.
- hyperledger#1988 Derive macros para Identifiable, Eq, Hash, Ord.
- biblioteca hyperledger#2434 FFI bindgen.
- hyperledger#2073 Prefira ConstString a String para tipos em blockchain.
- hyperledger#1889 Adicione gatilhos com escopo de domínio.
- hyperledger#2098 Bloqueia consultas de cabeçalho. #2098: adicionar consultas de cabeçalho de bloco
- hyperledger#2467 Adicione o subcomando de concessão de conta em iroha_client_cli.
- hyperledger#2301 Adiciona o hash do bloco da transação ao consultá-la.
 - hyperledger#2454 Adicione um script de construção à ferramenta decodificadora Norito.
- hyperledger#2061 Derivar macro para filtros.
- hyperledger#2228 Adicionar variante não autorizada ao erro de consulta de contratos inteligentes.
- hyperledger#2395 Adicione pânico se o genesis não puder ser aplicado.
- hyperledger#2000 Proibir nomes vazios. #2000: Proibir nomes vazios
 - hyperledger#2127 Adicione verificação de integridade para garantir que todos os dados decodificados pelo codec Norito sejam consumidos.
- hyperledger#2360 Torne `genesis.json` opcional novamente.
- hyperledger#2053 Adicione testes a todas as consultas restantes no blockchain privado.
- hyperledger#2381 Unificar registro `Role`.
- hyperledger#2053 Adicione testes às consultas relacionadas a ativos em blockchain privado.
- hyperledger#2053 Adicionar testes a 'private_blockchain'
- hyperledger#2302 Adicionar consulta de stub 'FindTriggersByDomainId'.
- hyperledger#1998 Adicione filtros às consultas.
- hyperledger#2276 Inclui o hash do bloco atual em BlockHeaderValue.
- hyperledger#2161 ID de identificador e fns FFI compartilhados.
- adicionar ID de identificador e implementar equivalentes FFI de características compartilhadas (Clone, Eq, Ord)
- hyperledger#1638 `configuration` retorna a subárvore do documento.
- hyperledger#2132 Adicionar macro de proc `endpointN`.
- hyperledger#2257 Revoke<Role> emite o evento RoleRevoked.
- hyperledger#2125 Adicionar consulta FindAssetDefinitionById.
- hyperledger#1926 Adicione manipulação de sinal e desligamento normal.
- hyperledger#2161 gera funções FFI para `data_model`
- hyperledger#1149 A contagem de arquivos de bloco não excede 1.000.000 por diretório.
- hyperledger#1413 Adicionar endpoint da versão da API.
- hyperledger#2103 suporta consultas de blocos e transações. Adicionar consulta `FindAllTransactions`
- hyperledger#2186 Adicionar ISI de transferência para `BigQuantity` e `Fixed`.
- hyperledger#2056 Adicione uma caixa de macro de derivação para `AssetValueType` `enum`.
- hyperledger#2100 Adicione consulta para encontrar todas as contas com ativos.
- hyperledger#2179 Otimiza a execução do gatilho.
- hyperledger#1883 Remova arquivos de configuração incorporados.
- hyperledger#2105 lida com erros de consulta no cliente.
- hyperledger#2050 Adicione consultas relacionadas à função.
- hyperledger#1572 Tokens de permissão especializados.
- hyperledger#2121 Verifique se o par de chaves é válido quando construído.
 - hyperledger#2003 Introduzir a ferramenta decodificadora Norito.
- hyperledger#1952 Adicione um benchmark TPS como padrão para otimizações.
- hyperledger#2040 Adiciona teste de integração com limite de execução de transações.
- hyperledger#1890 Introduzir testes de integração baseados em casos de uso do Orillion.
- hyperledger#2048 Adicionar arquivo de conjunto de ferramentas.
- hyperledger#2100 Adicione consulta para encontrar todas as contas com ativos.
- hyperledger#2179 Otimiza a execução do gatilho.
- hyperledger#1883 Remova arquivos de configuração incorporados.
- hyperledger#2004 Proíbe `isize` e `usize` de se tornarem `IntoSchema`.
- hyperledger#2105 lida com erros de consulta no cliente.
- hyperledger#2050 Adicione consultas relacionadas à função.
- hyperledger#1572 Tokens de permissão especializados.
- hyperledger#2121 Verifique se o par de chaves é válido quando construído.
 - hyperledger#2003 Introduzir a ferramenta decodificadora Norito.
- hyperledger#1952 Adicione um benchmark TPS como padrão para otimizações.
- hyperledger#2040 Adiciona teste de integração com limite de execução de transações.
- hyperledger#1890 Introduzir testes de integração baseados em casos de uso do Orillion.
- hyperledger#2048 Adicionar arquivo de conjunto de ferramentas.
- hyperledger#2037 Introduzir gatilhos de pré-confirmação.
- hyperledger#1621 Introduzido por gatilhos de chamada.
- hyperledger#1970 Adicionar endpoint de esquema opcional.
- hyperledger#1620 Introduzir gatilhos baseados em tempo.
- hyperledger#1918 Implementar autenticação básica para `client`
- hyperledger#1726 Implemente um fluxo de trabalho de PR de lançamento.
- hyperledger#1815 Torne as respostas das consultas mais estruturadas por tipo.
- hyperledger#1928 implementa geração de changelog usando `gitchangelog`
- hyperledger#1902 Script de configuração bare metal de 4 pares.

  Adicionada uma versão de setup_test_env.sh que não requer docker-compose e usa a compilação de depuração de Iroha.
- hyperledger#1619 Introduzir gatilhos baseados em eventos.
- hyperledger#1195 Fecha uma conexão de websocket de forma limpa.
- hyperledger#1606 Adicionar link ipfs ao logotipo do domínio na estrutura do domínio.
- hyperledger#1754 Adicionar CLI do inspetor Kura.
- hyperledger#1790 Melhore o desempenho usando vetores baseados em pilha.
- hyperledger#1805 Cores de terminal opcionais para erros de pânico.
- hiperledger#1749 `no_std` em `data_model`
- hyperledger#1179 Adicionar instrução de revogação de permissão ou função.
- hyperledger#1782 torna iroha_crypto no_std compatível.
- hyperledger#1172 Implementar eventos de instrução.
- hyperledger#1734 Valide `Name` para excluir espaços em branco.
- hyperledger#1144 Adicionar aninhamento de metadados.
- #1210 Bloquear streaming (lado do servidor).
- hyperledger#1331 Implemente mais métricas `Prometheus`.
- hyperledger#1689 Corrigir dependências de recursos. #1261: Adicione inchaço de carga.
- hyperledger#1675 usa type em vez de estrutura wrapper para itens versionados.
- hyperledger#1643 Aguarde até que os pares comprometam o genesis nos testes.
- hiperledger#1678 `try_allocate`
- hyperledger#1216 Adicionar terminal Prometheus. #1216: implementação inicial do endpoint de métricas.
- hyperledger#1238 Atualizações em nível de log em tempo de execução. Criado recarregamento básico baseado em ponto de entrada `connection`.
- Hyperledger#1652 Formatação de título PR.
- Adicione o número de pares conectados a `Status`

  - Reverter "Excluir itens relacionados ao número de pares conectados"

  Isso reverte o commit b228b41dab3c035ce9973b6aa3b35d443c082544.
  - Esclarecer `Peer` possui chave pública verdadeira somente após handshake
  - `DisconnectPeer` sem testes
  - Implementar execução de pares de cancelamento de registro
  - Adicionar subcomando (des) registrar peer a `client_cli`
  - Recusar reconexões de um par não registrado pelo seu endereçoDepois que seu peer cancelar o registro e desconectar outro peer,
  sua rede ouvirá solicitações de reconexão do peer.
  Tudo o que você pode saber inicialmente é o endereço cujo número de porta é arbitrário.
  Então lembre-se do par não registrado pela parte diferente do número da porta
  e recusar a reconexão a partir daí
- Adicione o endpoint `/status` a uma porta específica.

### Correções- hyperledger#3129 Corrigir des/serialização `Parameter`.
- hyperledger#3109 Impedir suspensão `sumeragi` após mensagem independente de função.
- hyperledger#3046 Certifique-se de que Iroha possa iniciar normalmente vazio
  `./storage`
- hyperledger#2599 Remova fiapos do berçário.
- hyperledger#3087 Colete votos dos validadores do Conjunto B após a alteração da visualização.
- hyperledger#3056 Corrigir o travamento do benchmark `tps-dev`.
- hyperledger#1170 Implementar manipulação de soft-fork no estilo clonagem wsv.
- hyperledger#2456 Torna o bloco genesis ilimitado.
- hyperledger#3038 Reative multisigs.
- hyperledger#2894 Correção da desserialização da variável env `LOG_FILE_PATH`.
- hyperledger#2803 Retorna o código de status correto para erros de assinatura.
- hyperledger#2963 `Queue` remove transações corretamente.
- hyperledger#0000 Vergen quebrando CI.
- hyperledger#2165 Remova a inquietação do conjunto de ferramentas.
- hyperledger#2506 Corrija a validação do bloco.
- hyperledger#3013 Validadores de gravação em cadeia corretamente.
- hyperledger#2998 Exclua o código Chain não utilizado.
- hyperledger#2816 Transferir a responsabilidade de acesso aos blocos para kura.
- hyperledger#2384 Substitua decode por decode_all.
- hyperledger#1967 Substitua ValueName por Name.
- hyperledger#2980 Corrige o valor do bloco tipo ffi.
- hyperledger#2858 Introduzir parking_lot::Mutex em vez de std.
- hyperledger#2850 Corrigir desserialização/decodificação de `Fixed`
- hyperledger#2923 Retorna `FindError` quando `AssetDefinition` não
  existir.
- hiperledger#0000 Correção `panic_on_invalid_genesis.sh`
- hyperledger#2880 Feche a conexão do websocket corretamente.
- hyperledger#2880 Corrigir streaming de bloco.
- hyperledger#2804 `iroha_client_cli` envia bloqueio de transação.
- hyperledger#2819 Remover membros não essenciais do WSV.
- Corrigido bug de recursão de serialização de expressão.
- hyperledger#2834 Melhore a sintaxe abreviada.
- hyperledger#2379 Adiciona capacidade de despejar novos blocos Kura em blocks.txt.
- hyperledger#2758 Adicionar estrutura de classificação ao esquema.
- CI.
- hyperledger#2548 Avisa sobre arquivo genesis grande.
- hyperledger#2638 Atualize `whitepaper` e propague as alterações.
- hyperledger#2678 Corrige testes na ramificação de teste.
- hyperledger#2678 Corrige a interrupção dos testes no desligamento forçado do Kura.
- hyperledger#2607 Refatore o código sumeragi para maior simplicidade e
  correções de robustez.
- hyperledger#2561 Reintroduza as alterações de visualização no consenso.
- hyperledger#2560 Adicione novamente block_sync e desconexão de pares.
- hyperledger#2559 Adicionar encerramento do thread sumeragi.
- hyperledger#2558 Valide o genesis antes de atualizar o wsv do kura.
- hyperledger#2465 Reimplementar o nó sumeragi como estado singlethreaded
  máquina.
- hyperledger#2449 Implementação inicial da Reestruturação Sumeragi.
- hyperledger#2802 Corrige o carregamento do env para configuração.
- hyperledger#2787 Notifica todos os ouvintes para desligar em caso de pânico.
- hyperledger#2764 Remove o limite do tamanho máximo da mensagem.
- #2571: Melhor UX do Inspetor Kura.
- hyperledger#2703 Corrigir bugs de ambiente de desenvolvimento do Orillion.
- Corrigir erro de digitação em um comentário de documento em esquema/src.
- hyperledger#2716 Torne pública a duração do tempo de atividade.
- hyperledger#2700 Exporte `KURA_BLOCK_STORE_PATH` em imagens do docker.
- hyperledger#0 Remover `/iroha/rust-toolchain.toml` do construtor
  imagem.
- hiperledger#0 Correção `docker-compose-single.yml`
- hyperledger#2554 Gera erro se a semente `secp256k1` for menor que 32
  bytes.
- hyperledger#0 Modifique `test_env.sh` para alocar armazenamento para cada par.
- hyperledger#2457 Desligamento forçado do kura em testes.
- hyperledger#2623 Corrigir doctest para VariantCount.
- Atualize um erro esperado nos testes ui_fail.
- Corrija comentários incorretos do documento nos validadores de permissão.
- hyperledger#2422 Ocultar chaves privadas na resposta do endpoint de configuração.
- hyperledger#2492: Corrige nem todos os gatilhos executados que correspondem a um evento.
- hyperledger#2504 Corrigir benchmark de TPS com falha.
- hyperledger#2477 Corrigido bug quando as permissões das funções não eram contadas.
- hyperledger#2416 Corrija lints no braço do macOS.
- hyperledger#2457 Correção de instabilidade nos testes relacionada ao desligamento em caso de pânico.
  #2457: Adicionar desligamento na configuração de pânico
- hyperledger#2473 analisa Rustc --version em vez de RUSTUP_TOOLCHAIN.
- hyperledger#1480 Desligue em caso de pânico. #1480: Adicionar gancho de pânico para sair do programa em caso de pânico
- hyperledger#2376 Kura simplificado, sem assíncrono, dois arquivos.
- falha de compilação do hyperledger#0000 Docker.
- hyperledger#1649 remove `spawn` de `do_send`
- hyperledger#2128 Corrigir construção e iteração `MerkleTree`.
- hyperledger#2137 Prepare testes para contexto multiprocesso.
- hyperledger#2227 Implementar registro e cancelamento de registro de ativos.
- hyperledger#2081 Corrigido bug de concessão de função.
- hyperledger#2358 Adicionar versão com perfil de depuração.
- hyperledger#2294 Adicione geração de flamegraph a oneshot.rs.
- hyperledger#2202 Corrige o campo total na resposta da consulta.
- hyperledger#2081 Corrija o caso de teste para conceder a função.
- hyperledger#2017 Corrigir cancelamento de registro de função.
- hyperledger#2303 Corrigir pares do docker-compose que não são desligados normalmente.
- hyperledger#2295 Corrigido bug de gatilho de cancelamento de registro.
- hyperledger#2282 melhora o FFI derivado da implementação do getset.
- hyperledger#1149 Remova o código nocheckin.
- hyperledger#2232 Faça com que Iroha imprima uma mensagem significativa quando o genesis tiver muitos isi.
- hyperledger#2170 Correção de compilação no contêiner docker em máquinas M1.
- hyperledger#2215 Tornar nightly-2022-04-20 opcional para `cargo build`
- hyperledger#1990 Habilite a inicialização peer via env vars na ausência de config.json.
- hyperledger#2081 Corrige o registro da função.
- hyperledger#1640 Gere config.json e genesis.json.
- hyperledger#1716 Corrige falha de consenso com casos f=0.
- hyperledger#1845 Ativos não mintáveis ​​podem ser cunhados apenas uma vez.
- hyperledger#2005 Correção de `Client::listen_for_events()` que não fecha o fluxo do WebSocket.
- hyperledger#1623 Crie um RawGenesisBlockBuilder.
- hyperledger#1917 Adicionar macro easy_from_str_impl.
- hyperledger#1990 Habilite a inicialização peer via env vars na ausência de config.json.
- hyperledger#2081 Corrige o registro da função.
- hyperledger#1640 Gere config.json e genesis.json.
- hyperledger#1716 Corrige falha de consenso com casos f=0.
- hyperledger#1845 Ativos não mintáveis ​​podem ser cunhados apenas uma vez.
- hyperledger#2005 Correção de `Client::listen_for_events()` que não fecha o fluxo do WebSocket.
- hyperledger#1623 Crie um RawGenesisBlockBuilder.
- hyperledger#1917 Adicionar macro easy_from_str_impl.
- hyperledger#1922 Mova crypto_cli para ferramentas.
- hyperledger#1969 Torne o recurso `roles` parte do conjunto de recursos padrão.
- argumentos CLI do hotfix do hyperledger#2013.
- hyperledger#1897 Remove usize/isize da serialização.
- hyperledger#1955 Correção da possibilidade de passar `:` dentro de `web_login`
- hyperledger#1943 Adicione erros de consulta ao esquema.
- hyperledger#1939 Recursos adequados para `iroha_config_derive`.
- hyperledger#1908 corrige manipulação de valor zero para script de análise de telemetria.
- hyperledger#0000 Torna o doc-test implicitamente ignorado explicitamente ignorado.
- hyperledger#1848 Impede que chaves públicas sejam reduzidas a nada.
- hyperledger#1811 adicionou testes e verificações para desduplicar chaves de pares confiáveis.
- hyperledger#1821 adiciona IntoSchema para MerkleTree e VersionedValidBlock, corrige esquemas HashOf e SignatureOf.
- hyperledger#1819 Remover rastreamento do relatório de erros na validação.
- hyperledger#1774 registra o motivo exato das falhas de validação.
- hyperledger#1714 Compare PeerId apenas por chave.
- hyperledger#1788 Reduza o consumo de memória de `Value`.
- hyperledger#1804 corrige a geração de esquema para HashOf, SignatureOf, adiciona teste para garantir que nenhum esquema esteja faltando.
- Melhorias na legibilidade do registro do hyperledger#1802.
  - log de eventos movido para nível de rastreamento
  - ctx removido da captura de log
  - as cores dos terminais são opcionais (para melhor saída de log para arquivos)
- hyperledger#1783 Corrigido o benchmark torii.
- hyperledger#1772 Correção após #1764.
- hyperledger#1755 Pequenas correções para #1743, #1725.
  - Corrigir JSONs de acordo com a alteração de estrutura #1743 `Domain`
- Correções de consenso do hyperledger#1751. #1715: Correções de consenso para lidar com cargas altas (#1746)
  - Ver correções de tratamento de alterações
  - Veja provas de alteração feitas independentemente de hashes de transação específicos
  - Redução na passagem de mensagens
  - Colete votos de alteração de visualização em vez de enviar mensagens imediatamente (melhora a resiliência da rede)
  - Usar totalmente a estrutura do Actor em Sumeragi (agendar mensagens para si mesmo em vez de gerar tarefas)
  - Melhora a injeção de falhas para testes com Sumeragi
  - Aproxima o código de teste do código de produção
  - Remove wrappers complicados
  - Permite que Sumeragi use o contexto do ator no código de teste
- hyperledger#1734 Atualize o genesis para se adequar à nova validação do Domínio.
- hyperledger#1742 Erros concretos retornados nas instruções `core`.
- hyperledger#1404 Verificação corrigida.
- hyperledger#1636 Remover `trusted_peers.json` e `structopt`
  Nº 1636: Remova `trusted_peers.json`.
- Atualização hyperledger#1706 `max_faults` com atualização de topologia.
- hyperledger#1698 Corrigidas chaves públicas, documentação e mensagens de erro.
- Edições de cunhagem (1593 e 1405) edição 1405

### Refatorar- Extraia funções do loop principal do sumeragi.
- Refatore `ProofChain` para novo tipo.
- Remover `Mutex` de `Metrics`
- Remova o recurso noturno adt_const_generics.
- hyperledger#3039 Introduzir buffer de espera para multisigs.
- Simplifique sumeragi.
- hyperledger#3053 Corrija fiapos cortados.
- hyperledger#2506 Adicione mais testes na validação de bloco.
- Remova `BlockStoreTrait` em Kura.
- Atualizar lints para `nightly-2022-12-22`
- hyperledger#3022 Remover `Option` em `transaction_cache`
- hyperledger#3008 Adicione valor de nicho em `Hash`
- Atualize os lints para 1.65.
- Adicione pequenos testes para aumentar a cobertura.
- Remova o código morto de `FaultInjection`
- Ligue para p2p com menos frequência do sumeragi.
- hyperledger#2675 Valida nomes/ids de itens sem alocar Vec.
- hyperledger#2974 Impede a falsificação de bloco sem revalidação completa.
- `NonEmpty` mais eficiente em combinadores.
- hyperledger#2955 Remover bloco da mensagem BlockSigned.
- hyperledger#1868 Impedir o envio de transações validadas
  entre pares.
- hyperledger#2458 Implementar API combinada genérica.
- Adicione a pasta de armazenamento ao gitignore.
- hyperledger#2909 Portas Hardcode para o próximo.
- hyperledger#2747 Alterar API `LoadFromEnv`.
- Melhorar mensagens de erro em caso de falha de configuração.
- Adicione exemplos extras ao `genesis.json`
- Remova dependências não utilizadas antes do lançamento do `rc9`.
- Finalize o linting no novo Sumeragi.
- Extraia subprocedimentos no loop principal.
- hyperledger#2774 Altere o modo de geração de gênese `kagami` de sinalizador para
  subcomando.
- hiperledger#2478 Adicionar `SignedTransaction`
- hyperledger#2649 Remover caixa `byteorder` de `Kura`
- Renomeie `DEFAULT_BLOCK_STORE_PATH` de `./blocks` para `./storage`
- hyperledger#2650 Adicione `ThreadHandler` para desligar os submódulos iroha.
- hyperledger#2482 Armazene tokens de permissão `Account` em `Wsv`
- Adicione novos lints ao 1.62.
- Melhorar as mensagens de erro `p2p`.
- verificação de tipo estático hyperledger#2001 `EvaluatesTo`.
- hyperledger#2052 Torna os tokens de permissão registráveis ​​com definição.
  #2052: Implementar PermissionTokenDefinition
- Certifique-se de que todas as combinações de recursos funcionem.
- hyperledger#2468 Remova o supertrait de depuração dos validadores de permissão.
- hyperledger#2419 Remova `drop`s explícitos.
- hyperledger#2253 Adicionar característica `Registrable` a `data_model`
- Implemente `Origin` em vez de `Identifiable` para os eventos de dados.
- hyperledger#2369 Refatorar validadores de permissão.
- hyperledger#2307 Torne `events_sender` em `WorldStateView` não opcional.
- hyperledger#1985 Reduza o tamanho da estrutura `Name`.
- Adicione mais `const fn`.
- Faça testes de integração usando `default_permissions()`
- adicione wrappers de token de permissão em private_blockchain.
- hyperledger#2292 Remover `WorldTrait`, remover genéricos de `IsAllowedBoxed`
- hyperledger#2204 Torne genéricas as operações relacionadas a ativos.
- hyperledger#2233 Substitua `impl` por `derive` para `Display` e `Debug`.
- Melhorias estruturais identificáveis.
- hyperledger#2323 Aprimorar mensagem de erro de inicialização do kura.
- hyperledger#2238 Adicionar construtor de pares para testes.
- hyperledger#2011 Parâmetros de configuração mais descritivos.
- hyperledger#1896 Simplifique a implementação do `produce_event`.
- Refatore em torno de `QueryError`.
- Mova `TriggerSet` para `data_model`.
- hyperledger#2145 refatora o lado `WebSocket` do cliente, extrai lógica de dados pura.
- remova a característica `ValueMarker`.
- hyperledger#2149 Exponha `Mintable` e `MintabilityError` em `prelude`
- hyperledger#2144 redesenha o fluxo de trabalho http do cliente, expõe a API interna.
- Mude para `clap`.
- Criar binário `iroha_gen`, consolidando documentos, esquema_bin.
- hyperledger#2109 Torne o teste `integration::events::pipeline` estável.
- hyperledger#1982 encapsula o acesso às estruturas `iroha_crypto`.
- Adicionar construtor `AssetDefinition`.
- Remova `&mut` desnecessário da API.
- encapsular o acesso às estruturas do modelo de dados.
- hyperledger#2144 redesenha o fluxo de trabalho http do cliente, expõe a API interna.
- Mude para `clap`.
- Criar binário `iroha_gen`, consolidando documentos, esquema_bin.
- hyperledger#2109 Torne o teste `integration::events::pipeline` estável.
- hyperledger#1982 encapsula o acesso às estruturas `iroha_crypto`.
- Adicionar construtor `AssetDefinition`.
- Remova `&mut` desnecessário da API.
- encapsular o acesso às estruturas do modelo de dados.
- Núcleo, `sumeragi`, funções de instância, `torii`
- hyperledger#1903 move a emissão do evento para métodos `modify_*`.
- Divida o arquivo `data_model` lib.rs.
- Adicione referência wsv à fila.
- hyperledger#1210 Fluxo de eventos dividido.
  - Mova a funcionalidade relacionada à transação para o módulo data_model/transaction
- hyperledger#1725 Remove o estado global em Torii.
  - Implemente `add_state macro_rules` e remova `ToriiState`
- Corrigir erro de linter.
- limpeza do hyperledger#1661 `Cargo.toml`.
  - Classificar dependências de carga
- hyperledger#1650 arrumar `data_model`
  - Mover o mundo para wsv, corrigir recurso de funções, derivar IntoSchema para CommittedBlock
- Organização de arquivos `json` e leia-me. Atualize o Leiame para estar em conformidade com o modelo.
- 1529: registro estruturado.
  - Refatorar mensagens de log
-`iroha_p2p`
  - Adicionar privatização p2p.

### Documentação

- Atualize o leia-me da CLI do cliente Iroha.
- Atualizar trechos do tutorial.
- Adicione 'sort_by_metadata_key' às especificações da API.
- Atualizar links para documentação.
- Estenda o tutorial com documentos relacionados a ativos.
- Remova arquivos de documentos desatualizados.
- Revise a pontuação.
- Mova alguns documentos para o repositório do tutorial.
- Relatório de instabilidade para branch de teste.
- Gerar changelog para pré-rc.7.
- Relatório de instabilidade de 30 de julho.
- Versões de colisão.
- Atualizar a instabilidade do teste.
- hyperledger#2499 Corrige mensagens de erro client_cli.
- hyperledger#2344 Gera CHANGELOG para 2.0.0-pre-rc.5-lts.
- Adicione links ao tutorial.
- Atualizar informações sobre ganchos git.
- redação do teste de instabilidade.
- hyperledger#2193 Atualização da documentação do cliente Iroha.
- hyperledger#2193 Atualização da documentação CLI Iroha.
- hyperledger#2193 Atualização README para macro crate.
 - hyperledger#2193 Atualização da documentação da ferramenta decodificadora Norito.
- hyperledger#2193 Atualização da documentação Kagami.
- hyperledger#2193 Atualizar documentação de benchmarks.
- hyperledger#2192 Revise as diretrizes de contribuição.
- Corrija referências quebradas no código.
- métricas do documento hyperledger#1280 Iroha.
- hyperledger#2119 Adicione orientação sobre como recarregar a quente Iroha em um contêiner Docker.
- hyperledger#2181 Revisão README.
- hyperledger#2113 Recursos do documento em arquivos Cargo.toml.
- hyperledger#2177 Limpe a saída do gitchangelog.
- hyperledger#1991 Adicione o leia-me ao inspetor Kura.
- hyperledger#2119 Adicione orientação sobre como recarregar a quente Iroha em um contêiner Docker.
- hyperledger#2181 Revisão README.
- hyperledger#2113 Recursos do documento em arquivos Cargo.toml.
- hyperledger#2177 Limpe a saída do gitchangelog.
- hyperledger#1991 Adicione o leia-me ao inspetor Kura.
- gerar o último changelog.
- Gerar log de alterações.
- Atualize arquivos README desatualizados.
- Adicionados documentos ausentes ao `api_spec.md`.

### Alterações de CI/CD- Adicione mais cinco executores auto-hospedados.
- Adicionar tag de imagem regular para registro Soramitsu.
- Solução alternativa para libgit2-sys 0.5.0. Reverta para 0.4.4.
- Tente usar imagem baseada em arco.
- Atualizar fluxos de trabalho para trabalhar em novos contêineres somente noturnos.
- Remova pontos de entrada binários da cobertura.
- Mude os testes de desenvolvimento para executores auto-hospedados da Equinix.
- hyperledger#2865 Remova o uso do arquivo tmp de `scripts/check.sh`
- hyperledger#2781 Adicione compensações de cobertura.
- Desative testes de integração lenta.
- Substitua a imagem base pelo cache do docker.
- hyperledger#2781 Adicionar recurso pai de commit do codecov.
- Mova trabalhos para executores do GitHub.
- hyperledger#2778 Verificação de configuração do cliente.
- hyperledger#2732 Adicione condições para atualizar imagens base iroha2 e adicione
  Etiquetas de relações públicas.
- Corrija a construção noturna da imagem.
- Corrigir erro `buildx` com `docker/build-push-action`
- Primeiros socorros para `tj-actions/changed-files` não funcional
- Habilitar publicação sequencial de imagens, após #2662.
- Adicionar registro do porto.
- Etiqueta automática `api-changes` e `config-changes`
- Confirmar hash na imagem, arquivo do conjunto de ferramentas novamente, isolamento da interface do usuário,
  rastreamento de esquema.
- Torne os fluxos de trabalho de publicação sequenciais e complemente o #2427.
- hyperledger#2309: Reative os testes de documentos no CI.
- hyperledger#2165 Remover instalação do codecov.
- Mude para um novo contêiner para evitar conflitos com os usuários atuais.
 - atualização do hyperledger#2158 `parity_scale_codec` e outras dependências. (Codec Norito)
- Corrigir compilação.
- hyperledger#2461 Melhore o CI iroha2.
- Atualização `syn`.
- mover a cobertura para um novo fluxo de trabalho.
- versão de login reverso do docker.
- Remova a especificação da versão de `archlinux:base-devel`
- Atualizar Dockerfiles e relatórios Codecov, reutilização e simultaneidade.
- Gerar log de alterações.
- Adicionar arquivo `cargo deny`.
- Adicionar ramificação `iroha2-lts` com fluxo de trabalho copiado de `iroha2`
- hyperledger#2393 Aumente a versão da imagem base Docker.
- hyperledger#1658 Adicionar verificação de documentação.
- Versão de caixas e remoção de dependências não utilizadas.
- Remova relatórios de cobertura desnecessários.
- hyperledger#2222 Divida os testes para determinar se envolve cobertura ou não.
- hiperledger#2153 Correção #2154.
- A versão bate em todas as caixas.
- Correção do pipeline de implantação.
- hyperledger#2153 Correção de cobertura.
- Adicionar verificação de gênese e atualizar documentação.
- Ferrugem, mofo e noturno para 1,60, 1.2.0 e 1,62, respectivamente.
- gatilhos load-rs.
- hiperledger#2153 Correção #2154.
- A versão bate em todas as caixas.
- Correção do pipeline de implantação.
- hyperledger#2153 Correção de cobertura.
- Adicionar verificação de gênese e atualizar documentação.
- Ferrugem, mofo e noturno para 1,60, 1.2.0 e 1,62, respectivamente.
- gatilhos load-rs.
- load-rs:libere gatilhos de fluxo de trabalho.
- Corrigir fluxo de trabalho push.
- Adicione telemetria aos recursos padrão.
- adicione a tag adequada para enviar o fluxo de trabalho para o principal.
- corrigir testes com falha.
- hyperledger#1657 Atualize a imagem para ferrugem 1.57. #1630: Volte para executores auto-hospedados.
- Melhorias no IC.
- Cobertura comutada para usar `lld`.
- Correção de dependência de CI.
- Melhorias na segmentação de CI.
- Usa uma versão fixa do Rust no CI.
- Correção de publicação Docker e CI push iroha2-dev. Mova a cobertura e a bancada para relações públicas
- Remova a compilação Iroha completa desnecessária no teste do docker CI.

  A compilação Iroha tornou-se inútil, pois agora é feita na própria imagem do docker. Portanto, o CI apenas constrói o CLI do cliente que é usado nos testes.
- Adicionar suporte para branch iroha2 no pipeline de CI.
  - testes longos só rodaram em PR em iroha2
  - publicar imagens docker apenas do iroha2
- Caches de CI adicionais.

### Web-Assembly


### Problemas de versão

- Versão pré-rc.13.
- Versão anterior ao rc.11.
- Versão para RC.9.
- Versão para RC.8.
- Atualizar versões para RC7.
- Preparativos de pré-lançamento.
- Atualizar Molde 1.0.
- Dependências de colisão.
- Atualizar api_spec.md: corrigir corpos de solicitação/resposta.
- Atualize a versão ferrugem para 1.56.0.
- Atualizar guia de contribuição.
- Atualize README.md e `iroha/config.json` para corresponder ao novo formato de API e URL.
- Atualize o destino de publicação do docker para hyperledger/iroha2 #1453.
- Atualiza o fluxo de trabalho para que corresponda ao principal.
- Atualize as especificações da API e corrija o endpoint de integridade.
- Atualização de ferrugem para 1.54.
- Docs (iroha_crypto): atualize os documentos `Signature` e alinhe os argumentos de `verify`
- Versão Ursa aumentada de 0.3.5 para 0.3.6.
- Atualize fluxos de trabalho para novos executores.
- Atualize o dockerfile para armazenamento em cache e compilações ci mais rápidas.
- Atualize a versão libssl.
- Atualize dockerfiles e async-std.
- Corrigir clippy atualizado.
- Atualiza a estrutura de ativos.
  - Suporte para instruções de valor-chave no ativo
  - Tipos de ativos como um enum
  - Vulnerabilidade de estouro na correção de ISI de ativos
- Guia de contribuição de atualizações.
- Atualizar lib desatualizada.
- Atualize o whitepaper e corrija problemas de linting.
- Atualize a biblioteca pepino_rust.
- Atualizações README para geração de chaves.
- Atualizar fluxos de trabalho do Github Actions.
- Atualizar fluxos de trabalho do Github Actions.
- Atualizar requisitos.txt.
- Atualize common.yaml.
- Atualizações de documentos de Sara.
- Atualizar lógica de instrução.
- Atualize o white paper.
- Atualiza a descrição das funções de rede.
- Atualize o whitepaper com base em comentários.
- Separação de atualização WSV e migração para Scale.
- Atualize o gitignore.
- Atualize ligeiramente a descrição do kura no WP.
- Atualizar a descrição sobre kura no whitepaper.

### Esquema

- hyperledger#2114 Suporte a coleções classificadas em esquemas.
- hyperledger#2108 Adicionar paginação.
- hyperledger#2114 Suporte a coleções classificadas em esquemas.
- hyperledger#2108 Adicionar paginação.
- Torne compatível o esquema, a versão e a macro no_std.
- Corrigir assinaturas no esquema.
- Representação alterada de `FixedPoint` no esquema.
- Adicionado `RawGenesisBlock` à introspecção de esquema.
- Alterados modelos de objetos para criar o esquema IR-115.

### Testes

- hyperledger#2544 Tutorial doctests.
- hyperledger#2272 Adicione testes para a consulta 'FindAssetDefinitionById'.
- Adicionar testes de integração `roles`.
- Padronize o formato dos testes de interface do usuário, mova os testes de derivação da interface do usuário para derivar caixas.
- Corrigir testes simulados (bug não ordenado de futuros).
- Removida a caixa DSL e movido os testes para `data_model`
- Certifique-se de que os testes de rede instáveis sejam aprovados para código válido.
- Adicionados testes ao iroha_p2p.
- Captura logs em testes, a menos que o teste falhe.
- Adicione pesquisas para testes e corrija testes que raramente quebram.
- Testa a configuração paralela.
- Remova o root dos testes iroha init e iroha_client.
- Corrige testes de avisos clippy e adiciona verificações ao ci.
- Corrigir erros de validação `tx` durante testes de benchmark.
- hyperledger#860: Iroha Consultas e testes.
- Guia ISI personalizado Iroha e testes de pepino.
- Adicionar testes para cliente sem padrão.
- Alterações e testes de registro de ponte.
- Testes de consenso com simulação de rede.
- Utilização de diretório temporário para execução de testes.
- Bancadas testa casos positivos.
- Funcionalidade inicial do Merkle Tree com testes.
- Corrigidos testes e inicialização do World State View.

### Outro- Mova a parametrização para características e remova os tipos FFI IR.
- Adicionar suporte para sindicatos, introduzir `non_robust_ref_mut` * implementar conversão conststring FFI.
- Melhorar IdOrdEqHash.
- Remova FilterOpt::BySome da (des)serialização.
- Faça Não transparente.
- Torne o ContextValue transparente.
- Tornar a tag Expression::Raw opcional.
- Adicione transparência para algumas instruções.
- Melhorar a (des)serialização do RoleId.
- Melhorar a (des)serialização do validator::Id.
- Melhorar a (des)serialização de PermissionTokenId.
- Melhorar a (des)serialização do TriggerId.
- Melhorar a (des) serialização de IDs de ativos (definição).
- Melhorar a (des)serialização do AccountId.
- Melhorar a (des)serialização de Ipfs e DomainId.
- Remova a configuração do logger da configuração do cliente.
- Adicione suporte para estruturas transparentes em FFI.
- Refatorar &Option<T> para Option<&T>
- Corrigir avisos clippy.
- Adicione mais detalhes na descrição do erro `Find`.
- Corrigir implementações `PartialOrd` e `Ord`.
- Use `rustfmt` em vez de `cargo fmt`
- Remova o recurso `roles`.
- Use `rustfmt` em vez de `cargo fmt`
- Compartilhe workdir como um volume com instâncias do dev docker.
- Remova o tipo associado ao Diff em Executar.
- Use codificação personalizada em vez de retorno multival.
- Remova serde_json como dependência iroha_crypto.
- Permitir apenas campos conhecidos no atributo de versão.
- Esclareça diferentes portas para endpoints.
- Remova a derivação `Io`.
- Documentação inicial de key_pairs.
- Volte para executores auto-hospedados.
- Corrija novos lints no código.
- Remova i1i1 dos mantenedores.
- Adicionar documento do ator e pequenas correções.
- Faça uma enquete em vez de enviar os blocos mais recentes.
- Eventos de status de transação testados para cada um dos 7 pares.
- `FuturesUnordered` em vez de `join_all`
- Mude para GitHub Runners.
- Use VersionedQueryResult vs QueryResult para /query endpoint.
- Reconecte a telemetria.
- Corrija a configuração do dependebot.
- Adicione commit-msg git hook para incluir aprovação.
- Corrija o pipeline de envio.
- Atualize o dependebot.
- Detectar carimbo de data/hora futuro no push da fila.
- hyperledger#1197: Kura lida com erros.
- Adicionar instrução de cancelamento de registro de pares.
- Adicione nonce opcional para distinguir transações. Feche #1493.
- Removido `sudo` desnecessário.
- Metadados para domínios.
- Corrija as rejeições aleatórias no fluxo de trabalho `create-docker`.
- Adicionado `buildx` conforme sugerido pelo pipeline com falha.
- hyperledger#1454: corrige resposta de erro de consulta com código de status e dicas específicas.
- hyperledger#1533: Encontre transação por hash.
- Correção do ponto de extremidade `configure`.
- Adicionar verificação de capacidade de cunhagem de ativos baseada em booleano.
- Adição de criptografias primitivas digitadas e migração para criptografia de tipo seguro.
- Melhorias no registro.
- hyperledger#1458: Adicione o tamanho do canal do ator à configuração como `mailbox`.
- hyperledger#1451: Adicionar aviso sobre configuração incorreta se `faulty_peers = 0` e `trusted peers count > 1`
- Adicionar manipulador para obter hash de bloco específico.
- Adicionada nova consulta FindTransactionByHash.
- hyperledger#1185: Altere o nome e o caminho das caixas.
- Corrigir logs e melhorias gerais.
- hyperledger#1150: Agrupe 1000 blocos em cada arquivo
- Teste de estresse de fila.
- Correção no nível do log.
- Adicione especificação de cabeçalho à biblioteca do cliente.
- Correção de falha de pânico na fila.
- Fila de correção.
- Correção da versão do dockerfile.
- Correção do cliente HTTPS.
- Aceleração ci.
- 1. Removidas todas as dependências da ursa, exceto iroha_crypto.
- Corrija o estouro ao subtrair durações.
- Tornar os campos públicos no cliente.
- Envie Iroha2 para Dockerhub todas as noites.
- Corrigir códigos de status http.
- Substitua iroha_error por thiserror, eyre e color-eyre.
- Substitua a fila pela trave transversal.
- Remova algumas sobras de fiapos inúteis.
- Introduz metadados para definições de ativos.
- Remoção de argumentos da caixa test_network.
- Remova dependências desnecessárias.
- Correção de iroha_client_cli::events.
- hyperledger#1382: Remova a implementação de rede antiga.
- hyperledger#1169: Adicionada precisão para ativos.
- Melhorias no arranque entre pares:
  - Permite carregar a chave pública do genesis apenas do env
  - config, genesis e caminho de trust_peers agora podem ser especificados em parâmetros cli
- hyperledger#1134: Integração de Iroha P2P.
- Altere o endpoint da consulta para POST em vez de GET.
- Execute on_start no ator de forma síncrona.
- Migrar para warp.
- Retrabalho do commit com correções de bugs do corretor.
- Reverter o commit "Introduz múltiplas correções de corretor" (9c148c33826067585b5868d297dcdd17c0efe246)
- Apresenta várias correções de corretor:
  - Cancelar inscrição do corretor na parada do ator
  - Suporte a múltiplas assinaturas do mesmo tipo de ator (anteriormente um TODO)
  - Corrigido um bug onde o corretor sempre colocava self como um ID de ator.
- Bug do corretor (mostra de teste).
- Adicione derivações para modelo de dados.
- Remova o rwlock do torii.
- Verificações de permissão de consulta OOB.
- hyperledger#1272: Implementação de contagens de pares,
- Verificação recursiva de permissões de consulta dentro das instruções.
- Agendar atores de parada.
- hyperledger#1165: Implementação de contagens de pares.
- Verifique as permissões de consulta por conta no endpoint torii.
- Removida a exposição do uso de CPU e memória nas métricas do sistema.
 - Substitua JSON por Norito para mensagens WS.
- Armazene alterações de prova de visualização.
- hyperledger#1168: Adicionado registro se a transação não passar na condição de verificação de assinatura.
- Corrigidos pequenos problemas, adicionado código de escuta de conexão.
- Introduzir o construtor de topologia de rede.
- Implementar rede P2P para Iroha.
- Adiciona métrica de tamanho de bloco.
- Característica PermissionValidator renomeada para IsAllowed. e outras alterações de nome correspondentes
- Correções de soquete da web com especificações de API.
- Remove dependências desnecessárias da imagem do Docker.
- Fmt usa Crate import_granularity.
- Apresenta o validador de permissão genérica.
- Migrar para framework de ator.
- Alterar o design do corretor e adicionar algumas funcionalidades aos atores.
- Configura verificações de status do codecov.
- Usa cobertura baseada em fonte com grcov.
- Corrigido o formato de vários build-args e ARG redeclarado para contêineres de build intermediários.
- Apresenta a mensagem SubscriptionAccepted.
- Remova ativos de valor zero das contas após a operação.
- Corrigido o formato dos argumentos de construção do docker.
- Corrigida mensagem de erro caso o bloco filho não fosse encontrado.
- Adicionado OpenSSL de fornecedor para construção, corrige a dependência do pkg-config.
- Corrija o nome do repositório para dockerhub e diferença de cobertura.
- Adicionado texto de erro e nome de arquivo claros se TrustedPeers não puder ser carregado.
- Alteradas entidades de texto para links em documentos.
- Corrija o segredo de nome de usuário errado na publicação Docker.
- Corrija pequenos erros de digitação no white paper.
- Permite o uso de mod.rs para melhor estrutura de arquivos.
- Mova main.rs para uma caixa separada e conceda permissões para blockchain público.
- Adicionar consulta dentro do cli do cliente.
- Migrar de clap para structopts para cli.
- Limitar a telemetria ao teste de rede instável.
- Mova características para o módulo smartcontracts.
- Sed -i "s/world_state_view/wsv/g"
- Mova contratos inteligentes para um módulo separado.
- Correção de bug de comprimento de conteúdo de rede Iroha.
- Adiciona armazenamento local de tarefas para o ID do ator. Útil para detecção de deadlock.
- Adicionar teste de detecção de deadlock ao CI
- Adicionar macro Introspecção.
- Desambigua nomes de fluxo de trabalho e também correções de formatação
- Mudança de API de consulta.
- Migração de async-std para tokio.
- Adicionar análise de telemetria ao ci.
- Adicionar telemetria de futuros para iroha.
- Adicione futuros iroha a todas as funções assíncronas.
- Adicione futuros de iroha para observabilidade do número de pesquisas.
- Implantação e configuração manuais adicionadas ao README.
- Correção do repórter.
- Adicionar macro de mensagem derivada.
- Adicione uma estrutura de ator simples.
- Adicionar configuração do dependebot.
- Adicione bons repórteres de pânico e erros.
- Migração da versão Rust para 1.52.1 e correções correspondentes.
- Gerar bloqueio de tarefas intensivas de CPU em threads separados.
- Use unique_port e cargo-lints de crates.io.
- Correção para WSV sem bloqueio:
  - remove Dashmaps e bloqueios desnecessários na API
  - corrige bug com número excessivo de blocos criados (transações rejeitadas não eram registradas)
  - Exibe a causa completa do erro
- Adicionar assinante de telemetria.
- Consultas de funções e permissões.
- Mova blocos de kura para wsv.
- Mudança para estruturas de dados sem bloqueio dentro do wsv.
- Correção de tempo limite da rede.
- Endpoint de integridade de correção.
- Apresenta funções.
- Adicione imagens push docker do branch dev.
- Adicione linting mais agressivo e remova pânico do código.
- Retrabalho do traço Execute para instruções.
- Remova o código antigo de iroha_config.
- IR-1060 Adiciona verificações de concessão para todas as permissões existentes.
- Correção de ulimit e tempo limite para iroha_network.
- Correção de teste de tempo limite de Ci.
- Remova todos os ativos quando sua definição for removida.
- Corrija o pânico do wsv ao adicionar ativos.
- Remova Arc e Rwlock para canais.
- Correção de rede Iroha.
- Validadores de permissão usam referências em verificações.
- Conceder Instrução.
- Adicionada configuração para limites de comprimento de string e validação de id's para NewAccount, Domain e AssetDefinition IR-1036.
- Substitua o log pela biblioteca de rastreamento.
- Adicionar verificação de documentos e negar macro dbg.
- Introduz permissões concedíveis.
- Adicione a caixa iroha_config.
- Adicione @alerdenisov como proprietário do código para aprovar todas as solicitações de mesclagem recebidas.
- Correção da verificação do tamanho da transação durante o consenso.
- Reverter a atualização do async-std.
- Substitua algumas consts pela potência de 2 IR-1035.
- Adicionar consulta para recuperar o histórico de transações IR-1024.- Adicionar validação de permissões para armazenamento e reestruturação de validadores de permissões.
- Adicione NewAccount para registro de conta.
- Adicione tipos para definição de ativos.
- Introduz limites de metadados configuráveis.
- Introduz metadados de transação.
- Adicione expressões dentro de consultas.
- Adicione lints.toml e corrija avisos.
- Separe os trust_peers do config.json.
- Correção de erro de digitação na URL da comunidade Iroha 2 no Telegram.
- Corrigir avisos clippy.
- Introduz suporte a metadados de valor-chave para conta.
- Adicionar versionamento de blocos.
- Fixup ci linting repetições.
- Adicione expressões mul,div,mod,raise_to.
- Adicione into_v* para versionamento.
- Substitua Error::msg pela macro de erro.
- Reescrever iroha_http_server e retrabalhar os erros do torii.
 - Atualiza a versão Norito para 2.
- Descrição do versionamento do whitepaper.
- Paginação infalível. Corrija os casos em que a paginação pode ser desnecessária devido a erros, e não retornar coleções vazias.
- Adicionar derive(Error) para enumerações.
- Corrigir versão noturna.
- Adicione a caixa iroha_error.
- Mensagens versionadas.
- Introduz primitivos de versionamento de contêiner.
- Corrigir benchmarks.
- Adicione paginação.
- Adicionar decodificação de codificação varint.
- Altere o carimbo de data/hora da consulta para u128.
- Adicionar enum RejectionReason para eventos de pipeline.
- Remove linhas desatualizadas dos arquivos genesis. O destino foi removido do registro ISI em commits anteriores.
- Simplifica o registro e cancelamento de registro de ISIs.
- Corrigido o tempo limite de commit não sendo enviado na rede de 4 pares.
- Embaralhamento da topologia na visualização de alteração.
- Adicione outros contêineres para a macro derivada FromVariant.
- Adicionar suporte MST para cliente cli.
- Adicionar macro FromVariant e base de código de limpeza.
- Adicione i1i1 aos proprietários do código.
- Transações de fofoca.
- Adicione comprimento para instruções e expressões.
- Adicione documentos para bloquear parâmetros de tempo e tempo de confirmação.
- Características de verificação e aceitação substituídas por TryFrom.
- Introduzir a espera apenas pelo número mínimo de pares.
- Adicione ação do github para testar a API com iroha2-java.
- Adicione gênese para docker-compose-single.yml.
- Condição de verificação de assinatura padrão para conta.
- Adicionar teste para conta com vários signatários.
- Adicionar suporte de API de cliente para MST.
- Construir na janela de encaixe.
- Adicione gênese ao docker compose.
- Introduzir MST condicional.
- Adicionar impl. wait_for_active_peers.
- Adicionar teste para cliente isahc em iroha_http_server.
- Especificação da API do cliente.
- Execução de consultas em Expressões.
- Integra expressões e ISIs.
- Expressões para ISI.
- Corrigir benchmarks de configuração de conta.
- Adicionar configuração de conta para cliente.
- Correção `submit_blocking`.
- Eventos de pipeline são enviados.
- Conexão de soquete web do cliente Iroha.
- Separação de eventos para pipeline e eventos de dados.
- Teste de integração para permissões.
- Adicione verificações de permissão para burn e mint.
- Cancelar registro da permissão ISI.
- Corrigir benchmarks para estrutura PR mundial.
- Introduzir a estrutura mundial.
- Implementar o componente de carregamento do bloco genesis.
- Apresente o relato do Gênesis.
- Introduzir o construtor validador de permissões.
- Adicione rótulos aos PRs Iroha2 com ações do Github.
- Introduzir estrutura de permissões.
- Limite de número de tx tx da fila e correções de inicialização Iroha.
- Envolva Hash em uma estrutura.
- Melhorar o nível de registro:
  - Adicione registros de nível de informação ao consenso.
  - Marcar os logs de comunicação de rede como nível de rastreamento.
  - Remova o vetor de bloco do WSV, pois é uma duplicação e mostrava todo o blockchain nos logs.
  - Defina o nível do log de informações como padrão.
- Remova referências WSV mutáveis ​​para validação.
- Incremento da versão Heim.
- Adicione peers confiáveis ​​padrão à configuração.
- Migração da API do cliente para http.
- Adicione transferência isi à CLI.
- Configuração de instruções relacionadas ao peer Iroha.
- Implementação de métodos e testes de execução ISI ausentes.
- Análise de parâmetros de consulta de URL
- Adicionar `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)`
- Substituição de antigos modelos de Instrução e Consulta pela abordagem DSL Iroha.
- Adicionar suporte a assinaturas BLS.
- Introduzir a caixa do servidor http.
- libssl.so.1.0.0 corrigido com link simbólico.
- Verifica a assinatura da conta para transação.
- Refatorar etapas da transação.
- Melhorias iniciais nos domínios.
- Implementar protótipo DSL.
- Melhorar os benchmarks Torii: desabilitar o registro em benchmarks, adicionar afirmação de taxa de sucesso.
- Melhorar o pipeline de cobertura de teste: substitui `tarpaulin` por `grcov`, publica relatório de cobertura de teste para `codecov.io`.
- Corrigir tema RTD.
- Artefatos de entrega para subprojetos iroha.
- Introduzir `SignedQueryRequest`.
- Corrija um bug com verificação de assinatura.
- Suporte a transações de reversão.
- Imprima o par de chaves gerado como json.
- Suporte ao par de chaves `Secp256k1`.
- Suporte inicial para diferentes algoritmos criptográficos.
- Recursos DEX.
- Substitua o caminho de configuração codificado pelo parâmetro cli.
- Correção do fluxo de trabalho mestre do banco.
- Teste de conexão do evento Docker.
- Guia do monitor Iroha e CLI.
- Melhorias no cli de eventos.
- Filtro de eventos.
- Conexões de eventos.
- Correção no fluxo de trabalho mestre.
- Rtd para iroha2.
- Hash raiz da árvore Merkle para transações em bloco.
- Publicação no docker hub.
- Funcionalidade CLI para Maintenance Connect.
- Funcionalidade CLI para Maintenance Connect.
- Eprintln para registrar macro.
- Melhorias de registro.
- Assinatura IR-802 para bloquear alterações de status.
- Envio de eventos de transações e blocos.
- Move o tratamento de mensagens Sumeragi para mensagem impl.
- Mecanismo Geral de Conexão.
- Extraia entidades de domínio Iroha para cliente sem padrão.
- Transações TTL.
- Máximo de transações por configuração de bloco.
- Armazene hashes de blocos invalidados.
- Sincronize blocos em lotes.
- Configuração da funcionalidade de conexão.
- Conecte-se à funcionalidade Iroha.
- Correções de validação de bloco.
- Sincronização de blocos: diagramas.
- Conecte-se à funcionalidade Iroha.
- Bridge: remove clientes.
- Sincronização de blocos.
- AdicionarPeer ISI.
- Comandos para renomeação de instruções.
- Ponto final de métricas simples.
- Bridge: obtenha pontes registradas e ativos externos.
- Teste de composição Docker no pipeline.
- Teste Sumeragi de votos insuficientes.
- Encadeamento de blocos.
- Bridge: manipulação manual de transferências externas.
- Endpoint de manutenção simples.
- Migração para serde-json.
- Demint ISI.
- Adicione clientes de ponte, AddSignatory ISI e permissão CanAddSignatory.
- Sumeragi: peers no conjunto b de correções TODO relacionadas.
- Valida o bloco antes de assinar Sumeragi.
- Ponte de ativos externos.
- Validação de assinatura em mensagens Sumeragi.
- Armazenamento de ativos binários.
- Substitua o alias PublicKey por tipo.
- Prepare caixas para publicação.
- Lógica de votos mínimos dentro do NetworkTopology.
- Refatoração de validação de TransactionReceipt.
- Alteração do gatilho OnWorldStateViewChange: IrohaQuery em vez de Instrução.
- Construção separada da inicialização em NetworkTopology.
- Adicionar instruções especiais Iroha relacionadas aos eventos Iroha.
- Manipulação do tempo limite de criação de blocos.
- Glossário e como adicionar documentos do módulo Iroha.
- Substitua o modelo de ponte codificado pelo modelo de origem Iroha.
- Introduzir estrutura NetworkTopology.
- Adicionar entidade de permissão com transformação de instruções.
- Sumeragi Mensagens no módulo de mensagens.
- Funcionalidade Genesis Block para Kura.
- Adicione arquivos README para caixas Iroha.
- Bridge e RegisterBridge ISI.
- O trabalho inicial com Iroha altera os ouvintes.
- Injeção de verificações de permissão no OOB ISI.
- Correção de vários pares Docker.
- Exemplo de docker ponto a ponto.
- Tratamento de recibos de transações.
- Permissões Iroha.
- Módulo para Dex e caixas para Bridges.
- Correção de teste de integração com criação de ativos com diversos peers.
- Reimplementação do modelo Asset no EC-S-.
- Confirmar tratamento de tempo limite.
- Cabeçalho do bloco.
- Métodos relacionados ao ISI para entidades de domínio.
- Enumeração do modo Kura e configuração de pares confiáveis.
- Regra de linting de documentação.
- Adicionar ComprometidoBlock.
- Desacoplando kura de `sumeragi`.
- Verifique se as transações não estão vazias antes da criação do bloco.
- Reimplementar as Instruções Especiais Iroha.
- Benchmarks para transações e transições de blocos.
- Ciclo de vida das transações e estados reformulados.
- Bloqueia o ciclo de vida e os estados.
- Corrigido bug de validação, ciclo de loop `sumeragi` sincronizado com o parâmetro de configuração block_build_time_ms.
- Encapsulamento do algoritmo Sumeragi dentro do módulo `sumeragi`.
- Módulo mocking para caixa de rede Iroha implementado via canais.
- Migração para API async-std.
- Recurso de simulação de rede.
- Limpeza de código assíncrono relacionado.
- Otimizações de desempenho no loop de processamento de transações.
- A geração de pares de chaves foi extraída do início Iroha.
- Pacote Docker do executável Iroha.
- Introduzir o cenário básico Sumeragi.
- Cliente CLI Iroha.
- Queda de iroha após execução do grupo de bancada.
-Integre `sumeragi`.
- Alterar a implementação de `sort_peers` para rand shuffle propagado com hash de bloco anterior.
- Remova o wrapper de mensagem no módulo peer.
- Encapsular informações relacionadas à rede dentro de `torii::uri` e `iroha_network`.
- Adicionar instrução Peer implementada em vez de manipulação de código rígido.
- Comunicação entre pares por meio de lista de pares confiáveis.
- Encapsulamento de tratamento de solicitações de rede dentro de Torii.
- Encapsulamento de lógica criptográfica dentro do módulo criptográfico.- Sinal de bloco com carimbo de data e hora e hash do bloco anterior como carga útil.
- Funções criptográficas colocadas na parte superior do módulo e funcionam com o assinante ursa encapsulado na assinatura.
- Sumeragi inicial.
- Validação de instruções de transação no clone da visão do estado mundial antes de confirmar para armazenar.
- Verifique as assinaturas na aceitação da transação.
- Correção de bug na desserialização de solicitação.
- Implementação da assinatura Iroha.
- A entidade Blockchain foi removida para limpar a base de código.
- Mudanças na API de Transações: melhor criação e trabalho com solicitações.
- Corrigido bug que criava blocos com vetor de transação vazio
- Encaminhar transações pendentes.
 - Corrigido bug com byte ausente no pacote TCP codificado u128 Norito.
- Atribuir macros para rastreamento de métodos.
- Módulo P2p.
- Utilização de iroha_network no torii e cliente.
- Adicione novas informações ISI.
- Alias ​​de tipo específico para estado da rede.
- Box<dyn Error> substituído por String.
- Escuta de rede com estado.
- Lógica de validação inicial para transações.
- Caixa Iroha_network.
- Derivar macro para características Io, IntoContract e IntoQuery.
- Implementação de consultas para cliente Iroha.
- Transformação de Comandos em contratos ISI.
- Adicionar design proposto para multisig condicional.
- Migração para espaços de trabalho Cargo.
- Migração de módulos.
- Configuração externa via variáveis ​​de ambiente.
- Tratamento de solicitações Get e Put para Torii.
- Correção do Github ci.
- Cargo-make limpa blocos após teste.
- Introduzir o módulo `test_helper_fns` com função de limpeza de diretório com blocos.
- Implementar validação via merkle tree.
- Remova a derivação não utilizada.
- Propague assíncrono/aguarde e corrija `wsv::put` inesperado.
- Use a junção da caixa `futures`.
- Implementar execução de armazenamento paralelo: a gravação em disco e a atualização do WSV acontecem em paralelo.
- Use referências em vez de propriedade para (des)serialização.
- Ejeção de código de arquivos.
- Use ursa::blake2.
- Regra sobre mod.rs no guia de contribuição.
- Hash de 32 bytes.
- Hash Blake2.
- O disco aceita referências para bloquear.
- Refatoração do módulo de comandos e Árvore Inicial Merkle.
- Estrutura de módulos refatorada.
- Formatação correta.
- Adicione comentários de documentos a read_all.
- Implementar `read_all`, reorganizar testes de armazenamento e transformar testes com funções assíncronas em testes assíncronos.
- Remova captura mutável desnecessária.
- Revise o problema e corrija o clippy.
- Remova o traço.
- Adicionar verificação de formato.
- Adicionar token.
- Crie Rust.yml para ações do GitHub.
- Apresentar protótipo de armazenamento em disco.
- Teste e funcionalidade de ativos de transferência.
- Adicione inicializador padrão às estruturas.
- Altere o nome da estrutura MSTCache.
- Adicionar empréstimo esquecido.
- Esboço inicial do código iroha2.
- API Kura inicial.
- Adicione alguns arquivos básicos e também libere o primeiro rascunho do white paper descrevendo a visão do iroha v2.
- Ramo iroha v2 básico.

## [1.5.0] - 08/04/2022

### Alterações de CI/CD
- Remova Jenkinsfile e JenkinsCI.

### Adicionado

- Adicionar implementação de armazenamento RocksDB para Burrow.
- Introduzir otimização de tráfego com filtro Bloom
- Atualize a rede do módulo `MST` para estar localizada no módulo `OS` em `batches_cache`.
- Propor otimização de tráfego.

### Documentação

- Corrigir compilação. Adicione diferenças de banco de dados, prática de migração, endpoint de verificação de integridade, informações sobre a ferramenta iroha-swarm.

### Outros

- Correção de requisitos para compilação de documentos.
- Corte a documentação de lançamento para destacar o item crítico restante de acompanhamento.
- Corrigido 'verificar se a imagem do docker existe' /build all skip_testing.
- /construir tudo skip_testing.
- /construir skip_testing; E mais documentos.
- Adicione `.github/_README.md`.
- Remova `.packer`.
- Remova alterações no parâmetro de teste.
- Use o novo parâmetro para pular a fase de teste.
- Adicione ao fluxo de trabalho.
- Remova o envio do repositório.
- Adicionar envio de repositório.
- Adicionar parâmetro para testadores.
- Remova o tempo limite `proposal_delay`.

## [1.4.0] - 31/01/2022

### Adicionado

- Adicionar estado do nó de sincronização
- Adiciona métricas para RocksDB
- Adicione interfaces de verificação de integridade via http e métricas.

### Correções

- Corrigir famílias de colunas em Iroha v1.4-rc.2
- Adicionar filtro Bloom de 10 bits em Iroha v1.4-rc.1

### Documentação

- Adicione zip e pkg-config à lista de dependências de compilação.
- Atualizar leia-me: corrija links quebrados para criar status, guia de construção e assim por diante.
- Corrigir métricas de configuração e Docker.

### Outros

- Atualize a tag docker GHA.
- Correção de erros de compilação Iroha 1 ao compilar com g++11.
- Substitua `max_rounds_delay` por `proposal_creation_timeout`.
- Atualize o arquivo de configuração de amostra para remover parâmetros antigos de conexão de banco de dados.