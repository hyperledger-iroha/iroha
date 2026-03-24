---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Atos confidenciais e transferências ZK
descrição: Blueprint Phase C para a circulação cega, registros e controles operacionais.
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design de ativos confidenciais e transferências ZK

## Motivação
- Livre o fluxo de ações cegas opt-in para que os domínios preservem a transação de privacidade sem modificar a circulação transparente.
- Garanta uma execução determinada em validadores heterogêneos de hardware e preserve Norito/Kotodama ABI v1.
- Fornecer auditores e operar controles de ciclo de vida (ativação, rotação, revogação) para circuitos e parâmetros criptográficos.

## Modelo de ameaça
- Os validadores são honestos, mas curiosos: eles executam o consenso fielmente mais do que o registro/estado do inspetor.
- Os observadores registram os dados do bloco e as fofocas de transações; nenhuma hipótese de canais de fofoca privada.
- Fora do escopo: analisar o off-ledger de tráfego, os adversários quantiques (acompanhando o roadmap PQ), os ataques de disponibilidade do ledger.

## Vue d conjunto du design
- Les actifs podem declarar uma *piscina blindada* e mais saldos transparentes existentes; a circulação cega é representada por meio de compromissos criptográficos.
- As notas encapsuladas `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independente da ordem das notas.
  - Número de carga útil: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- As transações que transportam cargas úteis `ConfidentialTransfer` codificam o conteúdo Norito:
  - Entradas públicas: âncora Merkle, anuladores, novos compromissos, ID de ativo, versão de circuito.
  - Cargas úteis definidas para destinatários e opções de auditores.
  - Preuve conhecimento zero atestado conservação de valor, propriedade e autorização.
- A verificação de chaves e conjuntos de parâmetros são controlados por meio de registros no livro-razão com janelas de ativação; as noeus recusaram validar as provas que referenciavam entradas inconnuas ou revogadas.
- Os cabeçalhos de consenso envolvem o resumo da funcionalidade confidencial ativa para que os blocos sejam aceitos somente se o registro/parâmetros corresponderem.
- A construção das provas utiliza uma pilha Halo2 (Plonkish) sem configuração confiável; Groth16 ou outras variantes SNARK são intencionalmente não suportadas na v1.

### Jogos determinados

Os envelopes de memorandos confidenciais livres são desormados em um dispositivo canônico para `fixtures/confidential/encrypted_payload_v1.json`. O conjunto de dados captura um envelope v1 positivo e os resultados são malformados para que os SDKs possam afirmar a parte de análise. Os testes do modelo de dados Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e a suíte Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) foram carregados diretamente no dispositivo, garantindo que a codificação Norito, as superfícies de erro e a cobertura de regressão permaneçam alinhadas com a evolução do codec.

Os SDKs Swift podem manter o emetre de instruções escudo sem cola JSON sob medida: construir um
`ShieldRequest` com comprometimento de 32 bytes, carga útil chiffre e metadados de débito,
então ligue para `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para assinar e retransmitir
transação via `/v1/pipeline/transactions`. Le helper valide les longueurs de compromisso,
Insira `ConfidentialEncryptedPayload` no codificador Norito e reflita o layout `zk::Shield`
ci-dessous afin que as carteiras restantes são sincronizadas com Rust.## Compromissos de consenso e controle de capacidade
- Os cabeçalhos dos blocos expostos `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o resumo participa do hash de consenso e iguala o local do registro para aceitar um bloco.
- A governança pode ocorrer no cenário de atualizações no programa `next_conf_features` com um futuro `activation_height`; jusqu a esta altivez, os produtores de blocos devem continuar a seguir o resumo precedente.
- Os noeuds validadores DOIVENT funcionam com `confidential.enabled = true` e `assume_valid = false`. As verificações de descasque recusaram a entrada no conjunto de validadores se uma das condições ecoasse ou se o `conf_features` divergesse localmente.
- Os metadados de handshake P2P incluem `{ enabled, assume_valid, conf_features }`. Os pares anunciaram recursos não-prêmios cobrados, foram rejeitados com `HandshakeConfidentialMismatch` e nunca entraram na rotação de consenso.
- Os resultados do handshake entre validadores, observadores e pares são capturados na matriz de handshake sob [Negociação de capacidade do nó] (#node-capability-negotiation). A verificação do aperto de mão expôs `HandshakeConfidentialMismatch` e o peer fora da rotação de consenso apenas quando o resumo corresponde.
- Les observadores não validadores podem definir `assume_valid = true`; Eles aplicam os deltas confidenciais para o público mais influente na segurança do consenso.

## Políticas e ações
- Toda definição de ato de transporte de um `AssetConfidentialPolicy` definida pelo criador ou por meio da governança:
  - `TransparentOnly`: modo par padrão; somente as instruções transparentes (`MintAsset`, `TransferAsset`, etc.) são permitidas e as operações protegidas são rejeitadas.
  - `ShieldedOnly`: toda emissão e toda transferência devem ser utilizadas com instruções confidenciais; `RevealConfidential` é interdito para que os saldos não sejam jamais expostos à publicação.
  - `Convertible`: os suportes podem substituir o valor entre as representações transparentes e protegidas por meio das instruções da rampa de ativação/desativação ci-dessous.
- As políticas seguem uma restrição do FSM para evitar bloqueios de fundos:
  - `TransparentOnly -> Convertible` (ativação imediata du pool blindado).
  - `TransparentOnly -> ShieldedOnly` (requer transição pendente e janela de conversão).
  - `Convertible -> ShieldedOnly` (delay mínimo imposto).
  - `ShieldedOnly -> Convertible` (plano de migração necessário para que as notas protegidas possam ser gastas).
  - `ShieldedOnly -> TransparentOnly` é interdito salvo se o pool protegido for vide ou se a governança codificar uma migração que desproteja as notas restantes.
- As instruções de governança fixam `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via ISI `ScheduleConfidentialPolicyTransition` e podem anular programas de alterações com `CancelConfidentialPolicyTransition`. A memória de validação garante que qualquer transação não substitua a arrogância da transição e a inclusão ecoe a determinística se uma verificação da política mudar no meio do bloco.
- As transições pendentes são aplicadas automaticamente à abertura de um novo bloco: uma vez que a altivez entre a janela de conversão (para as atualizações `ShieldedOnly`) ou atteint `effective_height`, o tempo de execução com um dia `AssetConfidentialPolicy`, rafraichit la metadata `zk.policy` e apague a entrada pendente. Se o fornecimento transparente não for nulo quando uma transição `ShieldedOnly` chegar, o tempo de execução anulará a alteração e registrará um anúncio, deixando o modo precedente intacto.
- Os botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` impõem um aviso mínimo e períodos de carência para permitir a conversão automática de carteiras no switch.
- `pending_transition.transition_id` também é usado para lidar com auditoria; a governança deve ser citada durante a finalização ou a anulação para que os operadores possam correlacionar os relatórios de entrada/saída.
- `policy_transition_window_blocks` padrão é 720 (~12 horas com tempo de bloqueio de 60 s). As noeus apertam as exigências de governança que tentam um aviso mais tribunal.
- Genesis manifesta e flui CLI expõe políticas correntes e pendentes. A lógica de admissão acendeu a política no momento da execução para confirmar que cada instrução confidencial está autorizada.
- Checklist de migração - veja "Sequenciamento de migração" ci-dessous para o plano de atualização nas etapas seguintes do Milestone M0.

#### Monitoramento de transições via ToriiCarteiras e auditores interrogadores `GET /v1/confidential/assets/{definition_id}/transitions` para inspecionar l `AssetConfidentialPolicy` ativo. A carga útil JSON inclui sempre o id de ativo canônico, o último alto nível de bloco observado, o `current_mode` da política, o modo efetivo para este alto nível (as janelas de conversão relatadas temporariamente `Convertible`) e os identificadores presentes em `vk_set_hash`/Poseidon/Pedersen. Quando uma transição de governança está atenta à resposta incorporada também:

- `transition_id` - manipula d reenvio de auditoria par `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e o `window_open_height` derivam (o bloco ou as carteiras devem iniciar a conversão para os cortes ShieldedOnly).

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

