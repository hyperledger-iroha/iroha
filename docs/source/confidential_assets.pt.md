---
lang: pt
direction: ltr
source: docs/source/confidential_assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969ffd4cee6ee4880d5f754fb36adaf30dde532a29e4c6397cf0f358438bb57e
source_last_modified: "2026-01-22T15:38:30.657840+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Ativos confidenciais e design de transferência ZK

## Motivação
- Forneça fluxos de ativos protegidos para que os domínios possam preservar a privacidade transacional sem alterar a circulação transparente.
- Fornecer aos auditores e operadores controles de ciclo de vida (ativação, rotação, revogação) para circuitos e parâmetros criptográficos.

## Modelo de ameaça
- Os validadores são honestos, mas curiosos: eles executam o consenso fielmente, mas tentam inspecionar o razão/estado.
- Os observadores da rede veem dados de blocos e transações fofocadas; nenhuma suposição de canais de fofoca privados.
- Fora do escopo: análise de tráfego off-ledger, adversários quânticos (rastreados separadamente no roteiro PQ), ataques de disponibilidade de razão.

## Visão geral do projeto
- Os ativos podem declarar um *pool protegido* além dos saldos transparentes existentes; a circulação protegida é representada por meio de compromissos criptográficos.
- As notas encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independente da ordenação das notas.
  - Carga criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transações transportam cargas úteis `ConfidentialTransfer` codificadas em Norito contendo:
  - Entradas públicas: âncora Merkle, anuladores, novos compromissos, id do ativo, versão do circuito.
  - Cargas criptografadas para destinatários e auditores opcionais.
  - Prova de conhecimento zero atestando conservação, propriedade e autorização de valor.
- Verificar se chaves e conjuntos de parâmetros são controlados através de registros on-ledger com janelas de ativação; os nós se recusam a validar provas que façam referência a entradas desconhecidas ou revogadas.
- Os cabeçalhos de consenso comprometem-se com o resumo do recurso confidencial ativo, de modo que os blocos só sejam aceitos quando o estado do registro e do parâmetro corresponder.
- A construção da prova usa uma pilha Halo2 (Plonkish) sem configuração confiável; Groth16 ou outras variantes do SNARK não são intencionalmente suportadas na v1.

### Jogos Determinísticos

Os envelopes de memorandos confidenciais agora são fornecidos com um acessório canônico em `fixtures/confidential/encrypted_payload_v1.json`. O conjunto de dados captura um envelope v1 positivo mais amostras negativas malformadas para que os SDKs possam afirmar a paridade de análise. Os testes de modelo de dados Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e o conjunto Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) carregam o fixture diretamente, garantindo que a codificação Norito, as superfícies de erro e a cobertura de regressão permaneçam alinhadas à medida que o codec evolui.

SDKs Swift agora podem emitir instruções de escudo sem cola JSON personalizada: construa um
`ShieldRequest` com compromisso de nota de 32 bytes, carga útil criptografada e metadados de débito,
em seguida, ligue para `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para assinar e retransmitir o
transação acima de `/v1/pipeline/transactions`. O auxiliar valida a duração do compromisso,
encadeia `ConfidentialEncryptedPayload` no codificador Norito e espelha o `zk::Shield`
layout descrito abaixo para que as carteiras permaneçam em sincronia com o Rust.## Compromissos de consenso e controle de capacidade
- Cabeçalhos de bloco expõem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o resumo participa do hash de consenso e deve ser igual à visualização do registro local para aceitação do bloco.
- A governança pode preparar atualizações programando `next_conf_features` com um futuro `activation_height`; até essa altura, os produtores de blocos devem continuar a emitir o resumo anterior.
- Os nós validadores DEVEM operar com `confidential.enabled = true` e `assume_valid = false`. As verificações de inicialização recusam-se a ingressar no conjunto de validadores se uma das condições falhar ou se o `conf_features` local divergir.
- Os metadados de handshake P2P agora incluem `{ enabled, assume_valid, conf_features }`. Os pares que anunciam recursos não suportados são rejeitados com `HandshakeConfidentialMismatch` e nunca entram na rotação de consenso.
- Observadores não validadores podem definir `assume_valid = true`; eles aplicam cegamente deltas confidenciais, mas não influenciam a segurança do consenso.## Políticas de Ativos
- Cada definição de ativo carrega um `AssetConfidentialPolicy` definido pelo criador ou via governança:
  - `TransparentOnly`: modo padrão; somente instruções transparentes (`MintAsset`, `TransferAsset`, etc.) são permitidas e operações blindadas são rejeitadas.
  - `ShieldedOnly`: todas as emissões e transferências deverão utilizar instruções confidenciais; `RevealConfidential` é proibido, portanto os saldos nunca aparecem publicamente.
  - `Convertible`: os titulares podem mover valor entre representações transparentes e blindadas usando as instruções da rampa de ativação/desativação abaixo.
- As políticas seguem um FSM restrito para evitar a perda de fundos:
  - `TransparentOnly → Convertible` (habilitação imediata de pool blindado).
  - `TransparentOnly → ShieldedOnly` (requer transição pendente e janela de conversão).
  - `Convertible → ShieldedOnly` (atraso mínimo imposto).
  - `ShieldedOnly → Convertible` (plano de migração necessário para que as notas protegidas permaneçam gastáveis).
  - `ShieldedOnly → TransparentOnly` não é permitido, a menos que o pool protegido esteja vazio ou a governança codifique uma migração que desproteja notas pendentes.
- As instruções de governança definem `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` por meio do ISI `ScheduleConfidentialPolicyTransition` e podem abortar alterações programadas com `CancelConfidentialPolicyTransition`. A validação do Mempool garante que nenhuma transação ultrapasse a altura da transição e a inclusão falhará deterministicamente se uma verificação de política mudar no meio do bloco.
- As transições pendentes são aplicadas automaticamente quando um novo bloco é aberto: quando a altura do bloco entra na janela de conversão (para atualizações `ShieldedOnly`) ou atinge o `effective_height` programado, o tempo de execução atualiza `AssetConfidentialPolicy`, atualiza os metadados `zk.policy` e limpa a entrada pendente. Se o fornecimento transparente permanecer quando uma transição `ShieldedOnly` amadurecer, o tempo de execução abortará a alteração e registrará um aviso, deixando o modo anterior intacto.
- Os botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` impõem aviso mínimo e períodos de carência para permitir que as carteiras convertam notas em torno do switch.
- `pending_transition.transition_id` funciona como identificador de auditoria; a governança deve citá-lo ao finalizar ou cancelar transições para que os operadores possam correlacionar relatórios de rampa de entrada/saída.
- `policy_transition_window_blocks` tem como padrão 720 (≈12 horas com tempo de bloqueio de 60 s). Os nós restringem solicitações de governança que tentam um aviso mais curto.
- Manifestos Genesis e fluxos CLI revelam políticas atuais e pendentes. A lógica de admissão lê a política em tempo de execução para confirmar que cada instrução confidencial é autorizada.
- Lista de verificação de migração — consulte “Sequenciamento de migração” abaixo para obter o plano de atualização em etapas que o Milestone M0 rastreia.

