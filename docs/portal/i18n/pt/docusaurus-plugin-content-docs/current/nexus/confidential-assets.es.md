---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ativos confidenciais e transferências ZK
descrição: Plano de Fase C para circulação cega, registros e controles de operador.
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseno de ativos confidenciais e transferências ZK

## Motivação
- Entregar fluxos de ativos blindados opt-in para que os domínios preservem a privacidade transacional sem alterar a circulação transparente.
- Mantém a execução determinista em hardware heterogêneo de validadores e conserva Norito/Kotodama ABI v1.
- Provar controles de ciclo de vida de auditores e operadores (ativação, rotação, revogação) para circuitos e parâmetros criptográficos.

## Modelo de Amenazas
- Los validadores são honestos-mas-curiosos: ejecutan consenso fielmente mas tentando inspecionar razão/estado.
- Observadores de dados vermelhos de bloqueio e transações fofocadas; não se assumam canais de fofoca privados.
- Fuera de alcance: análise de tráfego off-ledger, adversários cuanticos (seguidos no roadmap PQ), ataques de disponibilidade do ledger.

## Resumo de design
- Os ativos podem declarar uma *reserva protegida* além dos saldos transparentes existentes; a circulação cegada é representada por meio de compromissos criptográficos.
- As notas encapsuladas `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independente da ordem das notas.
  - Carga útil criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- As transações transportando cargas úteis `ConfidentialTransfer` codificadas com Norito que contêm:
  - Entradas públicas: âncora Merkle, anuladores, novos compromissos, ID de ativo, versão de circuito.
  - Payloads criptografados para destinatários e auditores opcionais.
  - Prueba conhecimento zero que atesta conservação de valor, propriedade e autorização.
- Verificação de chaves e conjuntos de parâmetros são controlados por meio de registros on-ledger com janelas de ativação; os nodos rechazan validam provas que referenciam entradas desconocidas ou revogadas.
- Os cabeçalhos de acordo comprometem o resumo ativo de capacidades confidenciais para que os blocos sejam aceitos apenas quando o estado de registros e os parâmetros coincidam.
- A construção de provas usa uma pilha Halo2 (Plonkish) sem configuração confiável; Groth16 e outras variantes SNARK são consideradas intencionalmente não suportadas na v1.

### Fixtures deterministas

As sobres de memorandos confidenciais agora incluem um dispositivo canônico em `fixtures/confidential/encrypted_payload_v1.json`. O conjunto de dados captura uma versão positiva, mas mostra negativas malformadas para que os SDKs possam afirmar paridade de análise. Os testes do modelo de dados em Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e o conjunto de Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) carregam o dispositivo diretamente, garantindo que a codificação Norito, as superfícies de erro e a cobertura de regressão permaneçam alinhadas enquanto o codec evolui.

Os SDKs do Swift agora podem emitir instruções escudo sem cola JSON bespoke: construye un
`ShieldRequest` com comprometimento de 32 bytes, carga útil criptografada e metadados de débito,
luego chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para firmar e enviar
transação sobre `/v2/pipeline/transactions`. El helper valida longitudes de compromisso,
elhebra `ConfidentialEncryptedPayload` no codificador Norito, e reflita o layout `zk::Shield`
descrito abaixo para que as carteiras sejam sincronizadas com Rust.## Compromissos de consenso e controle de capacidade
- Os cabeçalhos do bloco expostos `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o resumo participa do hash de consenso e deve igualar a vista local do registro para aceitar o bloco.
- Governança pode preparar atualizações programando `next_conf_features` com um `activation_height` futuro; até essa altura, os produtores de blocos devem seguir emitindo o resumo anterior.
- Os nodos validadores DEBEN operam com `confidential.enabled = true` e `assume_valid = false`. As verificações de início rechazan se unem ao conjunto de validadores se qualquer um falhar ou se o `conf_features` divergir localmente.
- Os metadados do handshake P2P agora incluem `{ enabled, assume_valid, conf_features }`. Os pares que anunciam recursos não suportados são rechaçados com `HandshakeConfidentialMismatch` e nunca entram em rotação de consenso.
- Os resultados do handshake entre validadores, observadores e pares são capturados na matriz de handshake inferior [Negociação de capacidade do nó] (#node-capability-negotiation). As falhas de aperto de mão expõem `HandshakeConfidentialMismatch` e mantêm-se no par fora da rotação de consenso até que seu resumo coincida.
- Observadores no validadores podem fijar `assume_valid = true`; aplicam-se deltas confidenciais a ciegas, mas não influenciam a segurança do consenso.

## Política de ativos
- Cada definição de ativo levará um `AssetConfidentialPolicy` fixado pelo criador ou via governança:
  - `TransparentOnly`: modo por defeito; apenas se permitem instruções transparentes (`MintAsset`, `TransferAsset`, etc.) e as operações blindadas são rechaçadas.
  - `ShieldedOnly`: todas as emissões e transferências devem usar instruções confidenciais; `RevealConfidential` é proibido que os saldos nunca sejam divulgados publicamente.
  - `Convertible`: os suportes podem mover valor entre representações transparentes e blindadas usando as instruções da rampa de entrada/desligamento de baixo.
- Las políticas seguem um FSM restrito para evitar fundos varados:
  - `TransparentOnly -> Convertible` (habilitação imediata da piscina blindada).
  - `TransparentOnly -> ShieldedOnly` (requer transição pendente e janela de conversão).
  - `Convertible -> ShieldedOnly` (demora mínima obrigatória).
  - `ShieldedOnly -> Convertible` (requer plano de migração para que as notas cegadas sigam quando estão gastáveis).
  - `ShieldedOnly -> TransparentOnly` não permite que o pool protegido seja vazio ou a governança codifique uma migração que des-cega notas pendentes.
- Instruções de governança fijan `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via ISI `ScheduleConfidentialPolicyTransition` e pode abortar mudanças programadas com `CancelConfidentialPolicyTransition`. A validação do mempool garante que nenhuma transação cruze a altura de transição e a inclusão caia na forma determinista se uma verificação de mudança política for meio de bloqueio.
- As transições pendentes são aplicadas automaticamente quando um novo bloqueio é aberto: uma vez que a altura entra na janela de conversão (para atualizações `ShieldedOnly`) ou alcanza `effective_height`, o tempo de execução atualiza `AssetConfidentialPolicy`, atualiza os metadados `zk.policy` e limpa a entrada pendente. Se o fornecimento for transparente quando uma transição `ShieldedOnly` estiver madura, o tempo de execução aborta a mudança e registra uma advertência, deixando o modo anterior intacto.
- Os botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` fornecem aviso mínimo e períodos de graça para permitir conversões de carteiras ao redor da mudança.
- `pending_transition.transition_id` também funciona como alça de auditório; governança deve citá-lo para encerrar ou cancelar transições para que os operadores correlacionem relatórios de rampa de ativação/desativação.
- `policy_transition_window_blocks` padrão é 720 (aprox 12 horas com tempo de bloqueio de 60 s). Os nodos limitam as solicitações de governança que pretendem avisos mas curtos.
- Genesis manifesta e flujos CLI expõe políticas atuais e pendentes. A lógica de admissão é a política de tempo de execução para confirmar que cada instrução confidencial foi autorizada.
- Checklist de migração - veja "Sequenciamento de migração" abaixo para o plano de atualização por etapas que seguem o Milestone M0.

#### Monitoramento de transições via Torii

Carteiras e auditores consultam `GET /v2/confidential/assets/{definition_id}/transitions` para inspecionar o `AssetConfidentialPolicy` ativo. O payload JSON sempre inclui o id de ativo canônico, a altura final do bloco observado, o `current_mode` da política, o modo efetivo nessa altura (as janelas de conversão reportam temporalmente `Convertible`), e os identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Quando há uma transição pendente, a resposta também inclui:- `transition_id` - identificador de auditórios desenvolvido por `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e o `window_open_height` derivado (o bloco onde as carteiras devem começar a conversão para cut-overs ShieldedOnly).

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

Uma resposta `404` indica que não existe uma definição de ativo coincidente. Quando não há transição programada no campo `pending_transition` é `null`.

### Máquina de estados de política

| Modo atual | Modo seguinte | Pré-requisitos | Manejo de altura efetivamente | Notas |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | A governança ativou entradas de registro de selecionados/parâmetros. Enviar `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | A transição é executada exatamente em `effective_height`; a piscina blindada fica disponível imediatamente.        | Ruta por defeito para habilitar a confidencialidade enquanto se mantêm fluxos transparentes. |
| Somente Transparente | Somente blindado | Igual que chega, mas `policy_transition_window_blocks >= 1`.                                                       | O tempo de execução entra automaticamente em `Convertible` em `effective_height - policy_transition_window_blocks`; mudança para `ShieldedOnly` e `effective_height`. | Provee ventana de conversão determinista antes de deshabilitar instruções transparentes. |
| Conversível | Somente blindado | Transição programada com `effective_height >= current_height + policy_transition_delay_blocks`. Governança DEBE certificada (`transparent_supply == 0`) via metadados de auditoria; o tempo de execução é aplicado na transição. | Semântica de janela idêntica à anterior. Se o fornecimento transparente for zero em `effective_height`, a transição será interrompida com `PolicyTransitionPrerequisiteFailed`. | Bloqueie o ativo em circulação de forma totalmente confidencial.                                |
| Somente blindado | Conversível | Transição programada; sin retiro de emergência ativo (`withdraw_height` não definido).                            | O estado muda em `effective_height`; as rampas reveladas se reabrem enquanto as notas ficam cegadas e depois são válidas. | Usado para janelas de manutenção ou revisões de auditores.                            |
| Somente blindado | Somente Transparente | A governança deve testar `shielded_supply == 0` ou preparar um plano `EmergencyUnshield` firmado (firmas de auditor requeridas). | O tempo de execução abre uma janela `Convertible` antes de `effective_height`; nesta altura, as instruções confidenciais falham duramente e o ativo volta para o modo somente transparente. | Saída de último recurso. A transição será cancelada automaticamente se ocorrer qualquer gasto de nota confidencial durante a janela. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` limpa a mudança pendente.                                                    | `pending_transition` é eliminado imediatamente.                                                                     | Mantenha o status quo; mostrado por completo.                                          |

As transições não arribas são rechaçadas durante a submissão da governança. O tempo de execução verifica os pré-requisitos antes de aplicar uma transição programada; se cair, desvie o ativo para o modo anterior e emita `PolicyTransitionPrerequisiteFailed` via telemetria e eventos de bloqueio.

### Sequência de migração1. **Preparar registros:** ativar todas as entradas selecionadas e parâmetros referenciados pelo objetivo político. Os nós anunciam o `conf_features`, fazendo com que os pares verifiquem a coerência.
2. **Programe a transição:** envie `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeita `policy_transition_delay_blocks`. Ao mover-se para `ShieldedOnly`, especifique uma janela de conversão (`window >= policy_transition_window_blocks`).
3. **Publicar guia para operadores:** registrar o `transition_id` deve ser publicado e circular um runbook de on/off-ramp. Carteiras e auditores se inscrevem em `/v2/confidential/assets/{id}/transitions` para saber a altura de abertura de janela.
4. **Aplicar janela:** quando abre a janela, o tempo de execução muda a política para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, e empieza para rechazar solicitações de governança em conflito.
5. **Finalizar ou abortar:** em `effective_height`, o tempo de execução verifica os pré-requisitos de transição (fornecimento transparente zero, sem emergências, etc.). Se passar, mude a política para o modo solicitado; se falhar, emite `PolicyTransitionPrerequisiteFailed`, limpe a transição pendente e deixe a política sem mudanças.
6. **Atualizações de esquema:** após uma transição exitosa, a governança depende da versão do esquema do ativo (por exemplo, `asset_definition.v2`) e a CLI de ferramentas requer `confidential_policy` para serializar manifestos. Os documentos de atualização do genesis instruem os operadores a adicionar configurações de política e impressões digitais do registro antes de reiniciar os validadores.

Novas redes que iniciam com confidencialidade habilitada codificam a política desejada diretamente na gênese. Aun asi siga a lista de verificação anterior para alterar os modos pós-lançamento para que as janelas de conversão sigam se deterministas e as carteiras tenham tempo de ajuste.

### Versão e ativação do manifesto Norito

- Genesis manifests DEBEN inclui um `SetParameter` para a chave personalizada `confidential_registry_root`. A carga útil é Norito JSON igual a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omita o campo (`null`) quando não houver entradas ativadas, ou prove uma string hexadecimal de 32 bytes (`0x...`) igual ao hash produzido por `compute_vk_set_hash` sobre eles instruções de verificação enviadas no manifesto. Os nós não serão iniciados se o parâmetro estiver faltando ou se o hash for diferente das escrituras de registro codificadas.
- El on-wire `ConfidentialFeatureDigest::conf_rules_version` incorpora a versão do layout do manifesto. Para redes v1 DEBE permanecer `Some(1)` e é igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quando o conjunto de regras evolui, sube a constante, regenera manifestos e despliega binários em lock-step; As versões mezclar fazem com que os validadores recuperem blocos com `ConfidentialFeatureDigestMismatch`.
- Manifestos de ativação DEBERIAN agrupando atualizações de registro, mudanças de ciclo de vida de parâmetros e transições políticas para que o resumo se mantenha consistente:
  1. Aplique as alterações de registro planejadas (`Publish*`, `Set*Lifecycle`) em uma vista offline do estado e calcule o resumo pós-ativação com `compute_confidential_feature_digest`.
  2. Emita `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando o hash calculado para que pares atrasados ​​possam recuperar o resumo correto mesmo que você tenha perdido instruções intermediárias de registro.
  3. Anexe as instruções `ScheduleConfidentialPolicyTransition`. Cada instrução deve citar o `transition_id` emitido por governança; manifesta que você omitirá ser rechazado pelo tempo de execução.
  4. Persista os bytes do manifesto, uma impressão digital SHA-256 e o ​​resumo usado no plano de ativação. Os operadores verificam os três artefatos antes de votar o manifesto para evitar partições.
- Quando as implementações exigirem um corte diferido, registre a altura desejada em um parâmetro personalizado acompanhante (por exemplo, `custom.confidential_upgrade_activation_height`). Isto foi dado aos auditores uma tentativa codificada em Norito de que os validadores respeitassem a janela de aviso antes de a mudança de resumo entrar em vigor.## Ciclo de vida de verificadores e parâmetros
### Registro ZK
- O livro-razão `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` onde `proving_system` atualmente está localizado em `Halo2`.
- Pares `(circuit_id, version)` são globalmente únicos; o registro mantém um índice secundário para consultas de metadados de circuito. As intenções de registrar uma par duplicada são rechazanadas durante a admissão.
- `circuit_id` deve ser no vacio e `public_inputs_schema_hash` deve ser fornecido (normalmente um hash Blake2b-32 da codificação canônica de entradas públicas do verificador). Admissão rechaza registros que omitem esses campos.
- As instruções de governança incluem:
  - `PUBLISH` para adicionar uma entrada `Proposed` apenas com metadados.
  - `ACTIVATE { vk_id, activation_height }` para programar ativação em um limite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar a altura final onde as provas podem referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergência; ativos afetados congelados gastos confidenciais após retirada de altura até que novas entradas sejam ativas.
- Genesis manifests emite automaticamente um parâmetro personalizado `confidential_registry_root` cujo `vk_set_hash` coincide com entradas ativas; a validação cruza é um resumo do estado local do registro antes que um nó possa se unir a um consenso.
- Registrador ou atualização de um selecionador requerido `gas_schedule_id`; a verificação exige que a entrada de registro seja `Active`, presente no índice `(circuit_id, version)`, e que as provas Halo2 provem um `OpenVerifyEnvelope` cujo `circuit_id`, `vk_hash`, e `public_inputs_schema_hash` coincidente com o registro do registro.

### Provando Chaves
- As chaves de prova são mantidas off-ledger, mas são referenciadas por identificadores direcionados por conteúdo (`pk_cid`, `pk_hash`, `pk_len`) publicados junto com os metadados do verificador.
- Os SDKs da carteira obtêm dados de PK, hashes verificados e cache localmente.

### Parâmetros Pedersen e Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) refletem controles de ciclo de vida do selecionador, cada um com `params_id`, hashes de geradores/constantes, ativação, depreciação e alturas de retiro.
- Commitments e hashes separados de domínios por `params_id` para que a rotação de parâmetros nunca reutilize patronos de bits de conjuntos obsoletos; o ID é incorporado em compromissos de notas e tags de domínio de nulificador.
- Los circuitos suportam seleção multiparâmetro no tempo de verificação; conjuntos de parâmetros obsoletos após serem gastáveis ​​até seu `deprecation_height`, e conjuntos retirados serão rechazados exatamente em seu `withdraw_height`.

## Ordenar deterministas e anuladores
- Cada ativo mantém um `CommitmentTree` com `next_leaf_index`; os blocos agregam compromissos em ordem determinista: iterar transações em ordem de bloco; dentro de cada transação iterar produz blindados por `output_idx` serializado ascendente.
- `note_position` é derivado de compensações da árvore, mas **não** faz parte do nulificador; só alimenta rotas de membresia dentro da testemunha de teste.
- A estabilidade do nulificador abaixo das reorganizações está garantida pelo diseno PRF; a entrada PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e as âncoras referenciadas raízes Merkle históricos limitados por `max_anchor_age_blocks`.## Fluxo de razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Requer política de ativo `Convertible` ou `ShieldedOnly`; admissão verifica autoridade de ativo, obtiene `params_id` real, muestrea `rho`, emite compromisso, atualiza arbol Merkle.
   - Emite `ConfidentialEvent::Shielded` com o novo compromisso, raiz delta de Merkle e hash de chamada de transação para trilhas de auditoria.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - Syscall da VM verifica a prova usando a entrada do registro; el host asegura nullifiers no usados, compromissos anexados de forma determinista, âncora recente.
   - O razão registra entradas de `NullifierSet`, armazena cargas úteis criptografadas para destinatários/auditores e emite `ConfidentialEvent::Transferred` resumindo nulificadores, saídas ordenadas, hash de prova e raízes Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - Disponível apenas para ativos `Convertible`; a prova válida é que o valor da nota é igual ao valor revelado, o razão considera saldo transparente e a nota fica cega, marcando o nulo como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o monte público, anuladores consumidos, identificadores de prova e hash de chamada de transação.

## Adicionados ao modelo de dados
- `ConfidentialConfig` (nova seção de configuração) com bandeira de habilitação, `assume_valid`, botões de gás/limites, janela de âncora, backend de verificação.
- `ConfidentialNote`, `ConfidentialTransfer`, e `ConfidentialMint` esquemas Norito com byte de versão explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envolve bytes de memorando AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, com padrão `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para o layout XChaCha20-Poly1305.
- Vetores canônicos de derivação de chave existentes em `docs/source/confidential_key_vectors.json`; tanto o CLI quanto o endpoint Torii retornam contra esses fixtures.
- `asset::AssetDefinition` acrescenta `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste a ligação `(backend, name, commitment)` para verificadores de transferência/desproteção; a ejecução rechaza provas cuja chave de verificação referenciada ou inline não coincide com o compromisso registrado.
- `CommitmentTree` (por ativos com pontos de verificação de fronteira), `NullifierSet` com a chave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armazenados no estado mundial.
- Mempool mantém estruturas transitórias `NullifierIndex` e `AnchorIndex` para detecção de temperatura duplicada e verificação de idade de âncora.
- Atualizações do esquema Norito incluem ordenação canônica para entradas públicas; testes de ida e volta para garantir o determinismo de codificação.
- Roundtrips de carga útil criptografada quedan fijados via testes unitários (`crates/iroha_data_model/src/confidential.rs`). Vetores de carteira de acompanhamento adicionais de transcrições AEAD canônicas para auditores. `norito.md` documenta o cabeçalho on-wire para o envelope.

## Integração com IVM e syscall
- Introduza o syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, e o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - O syscall carrega metadados do verificador do registro, aplica limites de tempo/tempo, cobra gas determinista, e só aplica o delta se a prova tiver sucesso.
- El host expõe um traço de leitura individual `ConfidentialLedger` para recuperar instantâneos de raiz Merkle e estado de nulificador; a biblioteca Kotodama fornece ajudantes de montagem de testemunha e validação de esquema.
- Os documentos do ponteiro-ABI são atualizados para esclarecer o layout do buffer de prova e manipuladores de registro.

## Negociação de capacidades de nodo
- O aperto de mão anuncia `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. A participação de validadores requer `confidential.enabled=true`, `assume_valid=false`, identificadores de backend de selecionados idênticos e resumos coincidentes; discrepâncias afetam o aperto de mão com `HandshakeConfidentialMismatch`.
- A configuração suporta `assume_valid` apenas para nós observadores: quando estiver desativado, encontre instruções confidenciais, produza `UnsupportedInstruction` determinista sin panic; Quando isso estiver habilitado, os observadores aplicarão os deltas declarados sem verificar as provas.
- Mempool rechaza transações confidenciais se a capacidade local estiver desativada. Os filtros de fofoca evitam enviar transações cegas para pares sem capacidades coincidentes enquanto reenviam a ciegas IDs de selecionados desconhecidos dentro de limites de tamanho.

### Matriz de aperto de mão| Anúncio remoto | Resultado para nós validadores | Notas para operadores |
|------------------|------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend coincide, resumo coincide | Aceito | O par chegou ao estado `Ready` e participou da proposta, voto e divulgação do RBC. Não é necessário manual de ação. |
| `enabled=true`, `assume_valid=false`, backend coincide, digest obsoleto ou faltante | Rechazado (`HandshakeConfidentialMismatch`) | O controle remoto deve aplicar ativações pendentes de registro/parâmetros ou esperar o `activation_height` programado. Até corrigir, o nodo continua visível, mas nunca entra em rotação de consenso. |
| `enabled=true`, `assume_valid=true` | Rechazado (`HandshakeConfidentialMismatch`) | Os validadores exigem verificação de provas; configure o controle remoto como observer com entrada solo Torii ou altere `assume_valid=false` após ativar a verificação completa. |
| `enabled=false`, campos omitidos (build desactualizado), o backend de selecionador distinto | Rechazado (`HandshakeConfidentialMismatch`) | Pares desactualizados ou parcialmente actualizados não podem unir-se à rede de consenso. Atualizamos a versão atual e garantimos que o backend da tupla + resumo coincida antes de reconectar. |

Observadores que intencionalmente omitem a verificação de provas não devem abrir conexões de consenso contra validadores com portas de capacidade. Você pode inserir blocos via Torii ou APIs de arquivo, mas a rede de consenso rechaza até que sejam anunciadas capacidades coincidentes.

### Política de poda de revelações e retenção de anuladores

Os livros-razões confidenciais devem reter histórico suficiente para testar o frescor das notas e reproduzir auditorias impulsionadas pela governança. A política por defeito, aplicada por `ConfidentialLedger`, é:

- **Retenção de nulificadores:** manter nulificadores gastos por um *mínimo* de `730` dias (24 meses) após a altura do gasto, ou a janela reguladora obrigatória se for maior. Os operadores podem estender a janela via `confidential.retention.nullifier_days`. Anuladores mais recentes que a janela DEBEN segue consultáveis ​​via Torii para que os auditores prubem ausencia de gasto duplo.
- **Pruning de revela:** as revelações transparentes (`RevealConfidential`) podem os compromissos associados imediatamente depois que o bloco finaliza, mas o nulificador consumido segue sujeito à regra de retenção anterior. Eventos relacionados com revelação (`ConfidentialEvent::Unshielded`) registram o monte público, destinatário e hash de prova para que reconstruir revelações históricas não exijam o texto cifrado podado.
- **Pontos de controle de fronteira:** as fronteiras de compromisso mantêm pontos de controle em movimento que cobrem o prefeito de `max_anchor_age_blocks` e a janela de retenção. Os nodos compactos checkpoints mas antigos só depois de todos os anuladores dentro do intervalo expirarem.
- **Remediacion por digest stale:** se `HandshakeConfidentialMismatch` for levantado por desvio de digest, os operadores devem (1) verificar se as janelas de retenção de nullifiers coincidem no cluster, (2) executar `iroha_cli app confidential verify-ledger` para regenerar o digest contra o conjunto de nullifiers retidos, e (3) reimplementar o manifesto atualizado. Qualquer nullifier podado prematuramente deve ser restaurado de um armazenamento frio antes de reingressar na rede.

A Documenta substitui localidades no runbook de operações; As políticas de governança que ampliam a janela de retenção devem atualizar a configuração do nó e os planos de armazenamento do arquivo em sincronia.

### Fluxo de despejo e recuperação

1. Durante o dial, `IrohaNetwork` compara as capacidades anunciadas. Qualquer incompatibilidade levanta `HandshakeConfidentialMismatch`; a conexão é fechada e o par permanece na cola da descoberta sem ser promovido a `Ready`.
2. A falha é exposta através do log do serviço de rede (incluye digest remoto e backend), e Sumeragi nunca programa al peer para propor o voto.
3. Os operadores corrigem os registros selecionados e os conjuntos de parâmetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou programam `next_conf_features` com um `activation_height` acordado. Uma vez que o resumo coincida, o handshake seguinte será encerrado automaticamente.
4. Se um par obsoleto registrar um bloco (por exemplo, via repetição de arquivo), os validadores rechazanão a forma determinista com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, mantendo o estado do razão consistente na rede.

### Fluxo de handshake seguro ante replay1. Cada intenção saliente designa material de chave Noise/X25519 novo. O payload de handshake que é firmado (`handshake_signature_payload`) concatena as chaves públicas efímeras locais e remotas, a direção de soquete anunciada codificada em Norito e, quando é compilada com `handshake_chain_id`, o identificador de cadeia. A mensagem é criptografada com AEAD antes de sair do nó.
2. O receptor recomputa a carga útil com a ordem de chaves peer/local invertida e verifica a firma Ed25519 incorporada em `HandshakeHelloV1`. Devido a ambas as chaves efímeras e a direção anunciada para parte do domínio da firma, reproduzir uma mensagem capturada contra outro par ou recuperar uma conexão obsoleta falha na verificação do formulário determinista.
3. Sinalizadores de capacidade confidencial e o `ConfidentialFeatureDigest` viaja dentro de `HandshakeConfidentialMeta`. O receptor compara a tupla `{ enabled, assume_valid, verifier_backend, digest }` com a `ConfidentialHandshakeCaps` local; qualquer incompatibilidade venda temprano com `HandshakeConfidentialMismatch` antes da transição de transporte para `Ready`.
4. Os operadores DEBEN recomputarão o resumo (via `compute_confidential_feature_digest`) e reiniciarão os nós com os registros/políticas atualizados antes de reconectar. Peers que anunciam digests antigos seguem falhando no handshake, evitando que estado obsoleto reingresse ao conjunto de validadores.
5. Saídas e falhas no handshake atualizam os contadores padrão `iroha_p2p::peer` (`handshake_failure_count`, ajudantes de taxonomia de erros) e emitem logs estruturados com o peer ID remoto e a impressão digital do digest. Monitore esses indicadores para detectar replays ou configurações incorretas durante a implementação.

## Gerenciamento de chaves e cargas úteis
- Jerarquia de derivação por conta:
  - `sk_spend` -> `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- Payloads de notas criptografadas usando AEAD com chaves compartilhadas derivadas de ECDH; você pode adicionar as chaves de auditoria opcionais às saídas de acordo com a política do ativo.
- Adicionados ao CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ferramentas de auditor para descifrar memorandos, e o auxiliar `iroha app zk envelope` para produzir/inspecionar envelopes Norito offline. Torii expõe o mesmo fluxo de derivação via `POST /v2/confidential/derive-keyset`, retornando formas hex e base64 para que carteiras obtenham jerarquias de chaves programaticamente.

## Gás, limites e controles DoS
- Cronograma de gás determinista:
  - Halo2 (Plonkish): gás base `250_000` + gás `2_000` por entrada pública.
  - `5` gás por byte de prova, mas cargas por anulador (`300`) e por compromisso (`500`).
  - Os operadores podem escrever estas constantes através da configuração do nó (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); as mudanças se propagam no início ou quando a capacidade de configuração hot-reload e se aplica de forma determinista no cluster.
- Limites duros (padrões configuráveis):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Provas que excedem `verify_timeout_ms` abortam a instrução de forma determinista (cédulas de governança emitem `proof verification exceeded timeout`, `VerifyProof` retorna erro).
- Cuotas adicionais para garantir a vivacidade: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, e `max_public_inputs` acotan block builders; `reorg_depth_bound` (>= `max_anchor_age_blocks`) governa a retenção de pontos de controle de fronteira.
- O tempo de execução de execução agora rechaza transações que excedem esses limites por transação ou por bloco, emitindo erros `InvalidParameter` deterministas e deixando o estado do razão sem mudanças.
- Mempool pré-filtre transações confidenciais por `vk_id`, comprimento de prova e idade de âncora antes de invocar o verificador para manter atualizado o uso de recursos.
- A verificação se detene de forma determinista em timeout ou violação de limites; as transações falharão com erros explícitos. Backends SIMD são opcionais, mas não alteram a contabilidade de gás.

### Linhas de base de calibração e portas de aceitação
- **Plataformas de referência.** As corridas de calibração DEBEN cobrem os três perfis de hardware abaixo. Corridas que não capturam todos os perfis se rechazan durante a revisão.| Perfil | Arquitetura | CPU / Instância | Bandeiras de compilador | Proposta |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabilidade de valores piso sem intrínsecos vetoriais; use-o para ajustar tabelas de custo alternativas. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | lançamento por defeito | Valida o caminho AVX2; verifique se os aceleradores SIMD são mantidos dentro da tolerância do gás neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | lançamento por defeito | Certifique-se de que o backend NEON permaneça determinista e alinhado com as programações x86. |

- **Arnês de referência.** Todos os relatórios de calibração de gás DEBEN produzem com:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar o dispositivo determinista.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` quando há custos variáveis ​​de opcode da VM.

- **Aleatoriedade fija.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de correr bancos para que `iroha_test_samples::gen_account_in` mude para a rota determinista `KeyPair::from_seed`. El chicote imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; se faltar a variável, a revisão DEBE falhará. Qualquer novo uso de calibração deve ser seguido respeitando este ambiente como introdução aleatória auxiliar.

- **Captura de resultados.**
  - Subir resmenes de Criterion (`target/criterion/**/raw.csv`) para cada perfil no artefato de lançamento.
  - Guardar métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Ledger de calibração de gás confidencial](./confidential-gas-calibration) junto com o commit do git e a versão do compilador usado.
  - Manter as últimas linhas de base por perfil; excluir instantâneos mais antigos uma vez validado o relatório mais novo.