Uma resposta `404` indica que nenhuma definição de ato correspondente existe. Durante qualquer transição, o campo `pending_transition` está planejado para ser `null`.

### Máquina de dados políticos

| Modo atual | Modo seguinte | Pré-requisito | Gestão de altivez eficaz | Notas |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | Governança ativa entradas de registro de verificador/parâmetros. Soumettre `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | A transição é executada exatamente como `effective_height`; a piscina blindada deve estar disponível imediatamente.        | Chemin par defaut para ativar a confidencialidade e proteger os fluxos transparentes. |
| Somente Transparente | Somente blindado | Idem ci-dessus, mais `policy_transition_window_blocks >= 1`.                                                         | O tempo de execução entre automaticamente em `Convertible` a `effective_height - policy_transition_window_blocks`; passe um `ShieldedOnly` a `effective_height`. | Janela de conversão determinada antes de desativar as instruções transparentes. |
| Conversível | Somente blindado | Programa de transição com `effective_height >= current_height + policy_transition_delay_blocks`. Certificador de governança DEVRAIT (`transparent_supply == 0`) via auditoria de metadados; le runtime l applique au cut-over. | Semântica de fenêtre idêntico. Se o fornecimento transparente não for nulo para `effective_height`, a transição deve ser feita com `PolicyTransitionPrerequisiteFailed`. | Verrouille l ativo em circulação totalmente confidencial.                             |
| Somente blindado | Conversível | Programa de transição; nenhum retrait d urgência ativo (`withdraw_height` não definido).                                  | L etat bascule a `effective_height`; as rampas revelam rouvrent tandis que as notas protegidas permanecem válidas.       | Utilize para janelas de manutenção ou revistas de auditoria.                               |
| Somente blindado | Somente Transparente | Governança doit comprovar `shielded_supply == 0` ou preparar um plano `EmergencyUnshield` assinado (assinaturas de requisitos do auditor). | O tempo de execução abre uma janela `Convertible` antes de `effective_height`; à la hauteur, as instruções confidenciais ecoam durante e o ativo revient no modo somente transparente. | Sortie de dernier recorre. A transição é anulada automaticamente se uma nota confidencial for dependente da janela. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` limpe a alteração e observe.                                              | `pending_transition` está desativado imediatamente.                                                                       | Manter o status quo; indique para completude.                                         |