#### Monitorando transições via ToriiCarteiras e auditores pesquisam `GET /v1/confidential/assets/{definition_id}/transitions` para inspecionar
o `AssetConfidentialPolicy` ativo. A carga JSON sempre inclui o canônico
ID do ativo, a última altura do bloco observada, o `current_mode` da política, o modo que é
efetivo nessa altura (as janelas de conversão relatam temporariamente `Convertible`), e o
identificadores de parâmetro `vk_set_hash`/Poseidon/Pedersen esperados. Os consumidores do Swift SDK podem ligar
`ToriiClient.getConfidentialAssetPolicy` para receber os mesmos dados que DTOs digitados sem
decodificação manuscrita. Quando uma transição de governação está pendente, a resposta também incorpora:

- `transition_id` — identificador de auditoria retornado por `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e o derivado `window_open_height` (o bloco onde as carteiras devem
  iniciar a conversão para cortes ShieldedOnly).

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

Uma resposta `404` indica que não existe definição de ativo correspondente. Quando nenhuma transição é
programado o campo `pending_transition` é `null`.

### Máquina de estado de política| Modo atual | Próximo modo | Pré-requisitos | Manuseio em altura efetiva | Notas |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | A governança ativou entradas de registro de verificador/parâmetro. Envie `ScheduleConfidentialPolicyTransition` com `effective_height ≥ current_height + policy_transition_delay_blocks`. | A transição é executada exatamente em `effective_height`; pool protegido fica disponível imediatamente.                   | Caminho padrão para permitir a confidencialidade enquanto mantém fluxos transparentes.               |
| Somente Transparente | Somente blindado | O mesmo que acima, mais `policy_transition_window_blocks ≥ 1`.                                                         | O tempo de execução insere automaticamente `Convertible` em `effective_height - policy_transition_window_blocks`; muda para `ShieldedOnly` em `effective_height`. | Fornece janela de conversão determinística antes que as instruções transparentes sejam desativadas.   |
| Conversível | Somente blindado | Transição agendada com `effective_height ≥ current_height + policy_transition_delay_blocks`. A governança DEVE ser certificada (`transparent_supply == 0`) por meio de metadados de auditoria; o tempo de execução impõe isso na transição. | Semântica de janela idêntica à acima. Se o fornecimento transparente for diferente de zero em `effective_height`, a transição será abortada com `PolicyTransitionPrerequisiteFailed`. | Bloqueia o ativo em circulação totalmente confidencial.                                     |
| Somente blindado | Conversível | Transição programada; nenhuma retirada de emergência ativa (`withdraw_height` não definido).                                    | O estado muda em `effective_height`; as rampas reveladas reabrem enquanto as notas protegidas permanecem válidas.                           | Usado para janelas de manutenção ou revisões de auditores.                                          |
| Somente blindado | Somente Transparente | A governança deve comprovar `shielded_supply == 0` ou apresentar um plano `EmergencyUnshield` assinado (é necessária a assinatura do auditor). | O tempo de execução abre uma janela `Convertible` antes de `effective_height`; no auge, as instruções confidenciais falham e o ativo retorna ao modo somente transparente. | Saída de último recurso. A transição é cancelada automaticamente se alguma nota confidencial for gasta durante a janela. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` limpa alterações pendentes.                                                        | `pending_transition` removido imediatamente.                                                                          | Mantém o status quo; mostrado para completude.                                             |As transições não listadas acima são rejeitadas durante o envio da governança. O tempo de execução verifica os pré-requisitos antes de aplicar uma transição agendada; a falha nas pré-condições empurra o ativo de volta ao seu modo anterior e emite `PolicyTransitionPrerequisiteFailed` por meio de telemetria e eventos de bloqueio.

