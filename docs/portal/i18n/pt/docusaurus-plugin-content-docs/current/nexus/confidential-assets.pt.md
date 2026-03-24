---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ativos provisórios e transferências ZK
descrição: Blueprint Fase C para circulação cegada, registros e controles de operador.
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design de ações engajadas e transferências ZK

## Motivação
- Entregar fluxos de ativos blindados opt-in para que os domínios preservem a privacidade transacional sem alterar a circulação transparente.
- Manter a execução determinista em hardware heterogêneo de validadores e preservar Norito/Kotodama ABI v1.
- Fornecer auditores e operadores controles de ciclo de vida (ativação, rotação, revogação) para circuitos e parâmetros criptográficos.

## Modelo de ameaça
- Validadores são honestos-mas-curiosos: executam consenso fielmente mas experimentam operar ledger/state.
- Observadores de rede veem dados de bloco e transações fofocadas; nenhuma suposição de canais privados de fofoca.
- Fora de escopo: análise de tráfego off-ledger, adversários quanticos (acompanhados no roadmap PQ), ataques de disponibilidade do ledger.

## Visão geral do design
- Os ativos podem declarar um *pool blindado* além dos saldos transparentes existentes; a circulação cegada e representada via compromissos criptográficos.
- Notas encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independente da ordem das notas.
  - Carga útil criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transações transportam cargas úteis `ConfidentialTransfer` codificadas em Norito contendo:
  - Entradas públicas: âncora Merkle, anuladores, novos compromissos, id de ativos, versão de circuito.
  - Payloads criptografados para destinatários e auditores indiretamente.
  - Prova de conhecimento zero que atesta conservação de valor, titularidade e autorização.
- Verificação de chaves e conjuntos de parâmetros são controlados via registros on-ledger com janelas de ativação; nós recusamos validar provas que referenciam entradas desconhecidas ou revogadas.
- Os cabeçalhos de consenso comprometem o resumo ativo de capacidade confidencial para que os blocos sejam aceitos quando o estado de registro e os parâmetros coincidirem.
- Construção de provas usando um stack Halo2 (Plonkish) sem configuração confiável; Groth16 ou outras variantes SNARK são intencionalmente não suportadas em v1.

### Fixtures deterministas

Envelopes de memorandos provisórios agora enviam um dispositivo canônico em `fixtures/confidential/encrypted_payload_v1.json`. O dataset captura um envelope v1 positivo mais amostras negativas malformadas para que SDKs possam afirmar paridade de análise. Os testes do modelo de dados em Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e a suíte Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) carregam o fixture diretamente, garantindo que a codificação Norito, as superfícies de erro e a cobertura de regressão permaneçam compatíveis enquanto o codec evolui.

SDKs Swift agora podem emitir instruções escudo sem cola JSON bespoke: construa um
`ShieldRequest` com o comprometimento de 32 bytes, o payload criptografado e metadados de débito,
e então chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para alternar e encaminhar a
transação via `/v1/pipeline/transactions`. O helper valida comprimentos de compromisso,
insira `ConfidentialEncryptedPayload` no encoder Norito, e reflita o layout `zk::Shield`
descrito abaixo para que carteiras sejam certificadas com Rust.