As transições não listadas ci-dessus são rejeitadas pela governança da submissão. O tempo de execução verifica os pré-requisitos necessários antes de aplicar um programa de transição; em caso de verificação, ele repousou o ato no modo precedente e emet `PolicyTransitionPrerequisiteFailed` via telemetria e eventos de bloco.

### Sequenciamento de migração1. **Preparar os registros:** ativa todas as entradas, verificador e parâmetros referenciados pela política cível. As noeus anunciaram o `conf_features` resultante para que os pares verificassem a coerência.
2. **Planificador de transição:** adicionado `ScheduleConfidentialPolicyTransition` com um `effective_height` correspondente a `policy_transition_delay_blocks`. Na versão `ShieldedOnly`, é preciso uma janela de conversão (`window >= policy_transition_window_blocks`).
3. **Publique o operador de orientação:** registre o retorno `transition_id` e o difusor em um runbook on/off-ramp. Carteiras e auditores estão abonnent a `/v1/confidential/assets/{id}/transitions` para conhecer a altivez da ouverture de fenetre.
4. **Aplicação da janela:** na abertura, o tempo de execução bascula a política em `Convertible`, emet `PolicyTransitionWindowOpened { transition_id }`, e começa a rejeitar as demandas de governança em conflito.
5. **Finalizar ou cancelar:** em `effective_height`, o tempo de execução verifica os pré-requisitos (fornecer zero transparente, passo de retorno de urgência, etc.). Com sucesso, a política passa ao modo exigido; em echec, `PolicyTransitionPrerequisiteFailed` é emis, a transição pendente é líquida e a política restante é alterada.
6. **Atualizações de esquema:** após uma transição rápida, a governança aumenta a versão do esquema do ativo (par ex `asset_definition.v2`) e a CLI de ferramentas exige `confidential_policy` durante a serialização dos manifestos. Os documentos de atualização genesis instruem os operadores a adicionar configurações políticas e empreintes de registro antes de redefinir os validadores.

As novas pesquisas que foram realizadas com a confidencialidade ativa codificaram a política desejada diretamente na gênese. A seguir, meme a lista de verificação ci-dessus durante as alterações pós-lançamento, para que as janelas de conversão permaneçam determinadas e as carteiras sejam ajustadas no tempo.

### Versão e ativação dos manifestos Norito

- Les genesis manifests DOIVENT incluem um `SetParameter` para a chave personalizada `confidential_registry_root`. A carga útil é um Norito JSON correspondente a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: adicione o campeão (`null`) quando alguma entrada não estiver ativa, sem fornecer uma cadeia hexadecimal de 32 bytes (`0x...`) igual ao produto hash par `compute_vk_set_hash` nas instruções do verificador do manifesto. As pessoas se recusaram a demarcar se o parâmetro fosse alterado ou se o hash divergesse das codificações do registro de escrituras.
- Le on-wire `ConfidentialFeatureDigest::conf_rules_version` inicia a versão do layout do manifesto. Para as pesquisas v1, restaure `Some(1)` e iguale `iroha_config::parameters::defaults::confidential::RULES_VERSION`. À medida que o conjunto de regras evolui, incrementa a constante, regenera os manifestos e implementa os binários em lock-step; Misture as versões para rejeitar os blocos pelos validadores com `ConfidentialFeatureDigestMismatch`.
- Os manifestos de ativação DEVRAIENT reagrupam erros no dia do registro, alterações no ciclo de vida dos parâmetros e transições políticas para que o resumo permaneça coerente:
  1. Aplique as mutações de registro planejadas (`Publish*`, `Set*Lifecycle`) em um modo offline do estado e calcule o resumo pós-ativação com `compute_confidential_feature_digest`.
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usa o cálculo de hash para que os pares recuperem o resumo do meme correto e avaliem as instruções intermediárias.
  3. Adicione as instruções `ScheduleConfidentialPolicyTransition`. Cada instrução doit cita le `transition_id` emis par governança; Os manifestos que estão abertos serão rejeitados pelo tempo de execução.
  4. Mantenha os bytes do manifesto, uma impressão SHA-256 e o ​​resumo utilizados no plano de ativação. Os operadores verificam os três artefatos antes de votar no manifesto para evitar partições.