### Sequenciamento de migração

2. **Preparar a transição:** Envie `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeite `policy_transition_delay_blocks`. Ao avançar para `ShieldedOnly`, especifique uma janela de conversão (`window ≥ policy_transition_window_blocks`).
3. **Publicar orientação do operador:** Registre o `transition_id` retornado e distribua um runbook de rampa de entrada/saída. Carteiras e auditores assinam `/v1/confidential/assets/{id}/transitions` para saber a altura de abertura da janela.
4. **Aplicação de janela:** Quando a janela é aberta, o tempo de execução alterna a política para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }` e começa a rejeitar solicitações de governança conflitantes.
5. **Finalizar ou abortar:** Em `effective_height`, o tempo de execução verifica os pré-requisitos de transição (zero fornecimento transparente, sem retiradas emergenciais, etc.). O sucesso muda a política para o modo solicitado; a falha emite `PolicyTransitionPrerequisiteFailed`, limpa a transição pendente e deixa a política inalterada.
6. **Atualizações de esquema:** após uma transição bem-sucedida, a governança altera a versão do esquema de ativos (por exemplo, `asset_definition.v2`) e as ferramentas CLI exigem `confidential_policy` ao serializar manifestos. Os documentos de atualização do Genesis instruem os operadores a adicionar configurações de política e impressões digitais de registro antes de reiniciar os validadores.

Novas redes que começam com a confidencialidade habilitada codificam a política desejada diretamente na gênese. Eles ainda seguem a lista de verificação acima ao alterar os modos pós-lançamento, para que as janelas de conversão permaneçam determinísticas e as carteiras tenham tempo para se ajustar.

### Controle de versão e ativação do manifesto Norito- Os manifestos do Genesis DEVEM incluir um `SetParameter` para a chave `confidential_registry_root` personalizada. A carga útil é Norito JSON correspondente a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omita o campo (`null`) quando nenhuma entrada do verificador estiver ativa, caso contrário, forneça uma string hexadecimal de 32 bytes (`0x…`) igual ao hash produzido por `compute_vk_set_hash` nas instruções do verificador enviadas no manifesto. Os nós se recusam a iniciar se o parâmetro estiver faltando ou se o hash discordar das gravações codificadas do registro.
- O `ConfidentialFeatureDigest::conf_rules_version` on-wire incorpora a versão do layout do manifesto. Para redes v1 DEVE permanecer `Some(1)` e é igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quando o conjunto de regras evoluir, supere a constante, regenere os manifestos e implemente os binários em sincronia; misturar versões faz com que os validadores rejeitem blocos com `ConfidentialFeatureDigestMismatch`.
- Os manifestos de ativação DEVEM agrupar atualizações de registro, alterações no ciclo de vida dos parâmetros e transições de políticas para que o resumo permaneça consistente:
  1. Aplique as mutações de registro planejadas (`Publish*`, `Set*Lifecycle`) em uma visualização de estado offline e calcule o resumo pós-ativação com `compute_confidential_feature_digest`.
  2. Emita `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` usando o hash calculado para que os pares atrasados ​​possam recuperar o resumo correto, mesmo que percam instruções intermediárias do registro.
  3. Anexe as instruções `ScheduleConfidentialPolicyTransition`. Cada instrução deve citar o `transition_id` emitido pela governança; manifestos que esquecerem serão rejeitados pelo tempo de execução.
  4. Persista os bytes de manifesto, uma impressão digital SHA-256 e o ​​resumo usado no plano de ativação. Os operadores verificam todos os três artefatos antes de votar a entrada em vigor do manifesto para evitar partições.