## Compromissos de consenso e controle de capacidade
- Headers do bloco expoem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o digest participa do hash de consenso e deve igualar o visto local do registro para aceitação do bloco.
- Governança pode preparar upgrades programando `next_conf_features` com um `activation_height` futuro; até essa altura, os produtores de bloco deverão continuar emitindo o resumo anterior.
- Nodes validadores DEVEM operar com `confidential.enabled = true` e `assume_valid = false`. Checks de startup recusam entrar no set validador se alguma condição falhar ou se o `conf_features` local divergir.
- Os metadados do handshake P2P agora incluem `{ enabled, assume_valid, conf_features }`. Pares que anunciam recursos incompatíveis são rejeitados com `HandshakeConfidentialMismatch` e nunca entram em rotação de consenso.
- Os resultados do handshake entre validadores, observadores e peers são capturados na matriz de handshake em [Node Capability Negotiation](#node-capability-negotiation). Falhas de handshake expoem `HandshakeConfidentialMismatch` e mantem o peer fora da rotação de consenso até que o digest coincide.
- Observadores e não validadores podem definir `assume_valid = true`; Aplicar deltas de permissão sem verificar provas, mas não influenciar a segurança do consenso.## Política de ativos
- Cada definição de ativo carrega um `AssetConfidentialPolicy` definido pelo criador ou via governança:
  - `TransparentOnly`: modo padrão; apenas instruções transparentes (`MintAsset`, `TransferAsset`, etc.) são permitidas e operações blindadas são rejeitadas.
  - `ShieldedOnly`: toda emissão e transferências devem usar instruções provisórias; `RevealConfidential` e proibido para que saldos nunca aparecam publicamente.
  - `Convertible`: os titulares podem mover valor entre representações transparentes e blindadas usando instruções de rampa on/off abaixo.
- Políticas seguem um FSM restritas para evitar fundos encalhados:
  - `TransparentOnly -> Convertible` (habilitação imediata da piscina blindada).
  - `TransparentOnly -> ShieldedOnly` (solicita transição pendente e janela de conversação).
  - `Convertible -> ShieldedOnly` (atraso mínimo obrigatório).
  - `ShieldedOnly -> Convertible` (plano de migração necessário para que notas cegadas continuem gastaveis).
  - `ShieldedOnly -> TransparentOnly` e proibido a menos que o blindado pool fique vazio ou governança codifique uma migração que des-blind note pendentes.
- Instruções de governança definem `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via o ISI `ScheduleConfidentialPolicyTransition` e podem abortar mudanças programadas com `CancelConfidentialPolicyTransition`. A validação do mempool garante que nenhuma transação atravesse a altura da transição e a inclusão de falha deterministicamente se um cheque de política mudaria no meio do bloco.
- Transições pendentes são aplicadas automaticamente quando um novo bloco abre: quando a altura entra na janela de conversao (para upgrades `ShieldedOnly`) ou atinge `effective_height`, o runtime atualiza `AssetConfidentialPolicy`, refresca os metadados `zk.policy` e limpa a entrada pendente. Se o fornecimento transparente permanece quando uma transição `ShieldedOnly` madura, o runtime aborta a mudança e registra um aviso, mantendo o modo anterior.
- Os botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` impõem aviso mínimo e períodos de tolerância para permitir conversões de carteira em torno da mudança.
- `pending_transition.transition_id` também funciona como alça de auditório; governança deve citá-lo ao encerrar ou cancelar transições para que os operadores correlacionem relatórios de rampa de entrada/saída.
- `policy_transition_window_blocks` padrão é 720 (~12 horas com tempo de bloco de 60 s). Nós limitamos solicitações de governança que tentam aviso mais curto.
- Gênesis manifesta e fluxos CLI expoem políticas atuais e pendentes. A lógica de admissão à política em tempo de execução para confirmar que cada instrução confidencial foi autorizada.
- Checklist de migração - veja "Sequenciamento de migração" abaixo para o plano de atualização em etapas que o Milestone M0 acompanha.

#### Monitorando transições via Torii

Carteiras e auditores consultam `GET /v1/confidential/assets/{definition_id}/transitions` para funcionar ou `AssetConfidentialPolicy` ativo. O payload JSON sempre inclui o asset id canonico, a última altura do bloco observado, o `current_mode` da política, o modo efetivo nessa altura (janelas de conversa reportam temporariamente `Convertible`), e os identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Quando uma transição de governança está pendente, a resposta também embute:

- `transition_id` - identificador de auditoria retornado por `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e o `window_open_height` derivados (o bloco onde carteiras devem começar a conversar para cut-overs ShieldedOnly).

Exemplo de resposta:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Uma resposta `404` indica que não existe nenhuma definição de ativo correspondente. Quando não há transição agendada para o campo `pending_transition` e `null`.

### Máquina de estados de política| Modo atual | Próximo modo | Pré-requisitos | Tratamento de altura efetiva | Notas |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | Governança ativa ou entradas de registro de selecionados/parâmetros. Submedidor `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | A transição é executada exatamente em `effective_height`; a piscina blindada fica disponível imediatamente.               | Caminho padrão para ativar a confidencialidade mantendo fluxos transparentes.          |
| Somente Transparente | Somente blindado | Mesmo acima, mais `policy_transition_window_blocks >= 1`.                                                         | O runtime entra automaticamente em `Convertible` em `effective_height - policy_transition_window_blocks`; muda para `ShieldedOnly` em `effective_height`. | Fornece janela de conversa determinista antes de desabilitar instruções transparentes.   |
| Conversível | Somente blindado | Transição programada com `effective_height >= current_height + policy_transition_delay_blocks`. Governança DEVE certifica (`transparent_supply == 0`) via metadados de auditoria; runtime aplica isso sem cortes. | Semântica de janela idêntica. Se o fornecimento transparente para não-zero em `effective_height`, a transição aborta com `PolicyTransitionPrerequisiteFailed`. | Trava o ativo em circulação totalmente confidencial.                                      |
| Somente blindado | Conversível | Transição programada; sem saque emergencial ativo (`withdraw_height` indefinido).                              | O estado muda em `effective_height`; revelar rampas reabrem enquanto notas cegadas permanecem válidas.             | Usado para janelas de manutenção ou revisões de auditores.                                |
| Somente blindado | Somente Transparente | Governança deve provar `shielded_supply == 0` ou preparar um plano `EmergencyUnshield` contratado (assinaturas de auditor requeridas). | O runtime abre uma janela `Convertible` antes de `effective_height`; na altura, as instruções de falha são difíceis e o ativo retorna ao modo transparente-apenas. | Disse o último recurso. A transição se cancela automaticamente se qualquer nota confidencial for gasta durante uma janela. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` limpa a mudança pendente.                                                    | `pending_transition` removido imediatamente.                                                                        | Mantem o status quo; mostrado por completo.                                             |

Transições não específicas acima são rejeitadas durante a submissão de governança. O runtime verifica os pré-requisitos logo antes de aplicar uma transição programada; falhas devolvem o ativo ao modo anterior e emitem `PolicyTransitionPrerequisiteFailed` via telemetria e eventos de bloco.

### Sequenciamento de migração1. **Preparar cadastros:** ativar todas as entradas de selecionador e parâmetros referenciados pela política alvo. Os nós anunciam o `conf_features` resultando para que os pares verifiquem a coerência.
2. **Agendar a transição:** submedidor `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeite `policy_transition_delay_blocks`. Ao mover para `ShieldedOnly`, especifique uma janela de conversa (`window >= policy_transition_window_blocks`).
3. **Publicar guia para operadores:** registrador o `transition_id` retornado e circular um runbook on/off-ramp. Carteiras e auditores assinam `/v1/confidential/assets/{id}/transitions` para saber a altura de abertura da janela.
4. **Aplicar janela:** quando uma janela abre, o runtime muda a política para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, e começa a rejeitar solicitações de governança conflitantes.
5. **Finalizar ou abortar:** em `effective_height`, o tempo de execução verifica os pré-requisitos (fornecimento transparente zero, sem retiradas de emergência, etc.). Sucesso muda a política para o modo solicitado; falha emite `PolicyTransitionPrerequisiteFailed`, limpa a transição pendente e deixa a política inalterada.
6. **Atualizações de esquema:** após uma transição bem-sucedida, a governança aumenta a versão de esquema do ativo (por exemplo, `asset_definition.v2`) e ferramentas CLI exigem `confidential_policy` ao serializar manifestos. Docs de upgrade de genesis instruem os operadores a adicionar configurações de política e impressões digitais de registro antes de reiniciar validadores.

Redes novas que iniciam com confidencialidade habilitada codificam a política específica diretamente em gênese. Ainda assim segue a lista de verificação acima quando mudam os modos pós-launch para que as janelas de conversa sejam deterministas e as carteiras tenham tempo de ajuste.

### Versionamento e ativação do manifesto Norito

- Genesis manifests DEVE incluir um `SetParameter` para uma chave customizada `confidential_registry_root`. O payload e Norito JSON que corresponde a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir o campo (`null`) quando não há entradas ativas, ou fornecer uma string hex de 32 bytes (`0x...`) igual ao hash produzido por `compute_vk_set_hash` sobre as instruções de envio. nenhum manifesto. Nós recusamos iniciar se o parâmetro faltar ou se o hash divergir das escritas de registro codificadas.
- O on-wire `ConfidentialFeatureDigest::conf_rules_version` embute uma versão de layout do manifesto. Para redes v1 DEVE permanecer `Some(1)` e e igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quando o conjunto de regras evolui, aumenta constantemente, regenera manifestos e executa rollout de binários em lock-step; Traje versoes faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- Manifestos de ativação DEVEM agrupar atualizações de registro, mudanças de ciclo de vida de parâmetros e transições de política para manter o resumo consistente:
  1. Aplique alterações de registro feitas (`Publish*`, `Set*Lifecycle`) em uma view offline do estado e calcule o digest pos-ativacao com `compute_confidential_feature_digest`.
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando o hash calculado para que pares atrasados ​​recuperem o resumo correto mesmo se perderem instruções intermediárias.
  3. Anexar instruções `ScheduleConfidentialPolicyTransition`. Cada instrução deve citar o `transition_id` emitido por governança; manifesta que o esquecimento será rejeitado pelo runtime.
  4. Persistir os bytes do manifesto, uma impressão digital SHA-256 e o ​​resumo usado no plano de ativação. Os operadores verificam os três artefatos antes de votar ou se manifestar para evitar partições.
- Quando os rollouts exigem um cut-over diferido, registre a altura alvo em um parâmetro custom companheiro (por exemplo `custom.confidential_upgrade_activation_height`). Isso fornece aos auditores uma prova codificada em Norito de que os validadores honraram uma janela de aviso antes do digest entrar em vigor.## Ciclo de vida de verificadores e parâmetros
### Registro ZK
- O ledger armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` onde `proving_system` atualmente e fixo em `Halo2`.
- Pares `(circuit_id, version)` são globalmente únicos; o registro mantém um índice secundário para buscas por metadados de circuito. Tentativas de registrar um par duplicado são rejeitadas durante a admissão.
- `circuit_id` deve ser não vazio e `public_inputs_schema_hash` deve ser fornecido (tipicamente um hash Blake2b-32 da codificação canônica de entradas públicas do selecionador). A admissão rejeita registros que omitem esses campos.
- As instruções de governança incluem:
  - `PUBLISH` para adicionar uma entrada `Proposed` apenas com metadados.
  - `ACTIVATE { vk_id, activation_height }` para programar ativação em limite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar pode a altura final onde as provas referenciam a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergência; ativos afetados por congelamento, gastos adicionais, retiradas de altura e novas entradas ativadas.
- Genesis manifests auto-emitem um parametro custom `confidential_registry_root` cujo `vk_set_hash` coincide com as entradas ativas; a validação cruza esse resumo com o estado local do registro antes que um nó possa entrar sem consenso.
- Registrador ou atualização de um verificador requer `gas_schedule_id`; a verificação exige que a entrada do registro esteja `Active`, apresente no índice `(circuit_id, version)`, e que as provas Halo2 fornecam um `OpenVerifyEnvelope` cujo `circuit_id`, `vk_hash`, e `public_inputs_schema_hash` correspondem ao registro do registro.

### Provando Chaves
- As chaves de prova ficam off-ledger mas são referenciadas por identificadores content-addressed (`pk_cid`, `pk_hash`, `pk_len`) publicados ao lado dos metadados do verificador.
- SDKs de carteira buscam dados de PK, verificam hashes e fazem cache local.

### Parâmetros Pedersen e Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) espelham controles de ciclo de vida de verificadores, cada um com `params_id`, hashes de geradores/constantes, ativação, depreciação e alturas de retirada.
- Commitments e hashes separam domínios por `params_id` para que a rotação de parâmetros nunca reutilize padrões de bits de conjuntos obsoletos; o ID e embutido em compromissos de nota e tags de domínio de nulificador.
- Circuitos que suportam seleção multiparâmetro em tempo de verificação; conjuntos de parâmetros obsoletos permanecem gastáveis ​​em `deprecation_height`, e conjuntos retirados são eliminados exatamente em `withdraw_height`.

##Ordenação determinista e anuladora
- Cada ativo mantido um `CommitmentTree` com `next_leaf_index`; blocos acrescentam compromissos em ordem determinista: iterar transações na ordem do bloco; dentro de cada transação iterar saídas blindadas por `output_idx` serializado ascendente.
- `note_position` e derivado dos offsets da árvore mas **não** faz parte do nullifier; ele assim alimenta caminhos de adesão dentro do testemunho da prova.
- A estabilidade do nullifier sob reorgs e garantida pelo design PRF; o input PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e âncoras referenciam raízes Merkle históricos limitados por `max_anchor_age_blocks`.

## Fluxo do razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Solicitar política de ativo `Convertible` ou `ShieldedOnly`; admissão checa autoridade do ativo, recupera `params_id` atual, amostra `rho`, emite comprometimento, atualiza a árvore Merkle.
   - Emite `ConfidentialEvent::Shielded` com o novo commit, delta de Merkle root e hash de chamada da transação para audit trails.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - Syscall VM verifica a prova usando a entrada do registro; o host garante nulos não usados, compromissos anexados deterministicamente e âncora recente.
   - O ledger registra entradas `NullifierSet`, armazena payloads criptografados para destinatários/auditores e emite `ConfidentialEvent::Transferred` resumindo nullifiers, outputs ordenados, hash de prova e raízes Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - Disponibilização apenas para ativos `Convertible`; a prova valida que o valor da nota igual ao montante revelado, o ledger credita balance transparente e queima a nota blindada marcando o nullifier como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o montante público, anuladores consumidos, identificadores de prova e hash de chamada da transação.## Adicoes ao modelo de dados
- `ConfidentialConfig` (nova seção de configuração) com flag de habilitação, `assume_valid`, botões de gás/limites, janela de âncora, backend de verificador.
- `ConfidentialNote`, `ConfidentialTransfer`, e `ConfidentialMint` esquemas Norito com byte de versão explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envolve bytes de memo AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, com padrão `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para o layout XChaCha20-Poly1305.
- Vetores canônicos de derivação de chave vivem em `docs/source/confidential_key_vectors.json`; Tanto o CLI quanto o endpoint Torii regressam contra esses fixtures.
- `asset::AssetDefinition` ganha `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste o bind `(backend, name, commitment)` para verificadores de transferência/desproteção; a execucao rejeita provas cuja chave de verificação referenciada ou inline não corresponde ao compromisso registrado.
- `CommitmentTree` (por asset com frontier checkpoints), `NullifierSet` com chave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armazenados em estado mundial.
- Mempool mantem estruturas transitórias `NullifierIndex` e `AnchorIndex` para detecção precoce de duplicados e verificação de idade de âncora.
- Atualizações do esquema Norito incluem ordenação canônica para entradas públicas; testes de ida e volta garantem o determinismo de codificação.
- Roundtrips de payload criptografado ficam fixados via testes unitários (`crates/iroha_data_model/src/confidential.rs`). Vetores de carteira de acompanhamento vão transcrições de fixação AEAD canônicos para auditores. `norito.md` documentação de cabeçalho on-wire para envelope.

## Integração IVM e syscall
- Introduzir syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, e o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - O syscall carrega metadados do verificador do registro, aplica limites de tamanho/tempo, cobra gás determinista, e então aplica o delta se a prova tiver sucesso.
- O host expoe o trait read-only `ConfidentialLedger` para recuperar snapshots de Merkle root e estado de nullifier; a biblioteca Kotodama fornece ajudantes de montagem de testemunha e validação de esquema.
- Documentos de ponteiro-ABI foram atualizados para esclarecimento de layout de buffer de prova e identificadores de registro.

## Negociação de capacidades de nó
- O aperto de mão anuncia `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. A participação de validadores requer `confidential.enabled=true`, `assume_valid=false`, identificadores de backend do verificador idênticos e resumos que incluem; incompatibilidades falham o handshake com `HandshakeConfidentialMismatch`.
- Configuração suportada `assume_valid` apenas para observadores: quando desativado, encontrar instruções disponíveis gera `UnsupportedInstruction` determinista sem pânico; quando habilitado, os observadores aplicam os deltas declarados sem verificar as provas.
- Mempool rejeita transações reservadas se a capacidade local estiver desabilitada. Filtros de fofoca evitam enviar transações blindadas para pares incompativos enquanto encaminham cegamente IDs de verificador desconhecidos dentro de limites de tamanho.

### Matriz de aperto de mão

| Anúncio remoto | Resultado para nós validadores | Notas do operador |
|------------------|------------------|----------------|
| `enabled=true`, `assume_valid=false`, correspondência de back-end, correspondência de resumo | Aceito | O par chega ao estado `Ready` e participa de proposta, voto e RBC fan-out. Nenhuma ação manual necessária. |
| `enabled=true`, `assume_valid=false`, correspondência de backend, resumo obsoleto ou ausente | Rejeitado (`HandshakeConfidentialMismatch`) | O remoto deve aplicar ativações pendentes de registro/parâmetros ou aguardar o `activation_height` programado. Ate concordo, o nó seguevel mas nunca entra na rotacao de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeitado (`HandshakeConfidentialMismatch`) | Validadores requerem verificação de provas; configure o remoto como observer com Torii-only ingress ou mude `assume_valid=false` após ativar a verificação completa. |
| `enabled=false`, campos omitidos (build desatualizado), ou backend de verificador diferente | Rejeitado (`HandshakeConfidentialMismatch`) | Pares desatualizados ou parcialmente atualizados não podem entrar na rede de consenso. Atualize para o release atual e garanta que a tupla backend + digest corresponda antes de reconectar. |

Observadores que intencionalmente pulam a verificação de provas não devem abrir conexões de consenso contra validadores com portas de capacidade. Eles ainda ingerem blocos via Sumeragi podem017X ou APIs de arquivo, mas a rede de consenso os rejeita ate anunciam capacidades compativeis.

### Política de poda de revelação e retenção de anuladorOs registros contábeis devem ter histórico suficiente para comprovar afrescos de notas e reproduzir auditorias de governança. A política padrão, aplicada por `ConfidentialLedger`, e:

- **Retenção de nulificadores:** manter nulificadores gastos por um *mínimo* de `730` dias (24 meses) após a altura de gasto, ou a janela reguladora obrigatória se for maior. Os operadores podem ampliar a janela via `confidential.retention.nullifier_days`. Os anuladores mais novos que a janela DEVEM permanecer consultáveis ​​via Torii para que os auditores provem ausencia de gasto duplo.
- **Pruning de revela:** revela transparentes (`RevealConfidential`) podam compromissos associados imediatamente após o bloco finalizar, mas o nullifier consumido continua sujeito a regra de retencao acima. Eventos `ConfidentialEvent::Unshielded` registram o valor público, destinatário e hash de prova para que reconstruir revela históricos não exija o texto cifrado podado.
- **Pontos de controle de fronteira:** fronteiras de compromisso mantem pontos de controle rolantes cobrindo o maior entre `max_anchor_age_blocks` e uma janela de retenção. Nodes compactam checkpoints antigos apenas depois que todos os nullifiers no intervalo expiram.
- **Remediacao de digest stale:** se `HandshakeConfidentialMismatch` ocorrer por desvio de digest, os operadores devem (1) verificar se as janelas de retenção de nullifiers estão homologadas no cluster, (2) rodar `iroha_cli app confidential verify-ledger` para regenerar o digest contra o conjunto de nullifiers retidos, e (3) reimplementar o manifesto atualizado. Os anuladores podados prematuramente devem ser restaurados no armazenamento refrigerado antes de reingressar na rede.

Documento substitui locais no runbook de operações; políticas de governança que estendem a janela de retenção devem atualizar a configuração do nó e os planos de armazenamento do arquivo em lockstep.

### Fluxo de despejo e recuperação

1. Durante o dial, `IrohaNetwork` compara as capacidades anunciadas. Qualquer incompatibilidade levanta `HandshakeConfidentialMismatch`; a conexão e fechada e o par permanece na fila de descoberta sem ser promovido a `Ready`.
2. Uma falha aparece no log do serviço de rede (inclui resumo remoto e backend), e Sumeragi nunca agenda o peer para proposta ou voto.
3. Operadores remediam alinhando registros de verificador e conjuntos de parâmetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou programando `next_conf_features` com um `activation_height` acordado. Uma vez que o resumo coincide, o próximo handshake tem sucesso automaticamente.
4. Se um peer stale consegue difundir um bloco (por exemplo, via replay de arquivo), validadores ou rejeitam deterministicamente com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, mantendo o estado do razão consistente na rede.

### Fluxo de handshake seguro contra replay

1. Cada tentativa de saída aloca material de chave Noise/X25519 novo. O payload de handshake assinado (`handshake_signature_payload`) concatena as chaves públicas efêmeras locais e remotas, o endereco de soquete anunciado codificado em Norito e, quando compilado com `handshake_chain_id`, o identificador de cadeia. A mensagem e criptografada com AEAD antes de sair do node.
2. O respondente recomputa o payload com a ordem de chaves peer/local invertida e verifica a assinatura Ed25519 embutida em `HandshakeHelloV1`. Como ambas as chaves efêmeras e o endereco anunciado fazem parte do domínio de assinatura, replay de uma mensagem capturada contra outro peer ou recuperar uma conexão obsoleta deterministicamente.
3. Bandeiras de capacidade confidencial e o `ConfidentialFeatureDigest` viajam dentro de `HandshakeConfidentialMeta`. O receptor compara a tupla `{ enabled, assume_valid, verifier_backend, digest }` com seu `ConfidentialHandshakeCaps` local; qualquer incompatibilidade sai cedo com `HandshakeConfidentialMismatch` antes de o transporte transitar para `Ready`.
4. Os operadores DEVEM recomputar o resumo (via `compute_confidential_feature_digest`) e reiniciar nós com registros/políticas atualizadas antes de reconectar. Peers anunciando digests antigos continuam falhando no handshake, evitando que estado stale reentre no set validador.
5. Sucessos e falhas de handshake atualizam contadores padrão `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomia de erros) e emitem logs estruturados com o peer ID remoto e a impressão digital do digest. Monitore esses indicadores para detectar replays ou configurações incorretas durante o lançamento.## Gerenciamento de chaves e cargas úteis
- Hierarquia de derivação por conta:
  - `sk_spend` -> `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- Payloads de notas criptografadas utilizam AEAD com chaves compartilhadas derivadas de ECDH; visualizar chaves de auditoria podem ser anexadas a resultados conforme a política do ativo.
- Adicoes ao CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ferramentas de auditor para descrever memorandos, e o auxiliar `iroha app zk envelope` para produzir/inspecionar envelopes Norito offline. Torii expõe o mesmo fluxo de derivação via `POST /v1/confidential/derive-keyset`, retornando formas hex e base64 para que carteiras busquem posições de chave programaticamente.

## Gás, limites e controles DoS
- Cronograma de gás determinista:
  - Halo2 (Plonkish): gás base `250_000` + gás `2_000` por entrada pública.
  - `5` gás por prova byte, mais cargas por nulificador (`300`) e por compromisso (`500`).
  - Operadores podem sobrescrever essas constantes via configuração do nó (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); mudam de propagação no startup ou no hot-reload da camada de configuração e são aplicados deterministicamente no cluster.
- Limites duros (padrões configuráveis):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Provas que excedem `verify_timeout_ms` abortam a instrução deterministicamente (cédulas de governança emitem `proof verification exceeded timeout`, `VerifyProof` retorna erro).
- Cotas adicionais garantindo vivacidade: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, e `max_public_inputs` limitam construtores de blocos; `reorg_depth_bound` (>= `max_anchor_age_blocks`) governa a retenção de postos de controle de fronteira.
- A execução runtime agora rejeita transações que excedem esses limites por transação ou por bloco, emitindo erros `InvalidParameter` deterministas e mantendo o estado do razão inalterado.
- Mempool pré-filtra transações temporárias por `vk_id`, tamanho de prova e idade de âncora antes de invocar o verificador para manter o uso de recursos limitados.
- Uma verificação para deterministicamente em timeout ou violação de limite; transações falham com erros explícitos. Os backends SIMD são instrutivos, mas não alteram a contabilidade de gás.

### Baselines de calibração e portas de aceitação
- **Plataformas de referência.** Rodadas de calibração DEVEM cobrir os três perfis abaixo. Rodadas sem todos os perfis são rejeitadas na revisão.

  | Perfil | Arquitetura | CPU / Instância | Bandeiras de compilador | Proposta |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores de piso sem intrínsecas orientações; usado para ajustar tabelas de custo de fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | versão padrão | Valida o caminho AVX2; checa se os ganhos SIMD ficam dentro da tolerância do gás neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versão padrão | Garante que o backend NEON permaneça determinista e alinhado aos horários x86. |

- **Benchmark chicote.** Todos os relatos de calibração de gás DEVEM ser produzidos com:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar o dispositivo determinista.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` sempre que os custos de opcode da VM mudam.

- **Correção de aleatoriedade.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de rodar bancos para que `iroha_test_samples::gen_account_in` mude para o caminho determinista `KeyPair::from_seed`. O chicote imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; se a variavel faltar, a review DEVE falhar. Qualquer nova de calibração russa deve continuar honrando este env var ao introduzir aleatoriedade auxiliar.

- **Captura de resultados.**
  - Critério de upload de resumos (`target/criterion/**/raw.csv`) para cada perfil no artista de lançamento.
  - Armazenar métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Confidential Gas Calibration ledger](./confidential-gas-calibration) junto com o commit git e a versão de traduzidor usado.
  - Manter as duas últimas linhas de base por perfil; apagar snapshots mais antigos uma vez validado o relatorio mais novo.

- **Tolerâncias de aceitação.**
  - Deltas de gás entre `baseline-simd-neutral` e `baseline-avx2` DEVEM permanecer <= +/-1,5%.
  - Deltas de gás entre `baseline-simd-neutral` e `baseline-neon` DEVEM permanecer <= +/-2,0%.
  - Propostas de calibração que excedem esses limites exigem ajustes de cronograma ou uma RFC explicando a discrepância e a mitigação.- **Checklist de revisão.** Os remetentes são responsáveis por:
  - Incluir `uname -a`, trechos de `/proc/cpuinfo` (modelo, stepping), e `rustc -Vv` sem log de calibração.
  - Verifique se `IROHA_CONF_GAS_SEED` aparece na saida do banco (as bancos imprimem a semente ativa).
  - Garantir que feature flags do pacemaker e do verificador confidencial espelhem produção (`--features confidential,telemetry` ao rodar bancos com Telemetria).

## Configurações e operações
- `iroha_config` adicione a seção `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetria emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total`, sem expor dados em claro.
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## Estratégia de testes
- Determinismo: randomização de transações dentro de blocos gera raízes Merkle e conjuntos de nulificadores idênticos.
- Resiliencia a reorg: reorgs simulares multi-bloco com âncoras; nullifiers permanecem estaveis e âncoras stale são rejeitados.
- Invariantes de gás: verifica o uso de gás idêntico entre nós com e sem aceleração SIMD.
- Teste de limite: provas sem limite de tamanho/gás, contagens máximas de entrada/saída, aplicação de tempo limite.
- Ciclo de vida: operações de governança para ativação/descontinuação de verificador e parâmetros, testes de gasto após rotação.
- Política FSM: transições permitidas/negadas, atrasos de transição pendente e rejeição de mempool perto de alturas efetivas.
- Emergências de registro: retirada emergencial de congelamento de ativos afetados em `withdraw_height` e rejeita provas depois.
- Capability gating: validadores com `conf_features` divergentes rejeitam blocos; observers com `assume_valid=true` acompanham sem consenso.
- Equivalência de estado: nós validador/full/observador produções raízes de estado idênticas na cadeia canônica.
- Fuzzing negativo: provas malformadas, payloads superdimensionados e colisões de nullifier são rejeitados deterministicamente.

## Migração
- Rollout com feature flag: ao final da Fase C3, `enabled` padrão a `false`; nós anunciam capacidades antes de entrar no set validador.
- Ativos transparentes não são afetados; instruções necessárias requerem entradas de registro e negociação de capacidades.
- Nós elaborados sem suporte confidencial rejeitam blocos relevantes deterministicamente; Não pode entrar no set validador, mas pode operar como observadores com `assume_valid=true`.
- Genesis manifests inclui entradas iniciais de registro, conjuntos de parâmetros, políticas previstas para ativos e chaves de auditor adicionais.
- Operadores seguem runbooks publicados para rotação de registro, transições políticas e retirada emergencial para manter atualizações deterministas.

##Trabalho pendente
- Benchmark de parâmetros Halo2 (tamanho de circuito, estratégia de lookup) e registrador de resultados no playbook de calibração para que padrões de gas/timeout sejam atualizados junto ao próximo update de `confidential_assets_calibration.md`.
- Finalizar políticas de divulgação de auditor e APIs de visualização seletiva associadas, conectando o fluxo de trabalho aprovado em Torii assim que o rascunho de governança para assinado.
- Estender o esquema de criptografia de testemunha para cobrir saídas multi-destinatários e memorandos em lote, documentando o formato do envelope para implementadores de SDK.
- Comissionar uma revisão de segurança externa de circuitos, registros e procedimentos de rotação de parâmetros e arquivar os achados ao lado dos relatos internos de auditoria.
- Especificar APIs de reconciliação de gastos para auditores e publicar guia de escopo de view-key para que fornecedores de carteira implementem as mesmas semânticas de atestação.## Fases de implementação
1. **Fase M0 - Endurecimento Stop-Ship**
   - [x] Derivação de nullifier segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com ordenação determinista de compromissos aplicados nas atualizações do ledger.
   - [x] Execução aplica limites de tamanho de prova e cotas previstas por transação/por bloco, rejeitando transações fora de orçamento com erros deterministas.
   - [x] Handshake P2P anuncia `ConfidentialFeatureDigest` (backend digest + impressões digitais de registro) e falha mismatches deterministicamente via `HandshakeConfidentialMismatch`.
   - [x] Remover panics em caminhos de execução confidenciais e adicionar role gating para nós incompativeis.
   - [ ] Aplicar orçamentos de timeout do verificador e limites de profundidade de reorganização para checkpoints de fronteira.
     - [x] Orçamentos de timeout de verificação aplicados; provas que excedem `verify_timeout_ms` agora falham deterministicamente.
     - [x] Frontier checkpoints agora respeitam `reorg_depth_bound`, podando checkpoints mais antigos que a janela configurada e mantendo snapshots deterministas.
   - Introduzir `AssetConfidentialPolicy`, política FSM e portões de execução para instruções mint/transfer/reveal.
   - Commit `conf_features` nos cabeçalhos do bloco e recusar participação de validadores quando resumos de registro/parâmetros divergem.
2. **Fase M1 - Registros e parâmetros**
   - Entregar registros `ZkVerifierEntry`, `PedersenParams`, e `PoseidonParams` com operações de governança, ancoragem de gênese e gestão de cache.
   - Conectar syscall para exigir pesquisas de registro, IDs de agendamento de gás, hash de esquema e verificações de tamanho.
   - Enviar formato de payload criptografado v1, vetores de derivação de chaves para carteira, e suporte CLI para gerenciamento de chaves privadas.
3. **Fase M2 - Gás e desempenho**
   - Implementar cronograma de gás determinista, contadores por bloco e chicotes de benchmark com telemetria (latência de verificação, tamanhos de prova, rejeitos de mempool).
   - Endurecer CommitmentTree checkpoints, carga LRU e índices de anulação para cargas de trabalho multi-asset.
4. **Fase M3 - Rotação e ferramental de carteira**
   - Habilitar aceitação de provas multiparâmetro e multiversão; suportar ativação/deprecacao guiada por governança com runbooks de transição.
   - Entregar fluxos de migração SDK/CLI, fluxos de trabalho de varredura de auditor e ferramentas de reconciliação de gastos.
5. **Fase M4 - Auditoria e operações**
   - Fornecer fluxos de trabalho de chaves de auditoria, APIs de divulgação seletiva e runbooks operacionais.
   - Agendar revisão externa de criptografia/segurança e publicar achados em `status.md`.

Cada fase atualiza marcos do roadmap e testes associados para manter garantias de execução determinista na rede blockchain.