- Quando os lançamentos exigem um corte diferente, registre a alta qualidade em um parâmetro personalizado (par ex `custom.confidential_upgrade_activation_height`). Isso fornece aos auditores uma prévia do código Norito e que os validadores devem respeitar a janela de aviso antes que a alteração do resumo tenha efeito.## Ciclo de vida de verificadores e parâmetros
### Registrar ZK
- O registro contábil `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ou `proving_system` está atualmente fixo em `Halo2`.
- Os pares `(circuit_id, version)` são únicos globalmente; o registro mantém um índice secundário para pesquisas por metadados de circuito. As tentativas de registrar um par duplicado são rejeitadas durante a admissão.
- `circuit_id` deve ser não vide e `public_inputs_schema_hash` deve ser fornecido (normalmente um hash Blake2b-32 da codificação canônica de entradas públicas do verificador). A admissão rejeita os registros que omitem esses campeões.
- As instruções de governança incluem:
  - `PUBLISH` para adicionar uma entrada `Proposed` com apenas metadados.
  - `ACTIVATE { vk_id, activation_height }` para ativação do programador em uma época limitada.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar o final de alta ou as provas podem ser referenciadas na entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento com urgência; les actifs touches geleront les depenses confidentielles apres retirar altura jusqu a ativação de novas entradas.
- Os manifestos do genesis emitem automaticamente um parâmetro personalizado `confidential_registry_root` que não `vk_set_hash` corresponde a entradas ativas; a validação cruzou este resumo com o estado do registro local antes de uma nova reunião poder reunir o consenso.
- Registrar ou fornecer no momento um verificador que requer um `gas_schedule_id`; a verificação impõe que a entrada seja `Active`, presente no índice `(circuit_id, version)`, e que as provas Halo2 forneçam um `OpenVerifyEnvelope` não `circuit_id`, `vk_hash`, e `public_inputs_schema_hash` correspondente uma entrada do registro.

### Provando Chaves
- As chaves de prova restantes fora do livro-razão são mais referenciadas pelos endereços de identificação do conteúdo (`pk_cid`, `pk_hash`, `pk_len`) publicadas com os metadados do verificador.
- A carteira dos SDKs recupera o PK, verifica os hashes e armazena a localização do cache.

### Parâmetros Pedersen e Poseidon
- Os registros separados (`PedersenParams`, `PoseidonParams`) espelham os controles de ciclo de vida dos verificadores, como `params_id`, hashes de geradores/constantes, ativação, depreciação e taxa de retirada.
- Os compromissos e os hashes geram uma separação de domínio por `params_id` para que a rotação de parâmetros não seja reutilizada jamais por padrões de bits e conjuntos obsoletos; l ID é embarcado nos compromissos de notas e nas tags do domínio anulador.
- Os circuitos suportam seleção multiparâmetros e verificação; Os conjuntos de parâmetros são depreciados e gastos apenas no `deprecation_height`, e os conjuntos são retirados exatamente como `withdraw_height`.

## Ordem determinante e anuladora
- Cada ativo mantém um `CommitmentTree` com `next_leaf_index`; os blocos acrescentam os compromissos em uma ordem determinada: iteram as transações na ordem do bloco; em cada transação itera as saídas protegidas por `output_idx` serializar ascendente.
- `note_position` deriva des offsets de l arbre mais **ne** fait pas partie du nullifier; il ne sert qu aux chemins de member dans le testemunho de preuve.
- A estabilidade dos anuladores sob reorgs é garantida pelo design PRF; l input PRF lie `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e as âncoras referenciam os limites históricos das raízes de Merkle par `max_anchor_age_blocks`.## Fluxo do razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Requer a política de ação `Convertible` ou `ShieldedOnly`; l admissão verifique l autorite, recupere `params_id`, echantillonne `rho`, emet le compromisso, met a jour l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` com novo compromisso, raiz delta de Merkle e hash de chamada de transação para trilhas de auditoria.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - O syscall VM verifica a prova por meio da entrada do registro; o host garante que os nulificadores sejam inutilizados, que os compromissos sejam adicionados determinísticos e que a âncora seja recente.
   - O livro-razão registra as entradas `NullifierSet`, armazena as cargas úteis para destinatários/auditores e emet `ConfidentialEvent::Transferred` resulta em nulificadores, ordens de saída, hash de prova e raízes Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - Disponível exclusivamente para os ativos `Convertible`; a prova válida é que o valor da nota é igual ao montante revelado, o livro-razão credita o saldo transparente e quebra a nota protegida e marca o nulificador comme depense.
   - Emet `ConfidentialEvent::Unshielded` com montante público, anuladores de consumo, identificadores de prova e hash de chamada de transação.

