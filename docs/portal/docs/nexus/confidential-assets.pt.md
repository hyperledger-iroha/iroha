---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffe425507ce77dadc54da5eb0a537eb67f0d7353e95c2aaf1f7467ce669845ee
source_last_modified: "2025-11-09T15:13:32.471456+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Ativos confidenciais e transferencias ZK
description: Blueprint Phase C para circulacao blindada, registries e controles de operador.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design de ativos confidenciais e transferencias ZK

## Motivacao
- Entregar fluxos de ativos blindados opt-in para que dominios preservem privacidade transacional sem alterar a circulacao transparente.
- Manter execucao determinista em hardware heterogeneo de validadores e preservar Norito/Kotodama ABI v1.
- Fornecer a auditores e operadores controles de ciclo de vida (ativacao, rotacao, revogacao) para circuitos e parametros criptograficos.

## Threat model
- Validadores sao honest-but-curious: executam consenso fielmente mas tentam inspecionar ledger/state.
- Observadores de rede veem dados de bloco e transacoes gossiped; nenhuma suposicao de canais privados de gossip.
- Fora de escopo: analise de trafego off-ledger, adversarios quanticos (acompanhado no roadmap PQ), ataques de disponibilidade do ledger.

## Visao geral do design
- Assets podem declarar um *shielded pool* alem dos balances transparentes existentes; a circulacao blindada e representada via commitments criptograficos.
- Notes encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independente da ordem das notes.
  - Payload encriptado: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transacoes transportam payloads `ConfidentialTransfer` codificados em Norito contendo:
  - Inputs publicos: Merkle anchor, nullifiers, novos commitments, asset id, versao de circuito.
  - Payloads encriptados para recipients e auditores opcionais.
  - Proof zero-knowledge que atesta conservacao de valor, ownership e autorizacao.
- Verifying keys e conjuntos de parametros sao controlados via registries on-ledger com janelas de ativacao; nodes recusam validar proofs que referenciam entradas desconhecidas ou revogadas.
- Headers de consenso comprometem o digest ativo de capacidade confidencial para que blocos so sejam aceitos quando o estado de registry e parametros coincide.
- Construcao de proofs usa um stack Halo2 (Plonkish) sem trusted setup; Groth16 ou outras variantes SNARK sao intencionalmente nao suportadas em v1.

### Fixtures deterministas

Envelopes de memo confidenciais agora enviam um fixture canonico em `fixtures/confidential/encrypted_payload_v1.json`. O dataset captura um envelope v1 positivo mais amostras negativas malformadas para que SDKs possam afirmar paridade de parsing. Os testes do data-model em Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e a suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) carregam o fixture diretamente, garantindo que o encoding Norito, as superficies de erro e a cobertura de regressao permanecam alinhadas enquanto o codec evolui.

SDKs Swift agora podem emitir instrucoes shield sem glue JSON bespoke: construa um
`ShieldRequest` com o commitment de 32 bytes, o payload encriptado e metadata de debito,
e entao chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para assinar e encaminhar a
transacao via `/v1/pipeline/transactions`. O helper valida comprimentos de commitment,
insere `ConfidentialEncryptedPayload` no encoder Norito, e espelha o layout `zk::Shield`
descrito abaixo para que wallets fiquem alinhadas com Rust.