- Quando as implementações exigirem uma transição adiada, registre a altura desejada em um parâmetro personalizado complementar (por exemplo, `custom.confidential_upgrade_activation_height`). Isso dá aos auditores uma prova codificada em Norito de que os validadores honraram a janela de aviso antes que a alteração do resumo entrasse em vigor.## Verificador e ciclo de vida dos parâmetros
### Registro ZK
- O razão armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`, onde `proving_system` está atualmente fixado em `Halo2`.
- Os pares `(circuit_id, version)` são globalmente únicos; o registro mantém um índice secundário para pesquisas por metadados de circuito. As tentativas de registrar um par duplicado são rejeitadas durante a admissão.
- `circuit_id` não deve estar vazio e `public_inputs_schema_hash` deve ser fornecido (normalmente um hash Blake2b-32 da codificação canônica de entrada pública do verificador). A admissão rejeita registros que omitem esses campos.
- As instruções de governança incluem:
  - `PUBLISH` para adicionar uma entrada `Proposed` apenas com metadados.
  - `ACTIVATE { vk_id, activation_height }` para agendar a ativação de entrada em um limite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcação da altura final onde as provas poderão referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergência; os ativos afetados congelam os gastos confidenciais após o pico de retirada até que novas entradas sejam ativadas.
- Os manifestos Genesis emitem automaticamente um parâmetro personalizado `confidential_registry_root` cujo `vk_set_hash` corresponde às entradas ativas; a validação compara esse resumo com o estado do registro local antes que um nó possa ingressar no consenso.
- O registro ou atualização de um verificador requer um `gas_schedule_id`; a verificação impõe que a entrada do registro seja `Active`, presente no índice `(circuit_id, version)`, e que as provas Halo2 forneçam um `OpenVerifyEnvelope` cujo `circuit_id`, `vk_hash` e `public_inputs_schema_hash` correspondem ao registro do registro.

### Provando Chaves
- As chaves de prova permanecem fora do registro, mas são referenciadas por identificadores endereçados ao conteúdo (`pk_cid`, `pk_hash`, `pk_len`) publicados junto com os metadados do verificador.
- Os SDKs da carteira buscam dados PK, verificam hashes e armazenam em cache localmente.

### Parâmetros de Pedersen e Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) controles de ciclo de vida do verificador de espelho, cada um com `params_id`, hashes de geradores/constantes, ativação, depreciação e alturas de retirada.

## Ordenação Determinística e Anuladores
- Cada ativo mantém um `CommitmentTree` com `next_leaf_index`; os blocos acrescentam compromissos em ordem determinística: iteram transações em ordem de blocos; dentro de cada transação, itere as saídas protegidas aumentando o `output_idx` serializado.
- `note_position` é derivado dos deslocamentos da árvore, mas **não** faz parte do anulador; apenas alimenta caminhos de adesão dentro da testemunha de prova.
- A estabilidade do anulador sob reorganizações é garantida pelo design do PRF; a entrada PRF liga `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` e as âncoras fazem referência às raízes históricas de Merkle limitadas por `max_anchor_age_blocks`.## Fluxo do razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Requer política de ativos `Convertible` ou `ShieldedOnly`; a admissão verifica a autoridade do ativo, recupera `params_id` atual, amostras `rho`, emite compromisso, atualiza a árvore Merkle.
   - Emite `ConfidentialEvent::Shielded` com o novo compromisso, Merkle root delta e hash de chamada de transação para trilhas de auditoria.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - VM syscall verifica a prova usando entrada de registro; host garante anuladores não utilizados, compromissos anexados deterministicamente, âncora é recente.
   - O Ledger registra entradas `NullifierSet`, armazena cargas criptografadas para destinatários/auditores e emite `ConfidentialEvent::Transferred` resumindo nulificadores, saídas ordenadas, hash de prova e raízes Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - Disponível apenas para ativos `Convertible`; a prova valida o valor da nota igual ao valor revelado, o razão credita o saldo transparente e queima a nota protegida marcando o anulador como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o valor público, anuladores consumidos, identificadores de prova e hash de chamada de transação.

## Adições ao modelo de dados
- `ConfidentialConfig` (nova seção de configuração) com sinalizador de ativação, `assume_valid`, botões de gás/limite, janela de âncora, backend do verificador.
- Esquemas `ConfidentialNote`, `ConfidentialTransfer` e `ConfidentialMint` Norito com byte de versão explícito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` agrupa bytes de memorando AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, padronizando `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para o layout XChaCha20-Poly1305.
- Vetores de derivação de chave canônica residem em `docs/source/confidential_key_vectors.json`; tanto o endpoint CLI quanto o Torii regridem em relação a esses equipamentos. Derivados voltados para carteira para a escada de gastos/anulador/visualização são publicados em `fixtures/confidential/keyset_derivation_v1.json` e exercidos pelos testes Rust + Swift SDK para garantir a paridade entre idiomas.
- `asset::AssetDefinition` ganha `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste a ligação `(backend, name, commitment)` para verificadores de transferência/desproteção; a execução rejeita provas cuja chave de verificação referenciada ou em linha não corresponde ao compromisso registrado e verifica as provas de transferência/desproteção em relação à chave de back-end resolvida antes de alterar o estado.
- `CommitmentTree` (por ativo com pontos de verificação de fronteira), `NullifierSet` codificado por `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armazenado no estado mundial.
- Mempool mantém estruturas transitórias `NullifierIndex` e `AnchorIndex` para detecção precoce de duplicatas e verificações de idade de âncora.
- As atualizações do esquema Norito incluem ordenação canônica para entradas públicas; testes de ida e volta garantem o determinismo da codificação.
- As viagens de ida e volta de carga útil criptografada são bloqueadas por meio de testes de unidade (`crates/iroha_data_model/src/confidential.rs`), e os vetores de derivação de chave de carteira acima ancoram as derivações de envelope AEAD para auditores. `norito.md` documenta o cabeçalho on-wire do envelope.## IVM Integração e Syscall
- Introduzir o syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` e `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - Syscall carrega metadados do verificador do registro, impõe limites de tamanho/tempo, cobra gás determinístico e aplica delta somente se a prova for bem-sucedida.
- O host expõe a característica `ConfidentialLedger` somente leitura para recuperar instantâneos de raiz Merkle e status de anulador; A biblioteca Kotodama fornece auxiliares de montagem de testemunha e validação de esquema.
- Documentos Pointer-ABI atualizados para esclarecer o layout do buffer de prova e os identificadores de registro.

## Negociação de capacidade do nó
- O aperto de mão anuncia `feature_bits.confidential` junto com um `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. A participação do validador requer `confidential.enabled=true`, `assume_valid=false`, identificadores de back-end do verificador idênticos e resumos correspondentes; incompatibilidades falham no handshake com `HandshakeConfidentialMismatch`.
- O Config suporta `assume_valid` apenas para nós observadores: quando desabilitado, encontrar instruções confidenciais produz `UnsupportedInstruction` determinístico sem pânico; quando habilitado, os observadores aplicam deltas de estado declarados sem verificar as provas.
- Mempool rejeita transações confidenciais se a capacidade local estiver desabilitada. Os filtros de fofoca evitam o envio de transações protegidas para pares sem capacidade de correspondência, enquanto encaminham IDs de verificador desconhecidos dentro dos limites de tamanho.