## Ajouts au modelo de dados
- `ConfidentialConfig` (nova seção de configuração) com ativação de flag d, `assume_valid`, botões de gás/limites, janela de âncora, backend de verificador.
- `ConfidentialNote`, `ConfidentialTransfer` e `ConfidentialMint` esquemas Norito com byte de versão explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envelope de memo bytes AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, padrão `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para o layout XChaCha20-Poly1305.
- Os vetores canônicos de derivação de cle vivent em `docs/source/confidential_key_vectors.json`; o CLI e o endpoint Torii regridem nesses equipamentos.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste a ligação `(backend, name, commitment)` para os verificadores transferirem/desprotegerem; A execução rejeita as provas e a chave de verificação referenciada ou inline não corresponde ao compromisso registrado.
- `CommitmentTree` (par ativo com pontos de verificação de fronteira), `NullifierSet` e `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` estoques em estado mundial.
- Mempool mantém estruturas transitórias `NullifierIndex` e `AnchorIndex` para detecção precoce de duplicações e verificações de d idade d âncora.
- As atualizações do esquema Norito incluem uma ordenação canônica de entradas públicas; Os testes de ida e volta garantem o determinismo da codificação.
- As viagens de ida e volta da carga útil criptografada são executadas por meio de testes de unidade (`crates/iroha_data_model/src/confidential.rs`). A carteira de vetores acompanha as transcrições canônicas da AEAD para os auditores. `norito.md` documenta o cabeçalho on-wire do envelope.

## Integração IVM e syscall
- Introduza o syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, e o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - O syscall cobra o verificador de metadados a partir do registro, aplica os limites de taille/temps, fabrica um gás determinado e aplica o delta que se a prova for reusada.
- O host expõe uma característica somente leitura `ConfidentialLedger` para recuperar instantâneos Merkle root e o status dos nulificadores; a biblioteca Kotodama fornece ajudantes para montagem de testemunha e validação de esquema.
- Os documentos pointer-ABI foram lançados hoje para esclarecer o layout do buffer de prova e os identificadores do registro.

## Negociação de capacidades de noeud
- O aperto de mão anuncia `feature_bits.confidential` com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. A participação dos validadores requer `confidential.enabled=true`, `assume_valid=false`, identificadores de backend verificadores idênticos e resumos correspondentes; as incompatibilidades ecoam o aperto de mão com `HandshakeConfidentialMismatch`.
- A configuração suportada `assume_valid` para os observadores apenas: quando desativada, reencontre as instruções confidenciais do produto `UnsupportedInstruction` determinado sem pânico; Quando ativos, os observadores aplicam os deltas declaram sem verificar as provas.
- Mempool rejeita transações confidenciais se a capacidade local estiver desativada. Os filtros de fofoca evitam enviar transações blindadas para pares sem capacidade correspondente, mas retransmitem para os verificadores de IDs desconhecidos nos limites de cauda.

### Matriz de aperto de mão| Anuncie distante | Resultados para validadores | Operador de notas |
|------------------|------------------|----------------|
| `enabled=true`, `assume_valid=false`, correspondência de back-end, correspondência de resumo | Aceitar | Le peer atteint l etat `Ready` et participe da la proposição, au vote, et au fan-out RBC. Aucune action manuelle requise. |
| `enabled=true`, `assume_valid=false`, correspondência de backend, resumo obsoleto ou ausente | Rejeitar (`HandshakeConfidentialMismatch`) | O controle remoto aplica os registros/parâmetros de ativação ao atender ou atender ao planejamento `activation_height`. Tanto que corrige, le noeud reste decouvrable mais nunca entre em rotação de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeitar (`HandshakeConfidentialMismatch`) | Os validadores exigem a verificação das provas; configure o observador remoto com entrada apenas Torii ou basculer `assume_valid=false` após a ativação da verificação ser concluída. |
| `enabled=false`, champs omis (construção obsoleta), ou verificador de backend diferente | Rejeitar (`HandshakeConfidentialMismatch`) | Pares obsoletos ou atualizações parciais não podem voltar a juntar-se à rede de consenso. Leia o lançamento atual e certifique-se de que o backend + resumo da tupla corresponda antes da reconexão. |

Os observadores que realizaram voluntariamente a verificação de provas não devem abrir conexões de consenso com os validadores ativos. Ele pode sempre ingerir blocos via Torii ou APIs de arquivamento, mas a reserva de consenso é rejeitada apenas quando as capacidades correspondentes são anunciadas.

### Política de poda revelada e retenção de anuladores

Os livros-razões confidenciais devem ser conservados com base no histórico para provar o fracasso das notas e rejubilar o governo das auditorias. A política por padrão, aplicada por `ConfidentialLedger`, é:

- **Retenção de anuladores:** conserve as dependências anuladoras por um *mínimo* de `730` dias (24 meses) após a alta da dependência, ou a janela imposta pelo regulador se mais longo. Os operadores podem abrir a janela via `confidential.retention.nullifier_days`. Os anuladores mais jogos que a janela DOIVENT restam interrogáveis ​​via Torii para que os auditores provem a ausência de gasto duplo.
- **Pruning des revela:** as revelações transparentes (`RevealConfidential`) removem os compromissos associados imediatamente após a finalização do bloco, mas o nulificador consomme permanece sob a regra de retenção ci-dessus. Os eventos `ConfidentialEvent::Unshielded` registram o monte público, o destinatário e o hash de prova para que a reconstrução das revelações históricas não exija a remoção do texto cifrado.
- **Pontos de controle de fronteira:** as fronteiras de compromisso mantêm os pontos de controle roulants cobrindo o maior de `max_anchor_age_blocks` e a janela de retenção. As noites compactam os pontos de verificação e os antigos somente após a expiração de todos os anuladores no intervalo.
- **Remediation digest obsoleto:** se `HandshakeConfidentialMismatch` sobreviver a uma causa de um desvio de digest, os operadores devem (1) verificar se as janelas de retenção de nulificadores estão alinhadas no cluster, (2) lancer `iroha_cli app confidential verify-ledger` para regenerar o digest no conjunto de nullifiers retenus, e (3) reimplantar le manifest rafraichi. Tout nullifier prune trop tot doit etre restaure depuis le stockage froid antes de reunir a reserva.

Documente as substituições locais nas operações do runbook; As políticas de governança que possuem a janela de retenção devem apresentar a configuração dos noeus e os planos de armazenamento e arquivamento em sincronia.

### Fluxo de despejo e recuperação

1. Pingente le dial, `IrohaNetwork` compare as capacidades anunciadas. Tout incompatibilidade nível `HandshakeConfidentialMismatch`; a conexão está fechada e o par permanece no arquivo de descoberta sem ser enviado para `Ready`.
2. L echec est exposto via le log du service reseau (incluindo resumo remoto e backend), et Sumeragi não planeja nunca le peer para proposição ou voto.
3. Os operadores remediam o alinhamento do verificador de registros e conjuntos de parâmetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou um programa `next_conf_features` com um `activation_height` convencional. Quando o resumo está alinhado, o aperto de mão prochain é reativado automaticamente.
4. Se um par obsoleto reusar um difusor em um bloco (por exemplo, por meio da repetição do arquivo), os validadores rejeitarão a determinação com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, observando o estado do registro coerente na reserva.

### Fluxo de handshake seguro para repetição1. Cada tentativa de tentativa de usar um novo material de cle Noise/X25519. A carga útil do sinal de handshake (`handshake_signature_payload`) concatena os efímeros públicos locais e distantes, o endereço do soquete é codificado Norito e - quando compila com `handshake_chain_id` - o identificador da cadeia. A mensagem é obrigatória AEAD antes de sair do noeud.
2. O respondente recalcula a carga útil com a ordem de peer/local inverse e verifica a assinatura Ed25519 embarcada em `HandshakeHelloV1`. Parce que les deux cles ephimeres et l endereço annoncee font partie du domaine de assinatura, rejouer uma captura de mensagem contra um outro peer ou recuperar uma conexão obsoleta echoue deterministiquement.
3. As bandeiras de capacidade confidencial e o agente `ConfidentialFeatureDigest` em `HandshakeConfidentialMeta`. O receptor compara a tupla `{ enabled, assume_valid, verifier_backend, digest }` com o filho `ConfidentialHandshakeCaps` local; toda incompatibilidade é classificada com `HandshakeConfidentialMismatch` antes que o transporte não passe para `Ready`.
4. Os operadores DOIVENT recomputam o resumo (via `compute_confidential_feature_digest`) e redemitem os noeuds com registros/políticas mises um dia antes de reconectar. Os pares que anunciam os resumos antigos continuam a repetir o aperto de mão, marcando um estado de reentrada no conjunto de validadores.
5. O sucesso e as verificações de aperto de mão atingiram o padrão do computador `iroha_p2p::peer` (`handshake_failure_count`, ajudantes de taxonomia para errar) e emitem estruturas de log com o ID do par distante e a impressão do resumo. Monitore esses indicadores para detectar replays ou configurações incorretas durante a implementação.

## Gerenciamento de arquivos e cargas úteis
- Hierarquia de derivação por conta:
  - `sk_spend` -> `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- As cargas úteis de notas chiffre são utilizadas AEAD com chaves compartilhadas derivadas de ECDH; des view keys d auditeur optionnelles podem ser adidos aux outputs de acordo com a política do ativo.