- **Tolerâncias de aceitação.**
  - Deltas de gás entre `baseline-simd-neutral` e `baseline-avx2` DEBEN permanecem <= +/-1,5%.
  - Deltas de gás entre `baseline-simd-neutral` e `baseline-neon` DEBEN permanecem <= +/-2,0%.
  - Propostas de calibração que excedem esses limites exigem ajustes de cronograma ou uma RFC que explique a discrepância e sua mitigação.

- **Checklist de revisão.** Os remetentes são responsáveis por:
  - Incluir `uname -a`, extratos de `/proc/cpuinfo` (modelo, escalonamento), e `rustc -Vv` no registro de calibração.
  - Verifique se `IROHA_CONF_GAS_SEED` está visível na saída da bancada (as bancadas imprimem a semente ativa).
  - Certifique-se de que os sinalizadores de recurso do marca-passo e do selecionador confidencial, especialmente de produção (`--features confidential,telemetry`, ao correr bancos com telemetria).

## Configurações e operações
- `iroha_config` adiciona a seção `[confidential]`:
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
- Telemetria emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total`, nunca exponiendo dados em claro.
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## Estratégia de teste
- Determinismo: o aleatório aleatório de transações dentro de blocos produz raízes Merkle e conjuntos de anuladores idênticos.
- Resiliencia a reorg: reorganizações simuladas multibloco com âncoras; anuladores se mantêm estáveis ​​e âncoras obsoletas são rechazan.
- Invariantes de gás: verifique o uso de gás idêntico entre nós com e sem aceleração SIMD.
- Testes de borda: provas em tecnologia de tamano/gás, contagens máximas de entrada/saída, aplicação de tempo limite.
- Ciclo de vida: operações de governança para ativação/descontinuação de selecionados e parâmetros, testes de gasto após rotação.
- Política FSM: transições permitidas/não permitidas, atrasos de transição pendentes e rechazo de mempool alrededor de alturas efetivas.
- Emergências de registro: retirada de emergência de congelamento de ativos afetados em `withdraw_height` e rechaza as provas após.
- Gating de capacidade: validadores com blocos rechazan incompatíveis `conf_features`; observadores con `assume_valid=true` avançam sem afetar consenso.
- Equivalência de estado: nós validadores/completos/observadores produzem raízes de estado idênticas na cadeia canônica.
- Fuzzing negativo: provas malformadas, payloads sobredimensionados e colisões de nulificador são rechaçadas deterministicamente.## Migração
- Rollout com sinalizador de recurso: até que a Fase C3 seja concluída, `enabled` padrão a `false`; os nodos declaram capacidades antes de se unirem ao conjunto validador.
- Activos transparentes não afectados; as instruções confidenciais exigem entradas de registro e negociação de capacidades.
- Nodos elaborados sem suporte confidencial rechazan blocos relevantes de forma determinista; não pode se unir ao conjunto validador, mas pode operar como observadores com `assume_valid=true`.
- Os manifestos Genesis incluem entradas iniciais de registro, conjuntos de parâmetros, políticas confidenciais para ativos e chaves de auditoria opcionais.
- Os operadores seguem runbooks publicados para rotação de registro, transições políticas e retirada de emergência para manter atualizações deterministas.

## Trabalho pendente
- Benchmarks de parâmetros Halo2 (tamanho de circuito, estratégia de pesquisa) e registrador de resultados no playbook de calibração para que os padrões de gás/tempo limite sejam atualizados junto com a atualização próxima de `confidential_assets_calibration.md`.
- Finalizar políticas de divulgação de auditor e APIs de visualização seletiva associadas, conectando o fluxo aprovado em Torii uma vez que o borrador de governança se firmar.
- Estende o esquema de criptografia de testemunha para capturar saídas multi-destinatários e memorandos em lote, documentando o formato do envelope para implementadores de SDK.
- Fazer uma revisão de segurança externa de circuitos, registros e procedimentos de rotação de parâmetros e arquivar hallazgos junto com os relatórios internos de auditoria.
- Especificar APIs de reconciliação de gastos para auditores e publicar guia de alcance de view-key para que fornecedores de carteiras implementem as mesmas semânticas de atestação.

## Fases de implementação
1. **Fase M0 - Endurecimiento Stop-Ship**
   - [x] A derivação do nulificador agora segue o dispositivo Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com ordem determinista de compromissos forzados em atualizações do razão.
   - [x] A execução aplica limites de tamanho de prova e cotas confidenciais por transação/por bloco, rejeitando transações fora de presunção com erros deterministas.
   - [x] El handshake P2P anuncia `ConfidentialFeatureDigest` (digest de backend + impressões digitais de registro) e falha na forma determinista via `HandshakeConfidentialMismatch`.
   - [x] Remover panics em caminhos de execução confidenciais e adicionar role gating para nós sem suporte.
   - [ ] Aplicar pressupostos de tempo limite de seleção e limites de profundidade de reorganização para pontos de verificação de fronteira.
     - [x] Pressupostos de tempo limite de verificação aplicados; provas que excedem `verify_timeout_ms` agora caíram deterministamente.
     - [x] Frontier checkpoints agora respeitam `reorg_depth_bound`, podando checkpoints mais antigos que a janela configurada enquanto mantêm snapshots deterministas.
   - Apresentar `AssetConfidentialPolicy`, política FSM e portões de aplicação para instruções mint/transfer/reveal.
   - Comprometa `conf_features` em cabeçalhos de bloco e recuse a participação de validadores quando os resumos de registro/parâmetros divergem.
2. **Fase M1 - Registros e parâmetros**
   - Entregar registros `ZkVerifierEntry`, `PedersenParams`, e `PoseidonParams` com operações de governança, anulação de gênese e gerenciamento de cache.
   - Conectar syscall para exigir pesquisas de registro, IDs de programação de gás, hash de esquema e verificações de tamanho.
   - Enviar formato de carga útil criptografado v1, vetores de derivação de chaves para carteira e suporte CLI para gerenciamento de chaves confidenciais.
3. **Fase M2 - Gás e desempenho**
   - Implementar cronograma de determinista de gás, contadores por bloco, e chicotes de benchmark com telemetria (latência de verificação, tamanos de prova, rechazos de mempool).
   - Endurecer CommitmentTree checkpoints, carga LRU, e índices de anulação para cargas de trabalho multi-asset.
4. **Fase M3 - Rotação e ferramental da carteira**
   - Habilitar aceitação de provas multiparâmetro e multiversão; apoiar ativação/descontinuação impulsionada por governança com runbooks de transição.
   - Entregar fluxos de migração em SDK/CLI, fluxos de trabalho de verificação de auditoria e ferramentas de reconciliação de gastos.
5. **Fase M4 - Auditoria e operações**
   - Prove fluxos de trabalho de chaves de auditoria, APIs de divulgação seletiva e runbooks operacionais.
   - Programar revisão externa de criptografia/segurança e publicar hallazgos em `status.md`.Cada fase atualiza marcos do roteiro e testes associados para manter garantias de execução determinista no blockchain vermelho.