### Revelar política de retenção de poda e anulador

Os livros confidenciais devem reter histórico suficiente para comprovar a atualidade das notas e para
repetir auditorias orientadas pela governança. A política padrão, aplicada por
`ConfidentialLedger`, é:

- **Retenção do anulador:** mantenha os anuladores gastos por *mínimo* dias `730` (24
  meses) após a altura do gasto, ou a janela exigida pelo regulador, se for maior.
  Os operadores podem estender a janela via `confidential.retention.nullifier_days`.
  Nullifiers mais jovens que a janela de retenção DEVEM permanecer consultáveis via Torii para que
  os auditores podem provar ausência de gasto duplo.
- **Revelar poda:** revelações transparentes (`RevealConfidential`) podam o
  compromissos de notas associados imediatamente após a finalização do bloco, mas o
  o anulador consumido permanece sujeito à regra de retenção acima. Relacionado à revelação
  eventos (`ConfidentialEvent::Unshielded`) registram o valor público, destinatário,
  e hash de prova, portanto, a reconstrução de revelações históricas não requer a remoção
  texto cifrado.
- **Pontos de verificação de fronteira:** as fronteiras de compromisso mantêm pontos de verificação contínuos
  cobrindo o maior de `max_anchor_age_blocks` e a janela de retenção. Nós
  compactar pontos de verificação mais antigos somente depois que todos os anuladores dentro do intervalo expirarem.
- **Correção de resumo obsoleto:** se `HandshakeConfidentialMismatch` for gerado devido
  para digerir o desvio, os operadores devem (1) verificar se as janelas de retenção do anulador
  alinhar no cluster, (2) execute `iroha_cli app confidential verify-ledger` para
  regenerar o resumo em relação ao conjunto anulador retido e (3) reimplantar o
  manifesto atualizado. Quaisquer anuladores removidos prematuramente devem ser restaurados de
  armazenamento frio antes de retornar à rede.Documente substituições locais no runbook de operações; políticas de governança que se estendem
a janela de retenção deve atualizar a configuração do nó e os planos de armazenamento de arquivamento em
passo a passo.

### Fluxo de despejo e recuperação

1. Durante a discagem, `IrohaNetwork` compara os recursos anunciados. Qualquer incompatibilidade gera `HandshakeConfidentialMismatch`; a conexão é fechada e o peer permanece na fila de descoberta sem nunca ser promovido para `Ready`.
2. A falha é revelada através do log de serviço de rede (incluindo o resumo remoto e back-end), e Sumeragi nunca agenda o par para proposta ou votação.
3. Os operadores corrigem alinhando registros de verificador e conjuntos de parâmetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou preparando `next_conf_features` com um `activation_height` acordado. Assim que o resumo corresponder, o próximo handshake será bem-sucedido automaticamente.
4. Se um par obsoleto conseguir transmitir um bloco (por exemplo, por meio de reprodução de arquivamento), os validadores o rejeitarão deterministicamente com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, mantendo o estado do razão consistente em toda a rede.

### Fluxo de handshake seguro para repetição