- Adiciona CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, auditor de ferramentas para deschiffrer memos, e o ajudante `iroha app zk envelope` para produzir/inspecionar envelopes Norito offline. Torii expõe o fluxo de derivação do meme via `POST /v1/confidential/derive-keyset`, retornando as formas hexadecimais e base64 para que as carteiras possam recuperar as hierarquias de sua programação.

## Gás, limites e controles DoS
- Cronograma de gás determinado:
  - Halo2 (Plonkish): gás base `250_000` + gás `2_000` par entrada pública.
  - Byte de prova por par de gás `5`, mais taxas por anulador (`300`) e compromisso por par (`500`).
  - Os operadores podem sobrecarregar essas constantes através do nó de configuração (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); As alterações são propagadas ao desmarrage ou quando a configuração do hot-reload é aplicada e as aplicações são determinísticas no cluster.
- Limites de duração (padrões configuráveis):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. As provas despassantes `verify_timeout_ms` abortaram a instrução de determinística (governança de cédulas emettent `proof verification exceeded timeout`, `VerifyProof` retornou um erro).
- Cotas adicionais garantidas à vida: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` e `max_public_inputs` nascem dos construtores de blocos; `reorg_depth_bound` (>= `max_anchor_age_blocks`) controla a retenção dos pontos de verificação de fronteira.
- O tempo de execução de execução é rejeitado, mantendo as transações fora dos limites por transação ou por bloco, emite erros `InvalidParameter` determinados e mantém o estado do livro-razão intacto.
- Mempool pré-filtra as transações confidenciais por `vk_id`, prolonga a prova e a idade da âncora antes de solicitar o verificador para suportar o uso dos recursos.
- A verificação arrete determinística em tempo limite ou violação de borne; as transações ecoam com erros explícitos. Os back-ends SIMD são opcionais, mas não modificam a capacidade de gás.

### Linhas de base de calibração e portas de aceitação
- **Placas-formas de referência.** As execuções de calibração DOIVENT cobrem os três perfis de hardware ci-dessous. Les run ne couvrant pas tous les profiles sont rejetes en review.| Perfil | Arquitetura | CPU/Instância | Compilador de bandeiras | Objetivo |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabeleça valores planejados sem vetores intrínsecos; utilize para regular as tabelas de substitutos. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | liberar por padrão | Valide o caminho AVX2; verifique se as acelerações SIMD permanecem na tolerância do gás neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | liberar por padrão | Certifique-se de que o backend NEON seja determinado e alinhado com as programações x86. |

- **Arnês de referência.** Todos os relatórios de gás de calibração DOIVENT etre produtos com:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar o dispositivo determinado.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` quando os valores da VM do opcode são alterados.