## Consensus commitments e capability gating
- Headers de bloco expoem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o digest participa do hash de consenso e deve igualar a visao local do registry para aceitacao de bloco.
- Governance pode preparar upgrades programando `next_conf_features` com um `activation_height` futuro; ate essa altura, produtores de bloco devem continuar emitindo o digest anterior.
- Nodes validadores DEVEM operar com `confidential.enabled = true` e `assume_valid = false`. Checks de startup recusam entrar no set validador se qualquer condicao falhar ou se o `conf_features` local divergir.
- A metadata do handshake P2P agora inclui `{ enabled, assume_valid, conf_features }`. Peers que anunciam features incompativeis sao rejeitados com `HandshakeConfidentialMismatch` e nunca entram em rotacao de consenso.
- Outcomes de handshake entre validadores, observers e peers sao capturados na matriz de handshake em [Node Capability Negotiation](#node-capability-negotiation). Falhas de handshake expoem `HandshakeConfidentialMismatch` e mantem o peer fora da rotacao de consenso ate que o digest coincida.
- Observers nao validadores podem definir `assume_valid = true`; aplicam deltas confidenciais sem verificar proofs, mas nao influenciam a seguranca do consenso.

## Politicas de assets
- Cada definicao de asset carrega um `AssetConfidentialPolicy` definido pelo criador ou via governance:
  - `TransparentOnly`: modo default; apenas instrucoes transparentes (`MintAsset`, `TransferAsset`, etc.) sao permitidas e operacoes shielded sao rejeitadas.
  - `ShieldedOnly`: toda emissao e transferencias devem usar instrucoes confidenciais; `RevealConfidential` e proibido para que balances nunca aparecam publicamente.
  - `Convertible`: holders podem mover valor entre representacoes transparentes e shielded usando instrucoes de on/off-ramp abaixo.
- Politicas seguem um FSM restrito para evitar fundos encalhados:
  - `TransparentOnly -> Convertible` (habilitacao imediata do shielded pool).
  - `TransparentOnly -> ShieldedOnly` (requer transicao pendente e janela de conversao).
  - `Convertible -> ShieldedOnly` (delay minimo obrigatorio).
  - `ShieldedOnly -> Convertible` (plano de migracao requerido para que notes blindadas continuem gastaveis).
  - `ShieldedOnly -> TransparentOnly` e proibido a menos que o shielded pool esteja vazio ou governance codifique uma migracao que des-blinde notes pendentes.
- Instrucoes de governance definem `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via o ISI `ScheduleConfidentialPolicyTransition` e podem abortar mudancas programadas com `CancelConfidentialPolicyTransition`. A validacao do mempool garante que nenhuma transacao atravesse a altura de transicao e a inclusao falha deterministicamente se um check de politica mudaria no meio do bloco.
- Transicoes pendentes sao aplicadas automaticamente quando um novo bloco abre: quando a altura entra na janela de conversao (para upgrades `ShieldedOnly`) ou atinge `effective_height`, o runtime atualiza `AssetConfidentialPolicy`, refresca a metadata `zk.policy` e limpa a entrada pendente. Se o supply transparente permanece quando uma transicao `ShieldedOnly` amadurece, o runtime aborta a mudanca e registra um aviso, mantendo o modo anterior.
- Knobs de config `policy_transition_delay_blocks` e `policy_transition_window_blocks` impoem aviso minimo e periodos de tolerancia para permitir conversoes de wallet em torno da mudanca.
- `pending_transition.transition_id` tambem funciona como handle de auditoria; governance deve cita-lo ao finalizar ou cancelar transicoes para que operadores correlacionem relatorios de on/off-ramp.
- `policy_transition_window_blocks` default a 720 (~12 horas com block time de 60 s). Nodes limitam requests de governance que tentem aviso mais curto.
- Genesis manifests e fluxos CLI expoem politicas atuais e pendentes. A logica de admission le a politica em tempo de execucao para confirmar que cada instrucao confidencial esta autorizada.
- Checklist de migracao - ver "Migration sequencing" abaixo para o plano de upgrade em etapas que o Milestone M0 acompanha.

#### Monitorando transicoes via Torii

Wallets e auditores consultam `GET /v1/confidential/assets/{definition_id}/transitions` para inspecionar o `AssetConfidentialPolicy` ativo. O payload JSON sempre inclui o asset id canonico, a ultima altura de bloco observada, o `current_mode` da politica, o modo efetivo nessa altura (janelas de conversao reportam temporariamente `Convertible`), e os identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Quando uma transicao de governance esta pendente a resposta tambem embute:

- `transition_id` - handle de auditoria retornado por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` e o `window_open_height` derivado (o bloco onde wallets devem comecar conversao para cut-overs ShieldedOnly).

Exemplo de resposta:

```json
{
  "asset_id": "rose#wonderland",
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

Uma resposta `404` indica que nenhuma definicao de asset correspondente existe. Quando nao ha transicao agendada o campo `pending_transition` e `null`.

### Maquina de estados de politica

| Modo atual       | Proximo modo      | Prerrequisitos                                                                 | Tratamento de altura efetiva                                                                                         | Notas                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance ativou entradas de registry de verificador/parametros. Submeter `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | A transicao executa exatamente em `effective_height`; o shielded pool fica disponivel imediatamente.               | Caminho default para habilitar confidencialidade mantendo fluxos transparentes.          |
| TransparentOnly    | ShieldedOnly     | Mesmo acima, mais `policy_transition_window_blocks >= 1`.                                                         | O runtime entra automaticamente em `Convertible` em `effective_height - policy_transition_window_blocks`; muda para `ShieldedOnly` em `effective_height`. | Fornece janela de conversao determinista antes de desabilitar instrucoes transparentes.   |
| Convertible        | ShieldedOnly     | Transicao programada com `effective_height >= current_height + policy_transition_delay_blocks`. Governance DEVE certificar (`transparent_supply == 0`) via metadata de auditoria; runtime aplica isso no cut-over. | Semantica de janela identica. Se o supply transparente for nao-zero em `effective_height`, a transicao aborta com `PolicyTransitionPrerequisiteFailed`. | Trava o asset em circulacao totalmente confidencial.                                      |
| ShieldedOnly       | Convertible      | Transicao programada; sem emergency withdrawal ativo (`withdraw_height` indefinido).                              | O estado muda em `effective_height`; reveal ramps reabrem enquanto notes blindadas permanecem validas.             | Usado para janelas de manutencao ou revisoes de auditores.                                |
| ShieldedOnly       | TransparentOnly  | Governance deve provar `shielded_supply == 0` ou preparar um plano `EmergencyUnshield` assinado (assinaturas de auditor requeridas). | O runtime abre uma janela `Convertible` antes de `effective_height`; na altura, instrucoes confidenciais falham duro e o asset retorna ao modo transparent-only. | Saida de ultimo recurso. A transicao se auto-cancela se qualquer note confidencial for gasta durante a janela. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` limpa a mudanca pendente.                                                    | `pending_transition` removido imediatamente.                                                                        | Mantem o status quo; mostrado por completude.                                             |

Transicoes nao listadas acima sao rejeitadas durante submissao de governance. O runtime checa prerrequisitos logo antes de aplicar uma transicao programada; falhas devolvem o asset ao modo anterior e emitem `PolicyTransitionPrerequisiteFailed` via telemetria e eventos de bloco.

### Sequenciamento de migracao

1. **Preparar registries:** ativar todas as entradas de verificador e parametros referenciadas pela politica alvo. Nodes anunciam o `conf_features` resultante para que peers verifiquem coerencia.
2. **Agendar a transicao:** submeter `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeite `policy_transition_delay_blocks`. Ao mover para `ShieldedOnly`, especificar uma janela de conversao (`window >= policy_transition_window_blocks`).
3. **Publicar guia para operadores:** registrar o `transition_id` retornado e circular um runbook on/off-ramp. Wallets e auditores assinam `/v1/confidential/assets/{id}/transitions` para aprender a altura de abertura da janela.
4. **Aplicar janela:** quando a janela abre, o runtime muda a politica para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, e comeca a rejeitar requests de governance conflitantes.
5. **Finalizar ou abortar:** em `effective_height`, o runtime verifica os prerrequisitos (supply transparente zero, sem retiradas de emergencia, etc.). Sucesso muda a politica para o modo solicitado; falha emite `PolicyTransitionPrerequisiteFailed`, limpa a transicao pendente e deixa a politica inalterada.
6. **Upgrades de schema:** apos uma transicao bem-sucedida, governance aumenta a versao de schema do asset (por exemplo, `asset_definition.v2`) e tooling CLI exige `confidential_policy` ao serializar manifests. Docs de upgrade de genesis instruem operadores a adicionar settings de politica e fingerprints de registry antes de reiniciar validadores.

Redes novas que iniciam com confidencialidade habilitada codificam a politica desejada diretamente em genesis. Ainda assim seguem a checklist acima quando mudam modos pos-launch para que janelas de conversao sejam deterministas e wallets tenham tempo de ajustar.

### Versionamento e ativacao de manifest Norito

- Genesis manifests DEVEM incluir um `SetParameter` para a key custom `confidential_registry_root`. O payload e Norito JSON que corresponde a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir o campo (`null`) quando nao ha entradas ativas, ou fornecer um string hex de 32 bytes (`0x...`) igual ao hash produzido por `compute_vk_set_hash` sobre as instrucoes de verificador enviadas no manifest. Nodes recusam iniciar se o parametro faltar ou se o hash divergir das escritas de registry codificadas.
- O on-wire `ConfidentialFeatureDigest::conf_rules_version` embute a versao de layout do manifest. Para redes v1 DEVE permanecer `Some(1)` e e igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quando o ruleset evoluir, aumente a constante, regenere manifests e execute rollout de binarios em lock-step; misturar versoes faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- Activation manifests DEVEM agrupar updates de registry, mudancas de ciclo de vida de parametros e transicoes de politica para manter o digest consistente:
  1. Aplicar mutacoes de registry planejadas (`Publish*`, `Set*Lifecycle`) em uma view offline do estado e calcular o digest pos-ativacao com `compute_confidential_feature_digest`.
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando o hash calculado para que peers atrasados recuperem o digest correto mesmo se perderem instrucoes intermediarias.
  3. Anexar instrucoes `ScheduleConfidentialPolicyTransition`. Cada instrucao deve citar o `transition_id` emitido por governance; manifests que o esquecem serao rejeitados pelo runtime.
  4. Persistir os bytes do manifest, um fingerprint SHA-256 e o digest usado no plano de ativacao. Operadores verificam os tres artefatos antes de votar o manifest para evitar particoes.
- Quando rollouts exigirem um cut-over diferido, registre a altura alvo em um parametro custom companheiro (por exemplo `custom.confidential_upgrade_activation_height`). Isso fornece aos auditores uma prova codificada em Norito de que validadores honraram a janela de aviso antes do digest entrar em efeito.

## Ciclo de vida de verifiers e parametros
### ZK Registry
- O ledger armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` onde `proving_system` atualmente e fixo em `Halo2`.
- Pares `(circuit_id, version)` sao globalmente unicos; o registry mantem um indice secundario para buscas por metadata de circuito. Tentativas de registrar um par duplicado sao rejeitadas durante admission.
- `circuit_id` deve ser nao vazio e `public_inputs_schema_hash` deve ser fornecido (tipicamente um hash Blake2b-32 do encoding canonico de public inputs do verificador). Admission rejeita registros que omitem esses campos.
- Instrucoes de governance incluem:
  - `PUBLISH` para adicionar uma entrada `Proposed` apenas com metadata.
  - `ACTIVATE { vk_id, activation_height }` para programar ativacao em limite de epoch.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar a altura final onde proofs podem referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergencia; assets afetados congelam gastos confidenciais apos withdraw height ate novas entradas ativarem.
- Genesis manifests auto-emitem um parametro custom `confidential_registry_root` cujo `vk_set_hash` coincide com as entradas ativas; a validacao cruza esse digest com o estado local do registry antes que um node possa entrar no consenso.
- Registrar ou atualizar um verifier requer `gas_schedule_id`; a verificacao exige que a entrada do registry esteja `Active`, presente no indice `(circuit_id, version)`, e que proofs Halo2 fornecam um `OpenVerifyEnvelope` cujo `circuit_id`, `vk_hash`, e `public_inputs_schema_hash` correspondam ao registro do registry.

### Proving Keys
- Proving keys ficam off-ledger mas sao referenciadas por identificadores content-addressed (`pk_cid`, `pk_hash`, `pk_len`) publicados ao lado da metadata do verifier.
- SDKs de wallet buscam dados de PK, verificam hashes e fazem cache local.

### Parametros Pedersen e Poseidon
- Registries separados (`PedersenParams`, `PoseidonParams`) espelham controles de ciclo de vida de verifiers, cada um com `params_id`, hashes de geradores/constantes, ativacao, deprecacao e withdraw heights.
- Commitments e hashes separam dominios por `params_id` para que a rotacao de parametros nunca reutilize padroes de bits de sets deprecados; o ID e embutido em commitments de note e tags de dominio de nullifier.
- Circuitos suportam selecao multi-parametro em tempo de verificacao; sets de parametros deprecados permanecem spendable ate `deprecation_height`, e sets retirados sao rejeitados exatamente em `withdraw_height`.

## Ordenacao determinista e nullifiers
- Cada asset mantem um `CommitmentTree` com `next_leaf_index`; blocos acrescentam commitments em ordem determinista: iterar transacoes na ordem do bloco; dentro de cada transacao iterar outputs shielded por `output_idx` serializado ascendente.
- `note_position` e derivado dos offsets da arvore mas **nao** faz parte do nullifier; ele so alimenta paths de membership dentro do witness da proof.
- A estabilidade do nullifier sob reorgs e garantida pelo design PRF; o input PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e anchors referenciam roots Merkle historicos limitados por `max_anchor_age_blocks`.

## Fluxo do ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requer politica de asset `Convertible` ou `ShieldedOnly`; admission checa autoridade do asset, recupera `params_id` atual, amostra `rho`, emite commitment, atualiza a arvore Merkle.
   - Emite `ConfidentialEvent::Shielded` com o novo commitment, delta de Merkle root e hash de chamada da transacao para audit trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall VM verifica proof usando a entrada do registry; o host garante nullifiers nao usados, commitments anexados deterministicamente e anchor recente.
   - O ledger registra entradas `NullifierSet`, armazena payloads encriptados para recipients/auditores e emite `ConfidentialEvent::Transferred` resumindo nullifiers, outputs ordenados, hash de proof e Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Disponivel apenas para assets `Convertible`; a proof valida que o valor da note iguala o montante revelado, o ledger credita balance transparente e queima a note shielded marcando o nullifier como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o montante publico, nullifiers consumidos, identificadores de proof e hash de chamada da transacao.

## Adicoes ao data model
- `ConfidentialConfig` (nova secao de config) com flag de habilitacao, `assume_valid`, knobs de gas/limites, janela de anchor, backend de verifier.
- `ConfidentialNote`, `ConfidentialTransfer`, e `ConfidentialMint` schemas Norito com byte de versao explicito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envolve bytes de memo AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, com default `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para o layout XChaCha20-Poly1305.
- Vectores canonicos de key-derivation vivem em `docs/source/confidential_key_vectors.json`; tanto o CLI quanto o endpoint Torii regressam contra esses fixtures.
- `asset::AssetDefinition` ganha `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste o binding `(backend, name, commitment)` para verifiers de transfer/unshield; a execucao rejeita proofs cujo verifying key referenciado ou inline nao corresponde ao commitment registrado.
- `CommitmentTree` (por asset com frontier checkpoints), `NullifierSet` com chave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armazenados em world state.
- Mempool mantem estruturas transitorias `NullifierIndex` e `AnchorIndex` para deteccao precoce de duplicados e checks de idade de anchor.
- Updates de schema Norito incluem ordering canonico para public inputs; tests de round-trip garantem determinismo de encoding.
- Roundtrips de encrypted payload ficam fixados via unit tests (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de acompanhamento vao anexar transcripts AEAD canonicos para auditores. `norito.md` documenta o header on-wire para o envelope.

## Integracao IVM e syscall
- Introduzir syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, e o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - O syscall carrega metadata do verifier do registry, aplica limites de tamanho/tempo, cobra gas determinista, e so aplica o delta se a proof tiver sucesso.
- O host expoe o trait read-only `ConfidentialLedger` para recuperar snapshots de Merkle root e estado de nullifier; a biblioteca Kotodama fornece helpers de assembly de witness e validacao de schema.
- Docs de pointer-ABI foram atualizados para esclarecer layout do buffer de proof e handles de registry.

## Negociacao de capacidades de node
- O handshake anuncia `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Participacao de validadores requer `confidential.enabled=true`, `assume_valid=false`, identificadores de backend do verifier identicos e digests que correspondem; mismatches falham o handshake com `HandshakeConfidentialMismatch`.
- Config suporta `assume_valid` apenas para observers: quando desabilitado, encontrar instrucoes confidenciais gera `UnsupportedInstruction` determinista sem panic; quando habilitado, observers aplicam deltas declarados sem verificar proofs.
- Mempool rejeita transacoes confidenciais se a capacidade local estiver desabilitada. Filtros de gossip evitam enviar transacoes shielded para peers incompativeis enquanto encaminham cegamente IDs de verifier desconhecidos dentro de limites de tamanho.

### Matriz de handshake

| Anuncio remoto | Resultado para nodes validadores | Notas de operador |
|----------------------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend match, digest match | Aceito | O peer chega ao estado `Ready` e participa de proposta, voto e RBC fan-out. Nenhuma acao manual requerida. |
| `enabled=true`, `assume_valid=false`, backend match, digest stale ou ausente | Rejeitado (`HandshakeConfidentialMismatch`) | O remoto deve aplicar ativacoes pendentes de registry/parametros ou aguardar o `activation_height` programado. Ate corrigir, o node segue descobrivel mas nunca entra na rotacao de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeitado (`HandshakeConfidentialMismatch`) | Validadores requerem verificacao de proofs; configure o remoto como observer com Torii-only ingress ou mude `assume_valid=false` apos habilitar verificacao completa. |
| `enabled=false`, campos omitidos (build desatualizado), ou backend de verifier diferente | Rejeitado (`HandshakeConfidentialMismatch`) | Peers desatualizados ou parcialmente atualizados nao podem entrar na rede de consenso. Atualize para o release atual e garanta que o tuple backend + digest corresponde antes de reconectar. |

Observers que intencionalmente pulam verificacao de proofs nao devem abrir conexoes de consenso contra validadores com capability gates. Eles ainda podem ingerir blocos via Torii ou APIs de arquivo, mas a rede de consenso os rejeita ate anunciarem capacidades compativeis.

### Politica de pruning de reveal e retencao de nullifier

Ledgers confidenciais devem reter historico suficiente para provar frescor de notes e reproduzir auditorias de governance. A politica default, aplicada por `ConfidentialLedger`, e:

- **Retencao de nullifiers:** manter nullifiers gastos por um *minimo* de `730` dias (24 meses) apos a altura de gasto, ou a janela regulatoria obrigatoria se for maior. Operadores podem estender a janela via `confidential.retention.nullifier_days`. Nullifiers mais novos que a janela DEVEM permanecer consultaveis via Torii para que auditores provem ausencia de double-spend.
- **Pruning de reveals:** reveals transparentes (`RevealConfidential`) podam commitments associados imediatamente apos o bloco finalizar, mas o nullifier consumido continua sujeito a regra de retencao acima. Eventos `ConfidentialEvent::Unshielded` registram o montante publico, recipient e hash de proof para que reconstruir reveals historicos nao exija o ciphertext podado.
- **Frontier checkpoints:** frontiers de commitment mantem checkpoints rolling cobrindo o maior entre `max_anchor_age_blocks` e a janela de retencao. Nodes compactam checkpoints antigos apenas depois que todos os nullifiers no intervalo expiram.
- **Remediacao de digest stale:** se `HandshakeConfidentialMismatch` ocorrer por drift de digest, operadores devem (1) verificar que as janelas de retencao de nullifiers estao alinhadas no cluster, (2) rodar `iroha_cli confidential verify-ledger` para regenerar o digest contra o conjunto de nullifiers retidos, e (3) redeployar o manifest atualizado. Nullifiers podados prematuramente devem ser restaurados do cold storage antes de reingressar na rede.

Documente overrides locais no runbook de operacoes; politicas de governance que estendem a janela de retencao devem atualizar configuracao de node e planos de storage de arquivo em lockstep.

### Fluxo de eviction e recovery

1. Durante o dial, `IrohaNetwork` compara as capacidades anunciadas. Qualquer mismatch levanta `HandshakeConfidentialMismatch`; a conexao e fechada e o peer permanece na fila de discovery sem ser promovido a `Ready`.
2. A falha aparece no log do servico de rede (inclui digest remoto e backend), e Sumeragi nunca agenda o peer para proposta ou voto.
3. Operadores remediam alinhando registries de verifier e conjuntos de parametros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou programando `next_conf_features` com um `activation_height` acordado. Uma vez que o digest coincide, o proximo handshake tem sucesso automaticamente.
4. Se um peer stale consegue difundir um bloco (por exemplo, via replay de arquivo), validadores o rejeitam deterministicamente com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, mantendo o estado do ledger consistente na rede.

### Fluxo de handshake seguro contra replay

1. Cada tentativa outbound aloca material de chave Noise/X25519 novo. O payload de handshake assinado (`handshake_signature_payload`) concatena as chaves publicas efemeras local e remota, o endereco de socket anunciado codificado em Norito e, quando compilado com `handshake_chain_id`, o identificador de chain. A mensagem e encriptada com AEAD antes de sair do node.
2. O responder recomputa o payload com a ordem de chaves peer/local invertida e verifica a assinatura Ed25519 embutida em `HandshakeHelloV1`. Como ambas as chaves efemeras e o endereco anunciado fazem parte do dominio de assinatura, replay de uma mensagem capturada contra outro peer ou recuperar uma conexao stale falha deterministicamente.
3. Flags de capacidade confidencial e o `ConfidentialFeatureDigest` viajam dentro de `HandshakeConfidentialMeta`. O receptor compara o tuple `{ enabled, assume_valid, verifier_backend, digest }` com seu `ConfidentialHandshakeCaps` local; qualquer mismatch sai cedo com `HandshakeConfidentialMismatch` antes de o transporte transitar para `Ready`.
4. Operadores DEVEM recomputar o digest (via `compute_confidential_feature_digest`) e reiniciar nodes com registries/politicas atualizadas antes de reconectar. Peers anunciando digests antigos continuam falhando o handshake, evitando que estado stale reentre no set validador.
5. Sucessos e falhas de handshake atualizam contadores padrao `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomia de erros) e emitem logs estruturados com o peer ID remoto e o fingerprint do digest. Monitore esses indicadores para detectar replays ou misconfiguracoes durante o rollout.

## Key management e payloads
- Hierarquia de derivacao por account:
  - `sk_spend` -> `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Payloads de notes encriptadas usam AEAD com shared keys derivadas por ECDH; view keys de auditor opcionais podem ser anexadas a outputs conforme a politica do asset.
- Adicoes ao CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, tooling de auditor para descriptografar memos, e o helper `iroha zk envelope` para produzir/inspecionar envelopes Norito offline. Torii expoe o mesmo fluxo de derivacao via `POST /v1/confidential/derive-keyset`, retornando formas hex e base64 para que wallets busquem hierarquias de chave programaticamente.

## Gas, limites e controles DoS
- Schedule de gas determinista:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas por public input.
  - `5` gas por proof byte, mais cargos por nullifier (`300`) e por commitment (`500`).
  - Operadores podem sobrescrever essas constantes via configuracao do node (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); mudancas propagam no startup ou no hot-reload da camada de config e sao aplicadas deterministicamente no cluster.
- Limites duros (defaults configuraveis):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs que excedem `verify_timeout_ms` abortam a instrucao deterministicamente (ballots de governance emitem `proof verification exceeded timeout`, `VerifyProof` retorna erro).
- Quotas adicionais garantem liveness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, e `max_public_inputs` limitam block builders; `reorg_depth_bound` (>= `max_anchor_age_blocks`) governa a retencao de frontier checkpoints.
- A execucao runtime agora rejeita transacoes que excedem esses limites por transacao ou por bloco, emitindo erros `InvalidParameter` deterministas e mantendo o estado do ledger inalterado.
- Mempool prefiltra transacoes confidenciais por `vk_id`, tamanho de proof e idade de anchor antes de invocar o verifier para manter uso de recursos limitado.
- A verificacao para deterministicamente em timeout ou violacao de limite; transacoes falham com erros explicitos. Backends SIMD sao opcionais mas nao alteram o accounting de gas.

### Baselines de calibracao e gates de aceitacao
- **Plataformas de referencia.** Rodadas de calibracao DEVEM cobrir os tres perfis abaixo. Rodadas sem todos os perfis sao rejeitadas na review.

  | Perfil | Arquitetura | CPU / Instancia | Flags de compilador | Proposito |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores piso sem intrinsics vetoriais; usado para ajustar tabelas de custo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | release default | Valida o path AVX2; checa se os ganhos SIMD ficam dentro da tolerancia do gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | release default | Garante que o backend NEON permanece determinista e alinhado aos schedules x86. |

- **Benchmark harness.** Todos os relatorios de calibracao de gas DEVEM ser produzidos com:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar o fixture determinista.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` sempre que custos de opcode do VM mudarem.

- **Randomness fixa.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de rodar benches para que `iroha_test_samples::gen_account_in` mude para o caminho determinista `KeyPair::from_seed`. O harness imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; se a variavel faltar, a review DEVE falhar. Qualquer utilidade nova de calibracao deve continuar honrando esta env var ao introduzir randomness auxiliar.

- **Captura de resultados.**
  - Upload de summaries Criterion (`target/criterion/**/raw.csv`) para cada perfil no artefato de release.
  - Armazenar metricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Confidential Gas Calibration ledger](./confidential-gas-calibration) junto com o commit git e a versao de compilador usada.
  - Manter os dois ultimos baselines por perfil; apagar snapshots mais antigos uma vez validado o relatorio mais novo.

- **Tolerancias de aceitacao.**
  - Deltas de gas entre `baseline-simd-neutral` e `baseline-avx2` DEVEM permanecer <= +/-1.5%.
  - Deltas de gas entre `baseline-simd-neutral` e `baseline-neon` DEVEM permanecer <= +/-2.0%.
  - Propostas de calibracao que excedem esses thresholds requerem ajustes de schedule ou um RFC explicando a discrepancia e a mitigacao.

- **Checklist de review.** Submitters sao responsaveis por:
  - Incluir `uname -a`, trechos de `/proc/cpuinfo` (model, stepping), e `rustc -Vv` no log de calibracao.
  - Verificar que `IROHA_CONF_GAS_SEED` aparece na saida do bench (as benches imprimem a seed ativa).
  - Garantir que feature flags do pacemaker e do verifier confidencial espelhem producao (`--features confidential,telemetry` ao rodar benches com Telemetry).

## Config e operacoes
- `iroha_config` adiciona a secao `[confidential]`:
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
- Telemetria emite metricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total`, sem expor dados em claro.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Estrategia de testes
- Determinismo: randomizacao de transacoes dentro de blocos gera Merkle roots e sets de nullifier identicos.
- Resiliencia a reorg: simular reorgs multi-bloco com anchors; nullifiers permanecem estaveis e anchors stale sao rejeitados.
- Invariantes de gas: verificar uso de gas identico entre nodes com e sem aceleracao SIMD.
- Boundary testing: proofs no teto de tamanho/gas, max in/out counts, enforcement de timeout.
- Lifecycle: operacoes de governance para ativacao/deprecacao de verifier e parametros, tests de gasto apos rotacao.
- Policy FSM: transicoes permitidas/negadas, delays de transicao pendente e rejeicao de mempool perto de alturas efetivas.
- Emergencias de registry: emergency withdrawal congela assets afetados em `withdraw_height` e rejeita proofs depois.
- Capability gating: validadores com `conf_features` divergentes rejeitam blocos; observers com `assume_valid=true` acompanham sem afetar consenso.
- Equivalencia de estado: nodes validator/full/observer produzem roots de estado identicos na cadeia canonica.
- Fuzzing negativo: proofs malformadas, payloads superdimensionados e colisoes de nullifier sao rejeitados deterministicamente.

## Migracao
- Rollout com feature flag: ate Phase C3 terminar, `enabled` default a `false`; nodes anunciam capacidades antes de entrar no set validador.
- Assets transparentes nao sao afetados; instrucoes confidenciais requerem entradas de registry e negociacao de capacidades.
- Nodes compilados sem suporte confidencial rejeitam blocos relevantes deterministicamente; nao podem entrar no set validador mas podem operar como observers com `assume_valid=true`.
- Genesis manifests incluem entradas iniciais de registry, sets de parametros, politicas confidenciais para assets e keys de auditor opcionais.
- Operadores seguem runbooks publicados para rotacao de registry, transicoes de politica e emergency withdrawal para manter upgrades deterministas.

## Trabalho pendente
- Benchmark de parametros Halo2 (tamanho de circuito, estrategia de lookup) e registrar resultados no playbook de calibracao para que defaults de gas/timeout sejam atualizados junto ao proximo refresh de `confidential_assets_calibration.md`.
- Finalizar politicas de disclosure de auditor e APIs de selective-viewing associadas, conectando o workflow aprovado em Torii assim que o draft de governance for assinado.
- Estender o esquema de witness encryption para cobrir outputs multi-recipient e memos em batch, documentando o formato do envelope para implementadores de SDK.
- Comissionar uma revisao de seguranca externa de circuitos, registries e procedimentos de rotacao de parametros e arquivar os achados ao lado dos relatorios internos de auditoria.
- Especificar APIs de reconciliacao de spentness para auditores e publicar guia de escopo de view-key para que vendors de wallet implementem as mesmas semanticas de atestacao.

## Phasing de implementacao
1. **Phase M0 - Stop-Ship Hardening**
   - [x] Derivacao de nullifier segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com ordering determinista de commitments aplicado nas atualizacoes do ledger.
   - [x] Execucao aplica limites de tamanho de proof e quotas confidenciais por transacao/por bloco, rejeitando transacoes fora de budget com erros deterministas.
   - [x] Handshake P2P anuncia `ConfidentialFeatureDigest` (backend digest + fingerprints de registry) e falha mismatches deterministicamente via `HandshakeConfidentialMismatch`.
   - [x] Remover panics em paths de execucao confidencial e adicionar role gating para nodes incompativeis.
   - [ ] Aplicar budgets de timeout do verifier e limites de profundidade de reorg para frontier checkpoints.
     - [x] Budgets de timeout de verificacao aplicados; proofs que excedem `verify_timeout_ms` agora falham deterministicamente.
     - [x] Frontier checkpoints agora respeitam `reorg_depth_bound`, podando checkpoints mais antigos que a janela configurada e mantendo snapshots deterministas.
   - Introduzir `AssetConfidentialPolicy`, policy FSM e gates de enforcement para instrucoes mint/transfer/reveal.
   - Commit `conf_features` nos headers de bloco e recusar participacao de validadores quando digests de registry/parametros divergem.
2. **Phase M1 - Registries e parametros**
   - Entregar registries `ZkVerifierEntry`, `PedersenParams`, e `PoseidonParams` com ops de governance, ancoragem de genesis e gestao de cache.
   - Conectar syscall para exigir lookups de registry, gas schedule IDs, schema hashing, e checks de tamanho.
   - Enviar formato de payload encriptado v1, vetores de derivacao de keys para wallet, e suporte CLI para gestao de chaves confidenciais.
3. **Phase M2 - Gas e performance**
   - Implementar schedule de gas determinista, contadores por bloco e harnesses de benchmark com telemetria (latencia de verificacao, tamanhos de proof, rejeicoes de mempool).
   - Endurecer CommitmentTree checkpoints, carga LRU e indices de nullifier para workloads multi-asset.
4. **Phase M3 - Rotacao e tooling de wallet**
   - Habilitar aceitacao de proofs multi-parametro e multi-versao; suportar ativacao/deprecacao guiada por governance com runbooks de transicao.
   - Entregar fluxos de migracao SDK/CLI, workflows de scan de auditor e tooling de reconciliacao de spentness.
5. **Phase M4 - Audit e ops**
   - Fornecer workflows de keys de auditor, APIs de selective disclosure, e runbooks operacionais.
   - Agendar revisao externa de criptografia/seguranca e publicar achados em `status.md`.

Cada phase atualiza milestones do roadmap e tests associados para manter garantias de execucao determinista na rede blockchain.