1. Cada tentativa de saída aloca material de chave Noise/X25519 novo. A carga de handshake assinada (`handshake_signature_payload`) concatena as chaves públicas efêmeras locais e remotas, o endereço de soquete anunciado codificado em Norito e, quando compilado com `handshake_chain_id`, o identificador de cadeia. A mensagem é criptografada por AEAD antes de sair do nó.
2. O respondedor recalcula a carga útil com a ordem da chave peer/local invertida e verifica a assinatura Ed25519 incorporada em `HandshakeHelloV1`. Como tanto as chaves efêmeras quanto o endereço anunciado fazem parte do domínio de assinatura, a repetição de uma mensagem capturada em outro ponto ou a recuperação de uma conexão obsoleta falha na verificação deterministicamente.
3. Os sinalizadores de capacidade confidencial e o `ConfidentialFeatureDigest` viajam dentro do `HandshakeConfidentialMeta`. O receptor compara a tupla `{ enabled, assume_valid, verifier_backend, digest }` com seu `ConfidentialHandshakeCaps` configurado localmente; qualquer incompatibilidade sai antecipadamente com `HandshakeConfidentialMismatch` antes da transição de transporte para `Ready`.
4. Os operadores DEVEM recalcular o resumo (via `compute_confidential_feature_digest`) e reiniciar os nós com os registros/políticas atualizados antes de reconectar. Os pares que anunciam resumos antigos continuam falhando no handshake, evitando que o estado obsoleto entre novamente no conjunto de validadores.
5. Os sucessos e falhas do handshake atualizam os contadores `iroha_p2p::peer` padrão (`handshake_failure_count`, auxiliares de taxonomia de erros) e emitem entradas de log estruturadas marcadas com o ID do peer remoto e a impressão digital de resumo. Monitore esses indicadores para detectar tentativas de repetição ou configurações incorretas durante a implementação.## Gerenciamento de chaves e cargas úteis
- Hierarquia de derivação de chave por conta:
  - `sk_spend` → `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- Cargas de notas criptografadas usam AEAD com chaves compartilhadas derivadas de ECDH; chaves opcionais de visão do auditor podem ser anexadas às saídas por política de ativos.
- Adições CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ferramentas de auditoria para descriptografar memorandos e o auxiliar `iroha app zk envelope` para produzir/inspecionar envelopes de memorando Norito offline. Torii expõe o mesmo fluxo de derivação via `POST /v1/confidential/derive-keyset`, retornando os formatos hexadecimal e base64 para que as carteiras possam buscar hierarquias de chaves programaticamente.

## Controles de gás, limites e DoS
- Cronograma determinístico de gás:
  - Halo2 (Plonkish): gás base `250_000` + gás `2_000` por entrada pública.
  - Gás `5` por byte de prova, mais cobranças por anulador (`300`) e por compromisso (`500`).
  - Os operadores podem sobrescrever estas constantes através da configuração do nó (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); as alterações se propagam na inicialização ou quando a camada de configuração é recarregada a quente e são aplicadas deterministicamente em todo o cluster.
- Limites rígidos (padrões configuráveis):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. As provas que excedem `verify_timeout_ms` abortam a instrução deterministicamente (as cédulas de governança emitem `proof verification exceeded timeout`, `VerifyProof` retorna um erro).
- Cotas adicionais garantem atividade: construtores de blocos vinculados `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` e `max_public_inputs`; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) rege a retenção de pontos de verificação de fronteira.
- A execução em tempo de execução agora rejeita transações que excedem esses limites por transação ou por bloco, emitindo erros determinísticos `InvalidParameter` e deixando o estado do razão inalterado.
- O Mempool pré-filtra as transações confidenciais por `vk_id`, comprimento da prova e idade da âncora antes de invocar o verificador para manter o uso de recursos limitado.
- A verificação é interrompida deterministicamente em caso de timeout ou violação de limite; as transações falham com erros explícitos. Os backends SIMD são opcionais, mas não alteram a contabilidade do gás.

### Linhas de base de calibração e portas de aceitação
- **Plataformas de referência.** As execuções de calibração DEVEM abranger os três perfis de hardware abaixo. As execuções que não conseguem capturar todos os perfis são rejeitadas durante a revisão.| Perfil | Arquitetura | CPU/Instância | Sinalizadores de compilador | Finalidade |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabeleça valores mínimos sem intrínsecos vetoriais; usado para ajustar tabelas de custos alternativos. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | versão padrão | Valida o caminho AVX2; verifica se as acelerações do SIMD permanecem dentro da tolerância do gás neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versão padrão | Garante que o back-end do NEON permaneça determinístico e alinhado com as programações x86. |

- **Arnês de referência.** Todos os relatórios de calibração de gás DEVEM ser produzidos com:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar a fixação determinística.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` sempre que os custos do opcode da VM mudam.

- **Aleatoriedade corrigida.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de executar bancadas para que `iroha_test_samples::gen_account_in` mude para o caminho determinístico `KeyPair::from_seed`. O chicote imprime `IROHA_CONF_GAS_SEED_ACTIVE=…` uma vez; se a variável estiver faltando, a revisão DEVE falhar. Quaisquer novos utilitários de calibração devem continuar honrando esta variável de ambiente ao introduzir aleatoriedade auxiliar.

- **Captura de resultados.**
  - Carregar resumos de critérios (`target/criterion/**/raw.csv`) para cada perfil no artefato de lançamento.
  - Armazene métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) em `docs/source/confidential_assets_calibration.md` junto com o commit do git e a versão do compilador usada.
  - Manter as duas últimas linhas de base por perfil; exclua instantâneos mais antigos assim que o relatório mais recente for validado.

- **Tolerâncias de aceitação.**
  - Os deltas de gás entre `baseline-simd-neutral` e `baseline-avx2` DEVEM permanecer ≤ ±1,5%.
  - Os deltas de gás entre `baseline-simd-neutral` e `baseline-neon` DEVEM permanecer ≤ ±2,0%.
  - As propostas de calibração que excedam esses limites exigem ajustes no cronograma ou uma RFC explicando a discrepância e a mitigação.

- **Lista de verificação de revisão.** Os remetentes são responsáveis por:
  - Incluindo trechos `uname -a`, `/proc/cpuinfo` (modelo, revisão) e `rustc -Vv` no registro de calibração.
  - Verificando `IROHA_CONF_GAS_SEED` ecoado na saída da bancada (as bancadas imprimem o seed ativo).
  - Garantir a produção de espelhos de sinalizadores de recurso de marcapasso e verificador confidencial (`--features confidential,telemetry` ao executar bancadas com Telemetria).