- **Aleatoriedade corrigida.** Exportador `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de lançar os bancos para que `iroha_test_samples::gen_account_in` bascule sur la voie determinista `KeyPair::from_seed`. Le chicote imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma única vez; se a variável for menor, a revisão DOIT ecoará. Todo novo utilitário de calibração deve continuar honrando este ambiente durante a introdução e alea auxiliar.

- **Captura dos resultados.**
  - Critério de upload de currículos (`target/criterion/**/raw.csv`) para cada perfil no produto de lançamento.
  - Armazene as métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Ledger de calibração de gás confidencial] (./confidential-gas-calibration) com o commit git e a versão do compilador utilizada.
  - Conservar as duas últimas linhas de base por perfil; suprime os snapshots mais antigos e foi o relacionamento mais recente válido.

- **Tolerâncias e aceitação.**
  - Os deltas de gás entre `baseline-simd-neutral` e `baseline-avx2` DOIVENT rester <= +/-1,5%.
  - Os deltas de gás entre `baseline-simd-neutral` e `baseline-neon` DOIVENT rester <= +/-2,0%.
  - As propostas de calibração passam por aqueles que exigem ajustes de cronograma ou um RFC explícito no cartão e na mitigação.

- **Lista de verificação de revisão.** Os remetentes são responsáveis por:
  - Inclua `uname -a`, extratos de `/proc/cpuinfo` (modelo, revisão) e `rustc -Vv` no registro de calibração.
  - Verificador de que `IROHA_CONF_GAS_SEED` aparece na bancada de saída (as bancadas imprimem a semente ativa).
  - Certifique-se de que o recurso sinaliza o marca-passo e o verificador confiável do espelho da produção (`--features confidential,telemetry` nos bancos com telemetria).

## Configuração e operações
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
- Telemetria emite des métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total`, sans jamais expor des donnees en clair.
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## Estratégia de testes
- Determinismo: o embaralhamento aleatório de transações nos blocos feitos com raízes Merkle e conjuntos anuladores idênticos.
- Resiliência aux reorg: simulação de reorganizações multiblocos com âncoras; os anuladores permanecem estáveis ​​e as âncoras obsoletas são rejeitadas.
- Invariantes de gás: verifica um uso de gás idêntico entre noites com e sem aceleração SIMD.
- Testes de limite: provas de plafonds de taille/gas, contagens máximas de entrada/saída, aplicação de timeouts.
- Ciclo de vida: operações de governança para ativação/descontinuação do verificador e parâmetros, testes de dependência após rotação.
- Política FSM: transições autorizadas/interditas, atrasos de transição pendentes, e rejeição do mempool autour des hauteurs effect.
- Urgências de registro: retrair d urgência fige les actifs a `withdraw_height` e rejeitar as provas apres.
- Gating de capacidade: validadores com `conf_features` incompatíveis rejeitam blocos; observadores com `assume_valid=true` suivent sem afetar o consenso.
- Equivalência de estado: noeuds validador/completo/observador que produz raízes de estado idênticas na cadeia canônica.
- Fuzzing negatif: provas malformadas, payloads surdimensionnes, et colisões de nullifier rejeitam deterministiquement.## Migração
- Implementação controlada por recursos: apenas quando a Fase C3 termina, `enabled` padrão a `false`; as noeus anunciaram suas capacidades antes de reunir o conjunto de validadores.
- Os atos transparentes não são afetados; as instruções confidenciais exigem registro de entradas e negociação de capacidades.
- Les noeuds compila sem suporte confidencial rejeitando os blocos pertinentes determinísticos; ele não pode se juntar ao conjunto de validadores, mas pode funcionar como observadores com `assume_valid=true`.
- Genesis manifesta incluindo entradas iniciais de registro, conjuntos de parâmetros, políticas confidenciais para os ativos e chaves de opção do auditor.
- Os operadores seguem os runbooks publicados para rotação de registro, transições políticas e retrocesso de urgência para manter as atualizações determinadas.

## Travail restante
- Faça benchmarking de conjuntos de parâmetros Halo2 (taille de circuito, estratégia de pesquisa) e registre os resultados no playbook de calibração para monitorar continuamente os padrões de gás/tempo limite com a atualização prochain `confidential_assets_calibration.md`.
- Finalizar as políticas de divulgação do auditor e as APIs dos associados de visualização seletiva, cabendo ao fluxo de trabalho aprovado em Torii um foi o projeto de governança assinado.
- Crie o esquema de criptografia de testemunha para cobrir as saídas de vários destinatários e memorandos em lotes e documente o formato de envelope para os implementadores SDK.
- Comissário uma revista de segurança externa de circuitos, registros e procedimentos de rotação de parâmetros e arquivador de conclusões na sequência de relatórios de auditoria interna.
- Especifica APIs de reconciliação de gastos para auditores e publica a orientação do escopo view-key para que os fornecedores de carteiras implementem os memes semânticos de atestado.## Fases da implementação
1. **Fase M0 - Endurecimento Stop-Ship**
   - [x] A derivação do nulificador mantém o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com uma ordem determinante de compromissos imposta nas atualizações do razão.
   - [x] L execução aplica des plafonds de taille de prova e des quotas confidenciais por transação/par bloco, rejeitando as transações fora do orçamento com erros determinísticos.
   - [x] O handshake P2P anuncia `ConfidentialFeatureDigest` (resumo de backend + registro de impressões digitais) e ecoa a determinação de incompatibilidades via `HandshakeConfidentialMismatch`.
   - [x] Retire os pânicos dos caminhos de execução confidencial e adicione um role gating para os noeuds sem preço de custo.
   - [] Aplique os limites de tempo limite do verificador e os suportes do profundor de reorganização para os pontos de verificação de fronteira.
     - [x] Orçamentos de timeout de aplicações de verificação; as provas deixadas `verify_timeout_ms` ecoam a manutenção da determinística.
     - [x] Os pontos de verificação de fronteira respeitando a manutenção `reorg_depth_bound`, removendo os pontos de verificação mais antigos que a janela configurada é totalmente responsável pelos instantâneos determinados.
   - Introduzindo `AssetConfidentialPolicy`, política FSM e portas de aplicação para instruções mint/transfer/reveal.
   - Confirme `conf_features` nos cabeçalhos do bloco e recuse a participação dos validadores quando os resumos de registro/parâmetros divergentes.
2. **Fase M1 - Registros e parâmetros**
   - Liberar os registros `ZkVerifierEntry`, `PedersenParams` e `PoseidonParams` com governança de operações, gênese de ancrage e gerenciamento de cache.
   - Cablage du syscall para impor pesquisas de registro, IDs de programação de gás, hash de esquema e verificações detalhadas.
   - Publique o formato de carga útil chiffre v1, vetores de derivação de chaves para carteiras e suporte CLI para gerenciamento de chaves confidenciais.
3. **Fase M2 - Gás e desempenho**
   - Implementador cronograma de determinação de gás, computadores por bloco, e chicotes de benchmark com telemetria (latência de verificação, caudas de prova, rejeitos mempool).
   - Durcir pontos de verificação CommitmentTree, cobrança LRU e índices anuladores para cargas de trabalho multiativos.
4. **Fase M3 - Rotação e carteira de ferramentas**
   - Ativação de provas multiparâmetros e multiversão; apoiador l piloto de ativação/descontinuação para governança com runbooks de transição.
   - Livre os fluxos de migração SDK/CLI, fluxos de trabalho do verificador de auditoria e ferramentas de reconciliação de gastos.
5. **Fase M4 - Auditoria e operações**
   - Fornecer fluxos de trabalho de chaves de auditoria, APIs de divulgação seletiva e runbooks operacionais.
   - Planeje uma revisão externa de criptografia/segurança e publique as conclusões em `status.md`.

Cada fase atendeu aos marcos do roteiro e aos testes associados para manter as garantias de execução determinadas do blockchain.