## Configuração e operações
- `iroha_config` ganha seção `[confidential]`:
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
- A telemetria emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` e `confidential_policy_transitions_total`, nunca expondo dados de texto simples.
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Estratégia de teste
- Determinismo: o embaralhamento aleatório de transações dentro dos blocos produz raízes Merkle e conjuntos anuladores idênticos.
- Reorganização de resiliência: simule reorganizações multibloco com âncoras; os anuladores permanecem estáveis ​​e as âncoras obsoletas são rejeitadas.
- Invariantes de gás: verifique o uso de gás idêntico em nós com e sem aceleração SIMD.
- Teste de limite: provas de tamanho/tetos de gás, contagens máximas de entrada/saída, aplicação de tempo limite.
- Ciclo de vida: operações de governança para ativação/descontinuação de verificador e parâmetros, testes de rotação de gastos.
- Política FSM: transições permitidas/não permitidas, atrasos de transição pendentes e rejeição de mempool em torno das alturas efetivas.
- Emergências cadastrais: saque emergencial congela bens afetados em `withdraw_height` e rejeita comprovantes posteriormente.
- Capability gating: validadores com blocos de rejeição `conf_features` incompatíveis; observadores com `assume_valid=true` acompanham sem afetar o consenso.
- Equivalência de estado: nós validadores/completos/observadores produzem raízes de estado idênticas na cadeia canônica.
- Fuzzing negativo: provas malformadas, cargas superdimensionadas e colisões de nulificadores são rejeitadas deterministicamente.

## Trabalho Excelente
- Compare conjuntos de parâmetros do Halo2 (tamanho do circuito, estratégia de pesquisa) e registre os resultados no manual de calibração para que os padrões de gás/tempo limite possam ser atualizados junto com a próxima atualização do `confidential_assets_calibration.md`.
- Finalizar as políticas de divulgação do auditor e as APIs de visualização seletiva associadas, conectando o fluxo de trabalho aprovado ao Torii assim que o rascunho de governança for aprovado.
- Estender o esquema de criptografia de testemunha para cobrir saídas de vários destinatários e memorandos em lote, documentando o formato de envelope para implementadores de SDK.
- Encomendar uma revisão de segurança externa de circuitos, registros e procedimentos de rotação de parâmetros e arquivar as conclusões junto aos relatórios de auditoria interna.
- Especifique APIs de reconciliação de gastos do auditor e publique orientações de escopo de chave de visualização para que os fornecedores de carteiras possam implementar a mesma semântica de atestado.## Fases de implementação
1. **Fase M0 — Endurecimento Stop-Ship**
   - ✅ A derivação do nulificador agora segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com ordem de compromisso determinística aplicada nas atualizações do razão.
   - ✅ A execução impõe limites de tamanho de prova e cotas confidenciais por transação/por bloco, rejeitando transações acima do orçamento com erros determinísticos.
   - ✅ O handshake P2P anuncia `ConfidentialFeatureDigest` (resumo de backend + impressões digitais de registro) e falha nas incompatibilidades deterministicamente via `HandshakeConfidentialMismatch`.
   - ✅ Remova pânicos em caminhos de execução confidenciais e adicione controle de função para nós sem capacidade de correspondência.
   - ⚪ Aplicar orçamentos de tempo limite do verificador e reorganizar limites de profundidade para pontos de verificação de fronteira.
     - ✅ Verificação de timeout de orçamentos aplicados; as provas que excedem `verify_timeout_ms` agora falham deterministicamente.
     - ✅ Os pontos de verificação de fronteira agora respeitam `reorg_depth_bound`, removendo pontos de verificação mais antigos que a janela configurada enquanto mantém instantâneos determinísticos.
   - Introduzir `AssetConfidentialPolicy`, política FSM e portões de aplicação para instruções de cunhagem/transferência/revelação.
   - Confirme `conf_features` em cabeçalhos de bloco e recuse a participação do validador quando os resumos de registro/parâmetro divergem.
2. **Fase M1 — Registros e Parâmetros**
   - Registros terrestres `ZkVerifierEntry`, `PedersenParams` e `PoseidonParams` com operações de governança, ancoragem de gênese e gerenciamento de cache.
   - Conecte o syscall para exigir pesquisas de registro, IDs de programação de gás, hash de esquema e verificações de tamanho.
   - Envie formato de carga criptografada v1, vetores de derivação de chave de carteira e suporte CLI para gerenciamento de chaves confidenciais.
3. **Fase M2 — Gás e Desempenho**
   - Implementar cronograma determinístico de gás, contadores por bloco e chicotes de benchmark com telemetria (verificar latência, tamanhos de prova, rejeições de mempool).
   - Pontos de verificação Harden CommitmentTree, carregamento de LRU e índices anuladores para cargas de trabalho de vários ativos.
4. **Fase M3 – Rotação e Ferramentas de Carteira**
   - Habilitar aceitação de prova multiparâmetro e multiversão; suporte à ativação/descontinuação orientada por governança com runbooks de transição.
   - Forneça fluxos de migração SDK/CLI de carteira, fluxos de trabalho de verificação de auditores e ferramentas de reconciliação de gastos.
5. **Fase M4 — Auditoria e Operações**
   - Fornece fluxos de trabalho importantes para auditores, APIs de divulgação seletiva e runbooks operacionais.
   - Agende uma revisão externa de criptografia/segurança e publique as descobertas em `status.md`.

Cada fase atualiza os marcos do roteiro e os testes associados para manter garantias de execução determinísticas para a rede blockchain.

### SDK e cobertura de dispositivos (Fase M1)

A carga útil criptografada v1 agora é fornecida com acessórios canônicos para que cada SDK produza o
mesmos envelopes Norito e hashes de transação. Os artefatos de ouro vivem em
`fixtures/confidential/wallet_flows_v1.json` e são exercidos diretamente pelo
Suítes Rust e Swift (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`):

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```Cada fixture registra o identificador do caso, o hexadecimal da transação assinada e o esperado
haxixe. Quando o codificador Swift ainda não consegue produzir o caso - `zk-transfer-basic` é
ainda controlado pelo construtor `ZkTransfer` - o conjunto de testes emite `XCTSkip` para que o
O roteiro rastreia claramente quais fluxos ainda exigem vinculações. Atualizando o aparelho
arquivo sem alterar a versão do formato falhará em ambos os conjuntos, mantendo os SDKs
e implementação de referência Rust em lock-step.

#### Construtores Swift
`TxBuilder` expõe auxiliares assíncronos e baseados em retorno de chamada para cada
solicitação confidencial (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`).
Os construtores contam com as exportações `connect_norito_bridge`
(`crates/connect_norito_bridge/src/lib.rs:3337`,
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) então o gerado
as cargas correspondem aos codificadores do host Rust, byte por byte. Exemplo:

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "rose#wonderland",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

A blindagem/desblindagem segue o mesmo padrão (`submit(shield:)`,
`submit(unshield:)`), e os testes do acessório Swift executam novamente os construtores com
material de chave determinística para garantir que os hashes de transação gerados permaneçam
iguais aos armazenados em `wallet_flows_v1.json`.

#### Construtores JavaScript
O JavaScript SDK espelha os mesmos fluxos por meio dos auxiliares de transação exportados
de `javascript/iroha_js/src/transaction.js`. Construtores como
`buildRegisterZkAssetTransaction` e `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) normalizar a chave de verificação
identificadores e emitem cargas úteis Norito que o host Rust pode aceitar sem qualquer
adaptadores. Exemplo:

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "i105...",
    assetDefinitionId: "rose#wonderland",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

Os construtores Shield, Transfer e Unshield seguem o mesmo padrão, dando ao JS
chamadores a mesma ergonomia do Swift e Rust. Testes em
`javascript/iroha_js/test/transactionBuilder.test.js` cobre a normalização
lógica enquanto os fixtures acima mantêm os bytes de transação assinados consistentes.

### Telemetria e Monitoramento (Fase M2)

A Fase M2 agora exporta a integridade do CommitmentTree diretamente via Prometheus e Grafana:

- `iroha_confidential_tree_commitments`, `iroha_confidential_tree_depth`, `iroha_confidential_root_history_entries` e `iroha_confidential_frontier_checkpoints` expõem a fronteira Merkle ativa por ativo enquanto `iroha_confidential_root_evictions_total` / `iroha_confidential_frontier_evictions_total` contam os cortes LRU aplicados por `zk.root_history_cap` e a janela de profundidade do ponto de verificação.
- `iroha_confidential_frontier_last_checkpoint_height` e `iroha_confidential_frontier_last_checkpoint_commitments` publicam a contagem de altura + comprometimento do ponto de verificação de fronteira mais recente para que exercícios de reorganização e reversões possam provar que os pontos de verificação avançam e retêm o volume de carga útil esperado.
- A placa Grafana (`dashboards/grafana/confidential_assets.json`) inclui uma série de profundidade, painéis de taxa de despejo e os widgets de cache do verificador existentes para que os operadores possam provar que a profundidade do CommitmentTree nunca entra em colapso, mesmo quando os pontos de verificação mudam.
- O alerta `ConfidentialTreeDepthZero` (em `dashboards/alerts/confidential_assets_rules.yml`) dispara assim que os compromissos são observados, mas a profundidade relatada permanece em zero por cinco minutos.

Você pode verificar as métricas localmente antes de conectar Grafana:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Combine isso com `rg 'iroha_confidential_tree_depth'` no mesmo rascunho para confirmar que a profundidade aumenta com novos compromissos, enquanto os contadores de despejo só aumentam quando o histórico limita as entradas. Esses valores devem estar alinhados com a exportação do painel Grafana que você anexa aos pacotes de evidências de governança.

#### Telemetria e alertas de programação de gásA Fase M2 também encadeia os multiplicadores de gás configuráveis no pipeline de telemetria para que os operadores possam provar que cada validador compartilha os mesmos custos de verificação antes de aprovar uma liberação:

- `iroha_confidential_gas_base_verify` espelha `confidential.gas.proof_base` (padrão `250_000`).
- `iroha_confidential_gas_per_public_input`, `iroha_confidential_gas_per_proof_byte`, `iroha_confidential_gas_per_nullifier` e `iroha_confidential_gas_per_commitment` espelham seus respectivos botões em `ConfidentialConfig`. Os valores são atualizados na inicialização e sempre que a configuração é recarregada a quente; `irohad` (`crates/irohad/src/main.rs:1591,1642`) envia o planejamento ativo por meio de `Telemetry::set_confidential_gas_schedule`.

Raspe os medidores junto com as métricas do CommitmentTree para confirmar se os botões são idênticos entre os pares:

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

Painel Grafana `confidential_assets.json` agora inclui um painel “Gas Schedule” que renderiza os cinco medidores e destaca a divergência. Regras de alerta na capa `dashboards/alerts/confidential_assets_rules.yml`:
- `ConfidentialGasMismatch`: verifica o máximo/mínimo de cada multiplicador em todos os alvos de raspagem e páginas quando algum diverge por mais de 3 minutos, solicitando que os operadores alinhem `confidential.gas` por meio de recarregamento a quente ou reimplantação.
- `ConfidentialGasTelemetryMissing`: avisa quando Prometheus não consegue raspar nenhum dos cinco multiplicadores por 5 minutos, indicando um alvo de raspagem ausente ou telemetria desabilitada.

Mantenha o seguinte PromQL à mão para investigações de plantão:

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

O desvio deve permanecer zero fora das implementações de configuração controladas. Ao alterar a tabela de gases, capture os arranhões antes/depois, anexe-os à solicitação de alteração e atualize `docs/source/confidential_assets_calibration.md` com os novos multiplicadores para que os revisores de governança possam vincular as evidências de telemetria ao relatório de calibração.