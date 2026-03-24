---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ativos confidenciais e transferências ZK
descrição: circulação protegida, registros e controles do operador کے لئے Fase C کا blueprint۔
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Ativos confidenciais e design de transferência ZK

## Motivação
- fluxos de ativos protegidos opt-in فراہم کرنا تاکہ domínios شفاف circulação بدلے بغیر privacidade transacional برقرار رکھ سکیں۔
- auditores اور operadores کو circuitos اور parâmetros criptográficos کے لئے controles de ciclo de vida (ativação, rotação, revogação) دینا۔

## Modelo de ameaça
- Validadores honestos, mas curiosos ہیں: consenso fiel چلاتے ہیں لیکن razão/estado کو inspecionar کرنے کی کوشش کرتے ہیں۔
- Observadores da rede bloqueiam dados e transações fofocadas دیکھتے ہیں؛ canais de fofoca privados کا کوئی suposição نہیں۔
- Fora do escopo: análise de tráfego fora do razão, adversários quânticos (roteiro PQ میں علیحدہ ٹریک), ataques de disponibilidade de razão

## Visão geral do projeto
- Ativos موجودہ saldos transparentes کے علاوہ *pool protegido* declarar کر سکتے ہیں؛ compromissos criptográficos de circulação blindada سے representam ہوتی ہے۔
- Notas `(asset_id, amount, recipient_view_key, blinding, rho)` کو encapsular کرتی ہیں، ساتھ:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, nota de pedido سے independente۔
  - Carga criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transações com cargas úteis `ConfidentialTransfer` codificadas em Norito لاتی ہیں جن میں:
  - Entradas públicas: âncora Merkle, anuladores, novos compromissos, identificação de ativos, versão do circuito۔
  - Destinatários e auditores opcionais para cargas úteis criptografadas۔
  - Prova de conhecimento zero, conservação de valor, propriedade e atestado de autorização کرتی ہے۔
- Verificando chaves e conjuntos de parâmetros em registros contábeis کے ذریعے janelas de ativação کے ساتھ کنٹرول ہوتے ہیں؛ nós desconhecidos یا entradas revogadas کو referir کرنے والی provas validar کرنے سے انکار کرتے ہیں۔
- Cabeçalhos de consenso resumo de recurso confidencial ativo پر commit کرتے ہیں تاکہ blocos صرف اسی وقت aceitar ہوں جب registro اور correspondência de estado de parâmetro کرے۔
- Pilha Halo2 (Plonkish) de construção de prova استعمال کرتی ہے بغیر configuração confiável; Groth16 یا دیگر SNARK variantes v1 میں دانستہ طور پر não suportado ہیں۔

### Jogos Determinísticos

Envelopes de memorandos confidenciais اب `fixtures/confidential/encrypted_payload_v1.json` میں dispositivo canônico کے ساتھ آتے ہیں۔ یہ conjunto de dados ایک مثبت envelope v1 اور منفی amostras malformadas پکڑتا ہے تاکہ paridade de análise de SDKs ثابت کر سکیں۔ Testes de modelo de dados de ferrugem (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e conjunto Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) fixture e codificação Norito, superfícies de erro e cobertura de regressão codec کے ارتقا کے ساتھ alinhado رہتے ہیں۔

Swift SDKs اب cola JSON sob medida کے بغیر instruções de escudo emitem کر سکتے ہیں: compromisso de nota de 32 bytes, carga útil criptografada e metadados de débito کے ساتھ `ShieldRequest` بنائیں, پھر `/v1/pipeline/transactions` پر sinal e relé کرنے کے لئے `IrohaSDK.submit(shield:keypair:)` (یا `submitAndWait`) کال کریں۔ Comprimentos de compromisso auxiliar validados کو espelho کرتا ہے تاکہ carteiras Rust کے ساتھ lock-step رہیں۔

## Compromissos de consenso e controle de capacidade
- Cabeçalhos de bloco `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` expor کرتے ہیں؛ digerir hash de consenso میں حصہ لیتا ہے اور aceitação de bloco کے لئے visualização do registro local سے correspondência ہونا چاہئے۔
- Governança مستقبل کے `activation_height` کے ساتھ `next_conf_features` پروگرام کر کے estágio de atualizações کر سکتی ہے؛ اس altura تک produtores de bloco پچھلا emissão de resumo کرتے رہتے ہیں۔
- Nós validadores کو `confidential.enabled = true` e `assume_valid = false` کے ساتھ operar کرنا DEVE ہے۔ Conjunto de validador de verificações de inicialização join کرنے سے انکار کرتے ہیں اگر کوئی شرط falha ہو ​​یا local `conf_features` diverge ہو۔
- Metadados de handshake P2P اب `{ enabled, assume_valid, conf_features }` شامل کرتا ہے۔ recursos não suportados anunciam کرنے والے peers `HandshakeConfidentialMismatch` کے ساتھ rejeitar ہوتے ہیں اور rotação de consenso میں داخل نہیں ہوتے۔
- Observadores não validadores `assume_valid = true` سیٹ کر سکتے ہیں؛ وہ deltas confidenciais کو aplicar cegamente کرتے ہیں مگر segurança de consenso پر اثر نہیں ڈالتے۔## Políticas de Ativos
- ہر definição de ativos میں criador یا governança کی طرف سے definir کیا گیا `AssetConfidentialPolicy` ہوتا ہے:
  - `TransparentOnly`: modo padrão; صرف instruções transparentes (`MintAsset`, `TransferAsset` وغیرہ) permitidas ہیں اور operações blindadas rejeitadas ہوتے ہیں۔
  - `ShieldedOnly`: emissão de dinheiro e transferências کو instruções confidenciais استعمال کرنا ہوں گے؛ `RevealConfidential` ممنوع ہے تاکہ saldos عوامی نہ ہوں۔
  - `Convertible`: suportes نیچے بیان کردہ instruções de rampa de ativação/desativação کے ذریعے transparente اور representações blindadas کے درمیان movimento de valor کر سکتے ہیں۔
- Políticas restritas do FSM seguem کرتی ہیں تاکہ fundos encalhados نہ ہوں:
  - `TransparentOnly → Convertible` (pool blindado habilitado)
  - `TransparentOnly → ShieldedOnly` (transição pendente e janela de conversão aberta)
  - `Convertible → ShieldedOnly` (atraso mínimo)
  - `ShieldedOnly → Convertible` (plano de migração درکار تاکہ notas protegidas gastáveis رہیں)
  - `ShieldedOnly → TransparentOnly` não permitido ہے جب تک pool protegido vazio نہ ہو یا notas pendentes de governança کو unshield کرنے والی codificação de migração نہ کرے۔
- Instruções de governança `ScheduleConfidentialPolicyTransition` ISI کے ذریعے `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` set کرتی ہیں اور `CancelConfidentialPolicyTransition` سے alterações programadas abortar کر سکتی ہیں۔ Validação Mempool یقینی بناتی ہے کہ کوئی altura de transição de transação کو straddle نہ کرے اور alteração de verificação de política de meio bloco ہونے پر inclusão determinística طور پر falha ہو۔
- Transições pendentes نئے bloco کے شروع میں خودکار aplicar ہوتے ہیں: جب janela de conversão de altura do bloco میں داخل ہو (atualizações ShieldedOnly کے لئے) یا `effective_height` پہنچے تو runtime `AssetConfidentialPolicy` atualização کرتا ہے, `zk.policy` atualização de metadados کرتا ہے اور entrada pendente limpa کرتا ہے۔ اگر `ShieldedOnly` transição madura ہونے پر fornecimento transparente باقی ہو تو mudança de tempo de execução abortar کر کے log de aviso کرتا ہے اور modo anterior برقرار رہتا ہے۔
- Botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` aviso mínimo اور períodos de carência impor کرتے ہیں تاکہ troca de carteiras کے آس پاس conversão de notas کر سکیں۔
- Identificador de auditoria `pending_transition.transition_id` کے طور پر بھی کام کرتا ہے؛ governança کو transições finalizar یا cancelar کرتے وقت اسے cotação کرنا چاہئے تاکہ relatórios de rampa de ativação/desativação de operadores correlacionam کر سکیں۔
- `policy_transition_window_blocks` padrão 720 ہے (60s block time پر تقریباً 12 گھنٹے)۔ Solicitações de governança de nós com aviso mais curto چاہیں انہیں clamp کرتے ہیں۔
- Manifestos Genesis e fluxos CLI atuais e políticas pendentes expõem کرتے ہیں۔ Tempo de execução da lógica de admissão پر política پڑھ کر confirmar کرتی ہے کہ ہر instrução confidencial autorizada ہے۔
- Lista de verificação de migração — نیچے “Sequenciamento de migração” میں Milestone M0 کے مطابق plano de atualização em etapas دیکھیں۔

#### Torii کے ذریعے transições کی monitoramento

Carteiras اور auditores `GET /v1/confidential/assets/{definition_id}/transitions` کو poll کر کے ativo `AssetConfidentialPolicy` دیکھتے ہیں۔ Carga útil JSON ہمیشہ id de ativo canônico, altura de bloco observada mais recente, `current_mode`, altura e modo efetivo (janelas de conversão عارضی طور پر `Convertible` relatório کرتے ہیں), اور esperado Identificadores `vk_set_hash`/Poseidon/Pedersen شامل کرتا ہے۔ جب transição de governança pendente ہو تو resposta میں یہ بھی ہوتا ہے:

- `transition_id` - `ScheduleConfidentialPolicyTransition` سے واپس ملنے والا audit handle۔
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` اور derivado `window_open_height` (وہ bloquear جہاں carteiras کو ShieldedOnly cut-over کیلئے conversão شروع کرنا ہو)۔

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

Resposta `404` کا مطلب ہے کہ definição de ativo correspondente موجود نہیں۔ جب کوئی transição programada نہ ہو تو `pending_transition` campo `null` ہوتا ہے۔

### Máquina de estado de política| Modo atual | Próximo modo | Pré-requisitos | Manuseio em altura efetiva | Notas |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | Governança نے entradas de registro de verificador/parâmetro ativadas کئے ہوں۔ `ScheduleConfidentialPolicyTransition` enviar کریں جس میں `effective_height ≥ current_height + policy_transition_delay_blocks` ہو۔ | Transição بالکل `effective_height` para executar ہوتا ہے؛ piscina blindada فوراً دستیاب ہوتا ہے۔                   | شفاف fluxos برقرار رکھتے ہوئے confidencialidade ativar کرنے کا caminho padrão۔               |
| Somente Transparente | Somente blindado | اوپر والی شرائط کے ساتھ `policy_transition_window_blocks ≥ 1` بھی۔                                                         | Runtime `effective_height - policy_transition_window_blocks` é `Convertible` میں داخل ہوتا ہے؛ `effective_height` é um `ShieldedOnly` de alta qualidade | شفاف instruções desabilitam ہونے سے پہلے janela de conversão determinística دیتا ہے۔   |
| Conversível | Somente blindado | `effective_height ≥ current_height + policy_transition_delay_blocks` کے ساتھ transição agendada۔ A governança DEVE auditar metadados کے ذریعے (`transparent_supply == 0`) certificar کرے؛ transição de tempo de execução para aplicar کرتا ہے۔ | اوپر جیسی semântica de janela۔ اگر `effective_height` پر fornecimento transparente diferente de zero ہو تو transição `PolicyTransitionPrerequisiteFailed` کے ساتھ abortar ہو جاتا ہے۔ | ativo کو مکمل circulação confidencial میں bloqueio کرتا ہے۔                                     |
| Somente blindado | Conversível | Transição agendada؛ کوئی saque de emergência ativo نہیں (`withdraw_height` não definido)۔                                    | `effective_height` پر state flip ہوتا ہے؛ revelar rampas دوبارہ کھلتی ہیں جبکہ notas protegidas válidas رہتے ہیں۔                           | janelas de manutenção یا avaliações de auditores کے لئے۔                                          |
| Somente blindado | Somente Transparente | Governança کو `shielded_supply == 0` ثابت کرنا ہوگا یا assinado `EmergencyUnshield` estágio do plano کرنا ہوگا (assinaturas do auditor درکار)۔ | Tempo de execução `effective_height` سے پہلے Janela `Convertible` کھولتا ہے؛ اس altura پر instruções confidenciais hard-fail ہو جاتی ہیں اور modo somente transparente de ativo میں واپس جاتا ہے۔ | saída de último recurso۔ اگر janela کے دوران کوئی gasto de nota confidencial ہو تو cancelamento automático de transição ہو جاتا ہے۔ |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` alteração pendente clara کرتا ہے۔                                                        | `pending_transition` Remover ہوتا ہے۔                                                                          | status quo completude کیلئے شامل۔                                             |

اوپر lista نہ ہونے والے submissão de governança de transições پر rejeitar ہوتے ہیں۔ Transição agendada em tempo de execução aplicada کرنے سے پہلے verificação de pré-requisitos کرتا ہے؛ ativo de falha کو modo anterior میں واپس دھکیلتی ہے اور eventos de telemetria/bloqueio کے ذریعے `PolicyTransitionPrerequisiteFailed` emitir کرتی ہے۔

### Sequenciamento de migração

2. **Preparar a transição:** `policy_transition_delay_blocks` کو respeito کرنے والے `effective_height` کے ساتھ `ScheduleConfidentialPolicyTransition` enviar کریں۔ `ShieldedOnly` کی طرف جاتے وقت janela de conversão especifique کریں (`window ≥ policy_transition_window_blocks`)۔
3. **Publicar orientação do operador:** واپس ملنے والا `transition_id` registro کریں اور on/off-rampa runbook circular کریں۔ Carteiras اور auditores `/v1/confidential/assets/{id}/transitions` subscrever کر کے altura de abertura da janela جانتے ہیں۔
4. **Imposição de janela:** janela کھلتے ہی política de tempo de execução کو `Convertible` پر switch کرتا ہے, `PolicyTransitionWindowOpened { transition_id }` emit کرتا ہے, اور solicitações de governança conflitantes rejeitadas کرنا شروع کرتا ہے۔
5. **Finalizar ou abortar:** `effective_height` پر verificação de pré-requisitos de transição de tempo de execução کرتا ہے (fornecimento transparente صفر، retirada de emergência نہیں وغیرہ)۔ Política de sucesso کو modo solicitado پر flip کرتا ہے؛ falha `PolicyTransitionPrerequisiteFailed` emissão کر کے transição pendente clara کرتا ہے اور política inalterada رہتی ہے۔
6. **Atualizações de esquema:** transição de transição کے بعد aumento de versão do esquema de ativos de governança کرتی ہے (مثلاً `asset_definition.v2`) e serialização de manifesto de ferramentas CLI کرتے وقت `confidential_policy` exigir کرتا ہے۔ Operadores de documentos de atualização Genesis کو reinicialização dos validadores سے پہلے configurações de política اور impressões digitais do registro adicionar کرنے کی ہدایت دیتے ہیں۔Confidencialidade habilitada کے ساتھ شروع ہونے والی نئی política desejada de redes کو genesis میں codificar کرتی ہیں۔ پھر بھی iniciar کے بعد mudança de modo کرتے وقت اوپر والی lista de verificação seguir کی جاتی ہے تاکہ janelas de conversão determinísticas رہیں اور carteiras کے پاس ajustar کرنے کا وقت ہو۔

### Controle de versão e ativação do manifesto Norito

- Genesis manifesta DEVE a chave personalizada `confidential_registry_root` کیلئے `SetParameter` incluir کریں۔ Payload Norito JSON ہے جو `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` سے match کرتا ہے: جب کوئی entrada do verificador فعال نہ ہو تو campo omitir کریں (`null`) ، ورنہ string hexadecimal de 32 bytes (`0x…`) دیں جو manifesto میں موجود instruções do verificador پر `compute_vk_set_hash` کے hash کے برابر ہو۔ Parâmetro ausente یا incompatibilidade de hash ہونے پر nós iniciam سے انکار کرتے ہیں۔
- Versão de layout de manifesto `ConfidentialFeatureDigest::conf_rules_version` on-wire incorporada کرتا ہے۔ redes v1 کیلئے اسے `Some(1)` رہنا DEVE ہے اور یہ `iroha_config::parameters::defaults::confidential::RULES_VERSION` کے برابر ہے۔ Evolução do conjunto de regras ہو تو colisão constante کریں, manifestos regenerados کریں، اور binários کو lock-step میں implementação کریں؛ versões mix کرنے سے validadores `ConfidentialFeatureDigestMismatch` کے ساتھ blocos rejeitados کرتے ہیں۔
- Manifestos de ativação e atualizações de registro, alterações no ciclo de vida dos parâmetros, e pacote de transições de política کرنا DEVE ہے تاکہ digerir رہے consistente:
  1. Mutações de registro planejadas (`Publish*`, `Set*Lifecycle`) کو visualização de estado offline میں aplicar کریں اور `compute_confidential_feature_digest` سے cálculo de resumo pós-ativação کریں۔
  2. Hash computado کے ساتھ `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` emitir کریں تاکہ pares atrasados ​​instruções intermediárias de registro perder کرنے کے باوجود digestão correta recuperar کر سکیں۔
  3. Instruções `ScheduleConfidentialPolicyTransition` anexadas کریں۔ ہر instrução کو cotação `transition_id` emitida pela governança کرنا لازم ہے؛ اسے بھولنے والے manifesta rejeição de tempo de execução کرتا ہے۔
  4. Bytes de manifesto, impressão digital SHA-256 e plano de ativação. Operadores تینوں artefatos verificam کر کے ہی votar دیتے ہیں تاکہ partições سے بچا جا سکے۔
- جب implementação کیلئے corte adiado درکار ہو، altura do alvo کو parâmetro personalizado complementar میں registro کریں (مثلاً `custom.confidential_upgrade_activation_height`)۔ یہ auditores کو prova codificada Norito دیتا ہے کہ validadores نے digestão da mudança سے پہلے honra da janela de aviso کیا۔

## Verificador e ciclo de vida dos parâmetros
### Registro ZK
- Ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` اسٹور کرتا ہے جہاں `proving_system` فی الحال `Halo2` پر fixo ہے۔
- `(circuit_id, version)` pares globalmente únicos ہیں؛ metadados do circuito de registro کے pesquisa کیلئے índice secundário رکھتی ہے۔ Par duplicado رجسٹر کرنے کی کوشش admissão پر rejeitar ہوتی ہے۔
- `circuit_id` não vazio ہونا چاہئے اور `public_inputs_schema_hash` فراہم کرنا لازم ہے (عام طور پر verificador کے codificação canônica de entrada pública کا Blake2b-32 hash)۔ Admissão ان فیلڈز کے بغیر registros rejeitados کرتا ہے۔
- Instruções de governança میں شامل ہیں:
  - `PUBLISH`, entrada `Proposed` somente metadados adicionar کرنے کیلئے۔
  - `ACTIVATE { vk_id, activation_height }`, limite de época e cronograma de ativação کرنے کیلئے۔
  - `DEPRECATE { vk_id, deprecation_height }`, آخری marca de altura کرنے کیلئے جہاں entrada de provas کو referência کر سکیں۔
  - `WITHDRAW { vk_id, withdraw_height }`, desligamento de emergência کیلئے؛ متاثرہ altura de retirada de ativos کے بعد congelamento de gastos confidenciais کرتے ہیں جب تک نئی entradas ativadas نہ ہوں۔
- Manifestos Genesis خودکار طور پر `confidential_registry_root` parâmetro personalizado emitir کرتے ہیں جس کا `vk_set_hash` entradas ativas سے corresponder کرتا ہے؛ nó de validação کے junção de consenso سے پہلے resumo کو estado de registro local کے خلاف verificação cruzada کرتی ہے۔
- Registro do verificador یا atualização کیلئے `gas_schedule_id` ضروری ہے؛ verificação impor کرتی ہے کہ entrada de registro `Active` ہو, índice `(circuit_id, version)` میں ہو، اور provas Halo2 میں `OpenVerifyEnvelope` ہو جس کا `circuit_id`, `vk_hash`, اور `public_inputs_schema_hash` registro de registro سے correspondência کرے۔

### Provando Chaves
- Provando chaves fora do razão رہتے ہیں مگر identificadores endereçados ao conteúdo (`pk_cid`, `pk_hash`, `pk_len`) کے ذریعے consulte کیے جاتے ہیں Os metadados do verificador são publicados e publicados
- Busca de dados PK dos SDKs da carteira کر کے verificação de hashes کرتے ہیں اور cache local میں رکھتے ہیں۔

### Parâmetros de Pedersen e Poseidon
- Registros de الگ (`PedersenParams`, `PoseidonParams`) espelho de controles de ciclo de vida do verificador کرتی ہیں, ہر ایک میں `params_id`, geradores/constantes کے hashes, alturas de ativação/descontinuação/retirada ہوتے ہیں۔## Ordenação Determinística e Anuladores
- ہر ativo `CommitmentTree` رکھتا ہے جس میں `next_leaf_index` ہوتا ہے؛ bloqueia compromissos کو ordem determinística میں anexar کرتے ہیں: ordem de bloqueio میں transações iteram کریں؛ ہر transação کے اندر serializada `output_idx` ascendente میں saídas blindadas iteram کریں۔
- `note_position` deslocamentos de árvore سے derivar ہوتا ہے مگر nullifier کا حصہ **نہیں**؛ یہ صرف testemunha de prova میں caminhos de adesão کے لئے استعمال ہوتا ہے۔
- Reorgs کے تحت estabilidade do anulador design PRF سے garantia ہوتی ہے؛ Entrada PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` vincular کرتا ہے, اور âncoras تاریخی raízes Merkle کو referência کرتے ہیں جو `max_anchor_age_blocks` سے محدود ہیں۔

## Fluxo do razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Política de ativos `Convertible` e `ShieldedOnly` verificação de autoridade de ativos de admissão کرتا ہے، atual `params_id` recuperar کرتا ہے, `rho` amostra کرتا ہے, compromisso emitir کرتا ہے, atualização da árvore Merkle کرتا ہے۔
   - `ConfidentialEvent::Shielded` emite کرتا ہے جس میں novo compromisso, Merkle root delta اور trilha de auditoria کیلئے hash de chamada de transação ہوتا ہے۔
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - Entrada de registro syscall VM سے verificação de prova کرتا ہے؛ host یقینی بناتا ہے کہ anuladores não utilizados ہوں، compromissos determinísticos طور پر anexar ہوں, اور âncora recente ہو۔
   - Registro de entradas do Ledger `NullifierSet` کرتا ہے, destinatários/auditores کیلئے armazenamento de cargas úteis criptografadas کرتا ہے, e `ConfidentialEvent::Transferred` emite کرتا ہے جو anuladores, saídas ordenadas, hash de prova, As raízes de اور Merkle resumem کرتا ہے۔
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - صرف `Convertible` ativos کیلئے؛ prova validar کرتا ہے کہ valor da nota valor revelado کے برابر ہے، razão saldo transparente crédito کرتا ہے، اور anulador marca gasta کر کے queima de nota protegida کرتا ہے۔
   - `ConfidentialEvent::Unshielded` emite کرتا ہے جس میں valor público, anuladores consumidos, identificadores de prova e hash de chamada de transação شامل ہیں۔

## Adições ao modelo de dados
- `ConfidentialConfig` (nova seção de configuração) com sinalizador de ativação, `assume_valid`, botões de gás/limite, janela de âncora, backend do verificador.
- `ConfidentialNote`, `ConfidentialTransfer`, اور `ConfidentialMint` Norito esquemas byte de versão explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`) کے ساتھ۔
- `ConfidentialEncryptedPayload` AEAD memo bytes کو `{ version, ephemeral_pubkey, nonce, ciphertext }` میں wrap کرتا ہے, padrão `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` XChaCha20-Poly1305 layout کیلئے۔
- Vetores de derivação de chave canônica `docs/source/confidential_key_vectors.json` میں ہیں؛ CLI para Torii endpoint e fixtures para regress کرتے ہیں۔
- `asset::AssetDefinition` کو `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ملتا ہے۔
- Verificadores de transferência/desproteção `ZkAssetState` کیلئے A ligação `(backend, name, commitment)` persiste کرتا ہے؛ execução ان provas کو rejeitar کرتا ہے جن کا referenciado یا verificação em linha do compromisso registrado da chave سے correspondência نہ کرے۔
- `CommitmentTree` (por ativo com pontos de verificação de fronteira), `NullifierSet` digitado por `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` estado mundial میں loja ہوتے ہیں۔
- Detecção antecipada de duplicatas do Mempool e verificações de idade da âncora کیلئے transiente `NullifierIndex` e estruturas `AnchorIndex` mantêm کرتا ہے۔
- Atualizações de esquema Norito میں entradas públicas کی ordenação canônica شامل ہے؛ testes de ida e volta que codificam o determinismo garantem کرتے ہیں۔
- Testes de unidade de ida e volta de carga útil criptografada (`crates/iroha_data_model/src/confidential.rs`) کے ذریعے lock ہیں۔ Auditores de vetores de carteira de acompanhamento کیلئے transcrições canônicas AEAD anexadas کریں گے۔ Envelope `norito.md` کیلئے documento de cabeçalho on-wire کرتا ہے۔

## IVM Integração e Syscall
- `VERIFY_CONFIDENTIAL_PROOF` syscall introduz کریں جو قبول کرتا ہے:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` e o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - Registro Syscall سے carga de metadados do verificador کرتا ہے، limites de tamanho/tempo impor کرتا ہے, carga determinística de gás کرتا ہے, اور prova de sucesso پر ہی delta aplicar کرتا ہے۔
- Host somente leitura `ConfidentialLedger` exposição de característica کرتا ہے جو Merkle root snapshots اور nullifier status retrieve کرتا ہے؛ Auxiliares de montagem de testemunha da biblioteca Kotodama e validação de esquema فراہم کرتی ہے۔
- Layout do buffer de prova de documentos Pointer-ABI e identificadores de registro واضح کرنے کیلئے atualização ہوئے ہیں۔## Negociação de capacidade do nó
- Aperto de mão `feature_bits.confidential` کے ساتھ `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` anunciar کرتا ہے۔ Participação do validador کیلئے `confidential.enabled=true`, `assume_valid=false`, identificadores de back-end do verificador idênticos اور resumos correspondentes ضروری ہیں؛ incompatibilidades `HandshakeConfidentialMismatch` کے ساتھ falha de handshake کرتے ہیں۔
- Config صرف nós observadores کیلئے `assume_valid` suporte کرتا ہے: desativado ہونے پر instruções confidenciais پر determinístico `UnsupportedInstruction` آتا ہے بغیر pânico؛ ativado ہونے پر observadores provas verificam کئے بغیر deltas de estado aplicam-se کرتے ہیں۔
- Transações confidenciais do Mempool rejeitadas کرتا ہے اگر capacidade local desativada ہو۔ Fofoca filtra pares sem recursos de correspondência کو transações protegidas بھیجنے سے گریز کرتے ہیں جبکہ IDs de verificador desconhecidos کو limites de tamanho کے اندر blind-forward کرتے ہیں۔

### Revelar política de retenção de poda e anulador

Livros confidenciais کو nota atualizada ثابت کرنے اور auditorias orientadas pela governança repetição کرنے کیلئے کافی histórico reter کرنی ہوتی ہے۔ `ConfidentialLedger` کی política padrão یہ ہے:

- **Retenção do nulificador:** nulificadores gastos کم از کم `730` دن (24 مہینے) altura de gasto کے بعد رکھیں, یا اگر regulador زیادہ مدت مانگے تو وہ۔ Operadores `confidential.retention.nullifier_days` کے ذریعے extensão de janela کر سکتے ہیں۔ janela de retenção کے اندر anuladores Torii کے ذریعے consultável رہیں تاکہ auditores gastos duplos ausência ثابت کر سکیں۔
- **Revelar poda:** revelações transparentes (`RevealConfidential`) متعلقہ anotar compromissos کو finalização de bloco کے فوراً بعد prune کرتے ہیں, مگر anulador consumido اوپر والی regra de retenção کے تابع رہتا ہے۔ Eventos relacionados à revelação (`ConfidentialEvent::Unshielded`) valor público, destinatário e registro de hash de prova کرتے ہیں تاکہ revelações históricas reconstruídas کرنے کیلئے texto cifrado podado کی ضرورت نہ ہو۔
- **Pontos de verificação de fronteira:** fronteiras de compromisso pontos de verificação rolantes رکھتے ہیں جو `max_anchor_age_blocks` اور janela de retenção میں سے بڑے کو cobertura کرتے ہیں۔ Nós پرانے pontos de verificação صرف تب compacto کرتے ہیں جب intervalo کے تمام nulificadores expiram ہو جائیں۔
- **Correção de resumo obsoleto:** اگر desvio de resumo کی وجہ سے `HandshakeConfidentialMismatch` آئے تو operadores کو (1) cluster میں janelas de retenção do nulificador alinhar ہونے کی تصدیق کرنی چاہئے، (2) `iroha_cli app confidential verify-ledger` چلا کر conjunto anulador retido کے خلاف digest regenerate کرنا چاہئے, اور (3) refrescado manifesto reimplantar کرنا چاہئے۔ Anulificadores eliminados prematuramente کو junção de rede سے پہلے armazenamento frio سے restauração کرنا ہوگا۔

Substituições locais کو runbook de operações میں documento کریں؛ janela de retenção بڑھانے والی políticas de governança کو configuração de nó اور planos de armazenamento de arquivamento کے ساتھ lockstep میں atualização کرنا ضروری ہے۔

### Fluxo de despejo e recuperação

1. Disque کے دوران `IrohaNetwork` capacidades anunciadas compare کرتا ہے۔ incompatibilidade `HandshakeConfidentialMismatch` aumentar کرتا ہے؛ conexão بند ہوتی ہے اور fila de descoberta de pares میں رہتا ہے بغیر `Ready` بنے۔
2. Log de serviço de rede de falha میں superfície ہوتی ہے (digest remoto اور backend سمیت), اور Sumeragi peer کو proposta یا votação کیلئے agendamento نہیں کرتا۔
3. Registros de verificadores de operadores e conjuntos de parâmetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) alinhar کر کے یا acordado `activation_height` کے ساتھ Estágio `next_conf_features` کر کے remediação کرتے ہیں۔ Digest match ہوتے ہی اگلا aperto de mão خودکار طور پر sucesso کرتا ہے۔
4. اگر peer obsoleto کوئی block broadcast کر دے (مثلاً replay de arquivamento کے ذریعے), validadores اسے `BlockRejectionReason::ConfidentialFeatureDigestMismatch` کے ساتھ determinístico طور پر rejeitar کرتے ہیں, جس سے rede میں estado do razão consistente رہتا ہے۔

### Fluxo de handshake seguro para repetição1. ہر tentativa de saída novo material chave de ruído/X25519 alocar کرتا ہے۔ Carga útil de handshake assinada (`handshake_signature_payload`) local e chaves públicas efêmeras remotas, endereço de soquete anunciado codificado em Norito, e `handshake_chain_id` کے ساتھ compilar ہونے پر identificador de cadeia concatenar کرتا ہے۔ Nó de mensagem سے نکلنے سے پہلے criptografado por AEAD ہوتی ہے۔
2. Ordem de chave local/ponto do respondedor کو reverso کر کے recálculo de carga útil کرتا ہے اور `HandshakeHelloV1` میں verificação de assinatura Ed25519 incorporada کرتا ہے۔ چونکہ دونوں chaves efêmeras اور domínio de assinatura de endereço anunciado میں ہیں, mensagem capturada کو کسی دوسرے peer کے خلاف replay کرنا یا conexão obsoleta recuperar کرنا determinístico طور پر falhar ہوتا ہے۔
3. Sinalizadores de capacidade confidencial اور `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` کے اندر travel کرتے ہیں۔ Tupla do receptor `{ enabled, assume_valid, verifier_backend, digest }` کو local `ConfidentialHandshakeCaps` کے ساتھ comparar کرتا ہے؛ incompatibilidade ہونے پر `HandshakeConfidentialMismatch` کے ساتھ saída antecipada ہوتا ہے۔
4. Operadores کو digerir ( `compute_confidential_feature_digest` کے ذریعے) recalcular کرنا اور registros/políticas atualizados کے ساتھ reinicialização de nós کرنا DEVE ہے۔ Resumos antigos anunciam کرنے والے falha de handshake de pares کرتے رہیں گے اور estado obsoleto کو conjunto de validador میں واپس آنے سے روکیں گے۔
5. Sucessos/falhas de handshake contadores `iroha_p2p::peer` padrão (`handshake_failure_count` وغیرہ) atualização کرتے ہیں اور entradas de log estruturadas emitem کرتے ہیں جن میں ID de peer remoto اور digerir tags de impressão digital ہوتے ہیں۔ ان indicadores کو monitor کریں تاکہ rollout کے دوران tentativas de repetição یا configurações incorretas پکڑی جا سکیں۔

## Gerenciamento de chaves e cargas úteis
- Hierarquia de derivação de chave por conta:
  - `sk_spend` → `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- Cargas úteis de notas criptografadas AEAD استعمال کرتے ہیں جو chaves compartilhadas derivadas de ECDH سے بنے ہوتے ہیں؛ auditor opcional visualizar chaves política de ativos کے مطابق saídas کے ساتھ anexar کئے جا سکتے ہیں۔
- Adições CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, descriptografia de memorandos کرنے کیلئے ferramentas de auditor, e auxiliar `iroha app zk envelope` e envelopes de memorando Norito offline produzir/inspecionar کرتا ہے۔ Torii `POST /v1/confidential/derive-keyset` کے ذریعے وہی fluxo de derivação فراہم کرتا ہے اور hex/base64 دونوں formulários واپس دیتا ہے تاکہ carteiras, hierarquias de chaves programaticamente buscam کر سکیں۔

## Controles de gás, limites e DoS
- Cronograma determinístico de gás:
  - Halo2 (Plonkish): gás base `250_000` + gás `2_000` por entrada pública.
  - Gás `5` por byte de prova, mais cobranças por anulador (`300`) e por compromisso (`500`)۔
  - Operadores یہ configuração de nó de constantes (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) کے ذریعے substituir کر سکتے ہیں؛ altera inicialização یا configuração hot-reload پر propagate ہو کر cluster میں determinístico طور پر aplicar ہوتے ہیں۔
- Limites rígidos (padrões configuráveis):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms` سے زیادہ instrução de provas کو determinística طور پر abortar کرتے ہیں (cédulas de governança `proof verification exceeded timeout` emitem کرتے ہیں, `VerifyProof` retorno de erro کرتا ہے)۔
- Cotas adicionais de vivacidade garantem کرتے ہیں: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, اور `max_public_inputs` construtores de blocos کو vinculados کرتے ہیں؛ `reorg_depth_bound` (≥ `max_anchor_age_blocks`) controle de retenção de ponto de verificação de fronteira کرتا ہے۔
- Tempo de execução اب por transação یا limites por bloco excedem کرنے والی transações rejeitadas کرتا ہے, erros determinísticos `InvalidParameter` emitem کرتا ہے اور estado do razão inalterado رہتی ہے۔
- Mempool `vk_id`, comprimento da prova, idade da âncora کے ذریعے transações confidenciais کو pré-filtro کرتا ہے, verificador invocar کرنے سے پہلے uso de recursos limitado رکھتا ہے۔
- Verificação determinística طور پر timeout یا violação vinculada پر parada ہوتی ہے؛ erros explícitos de transações کے ساتھ falhar ہوتی ہیں۔ Backends SIMD opcionais ہیں مگر alteração de contabilidade de gás نہیں کرتے۔

### Linhas de base de calibração e portas de aceitação
- **Plataformas de referência.** Execuções de calibração کو نیچے دیئے گئے تین cobertura de perfis de hardware کرنا DEVE ہے۔ Como capturar perfis e rejeitar revisão| Perfil | Arquitetura | CPU/Instância | Sinalizadores de compilador | Finalidade |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | intrínsecos vetoriais کے بغیر valores mínimos estabelecidos کرتا ہے؛ ajuste de tabelas de custos substitutos |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | versão padrão | Validação de caminho AVX2 کرتا ہے؛ چیک کرتا ہے کہ SIMD acelera a tolerância ao gás neutro کے اندر رہیں۔ |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versão padrão | Backend NEON کو determinístico اور x86 cronogramas کے ساتھ alinhado رکھتا ہے۔ |

- **Arnês de referência.** Todos os relatórios de calibração de gás DEVEM ser:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - Dispositivo determinístico `cargo test -p iroha_core bench_repro -- --ignored` confirma کرنے کیلئے۔
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` جب بھی Custos de opcode VM بدلیں۔

- **Aleatoriedade fixa.** bancos چلانے سے پہلے `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` exportação کریں تاکہ `iroha_test_samples::gen_account_in` determinístico `KeyPair::from_seed` caminho پر switch ہو۔ Arnês ایک بار `IROHA_CONF_GAS_SEED_ACTIVE=…` print کرتا ہے؛ variável ausente ہو تو revisão DEVE falhar ہو۔ نئی utilitários de calibração بھی introdução de aleatoriedade auxiliar کرتے وقت اس env var کو honor کریں۔

- **Captura de resultados.**
  - Resumos de critérios (`target/criterion/**/raw.csv`) ہر perfil کیلئے liberar artefato میں fazer upload کریں۔
  - Métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) کو [Ledger de calibração de gás confidencial](./confidential-gas-calibration) میں git commit ou versão do compilador کے ساتھ store کریں۔
  - ہر perfil کیلئے آخری دو linhas de base برقرار رکھیں؛ نئی report validar ہونے کے بعد پرانی snapshots delete کریں۔

- **Tolerâncias de aceitação.**
  - `baseline-simd-neutral` e `baseline-avx2` کے درمیان deltas de gás ≤ ±1,5% رہیں۔
  - `baseline-simd-neutral` e `baseline-neon` کے درمیان deltas de gás ≤ ±2,0% رہیں۔
  - ان limites سے تجاوز کرنے والی propostas de calibração کو ajustes de cronograma یا discrepância/mitigação وضاحت کرنے والا RFC درکار ہوگا۔

- **Lista de verificação de revisão.** Remetentes ذمہ دار ہیں:
  - `uname -a`, trechos `/proc/cpuinfo` (modelo, revisão), e registro de calibração `rustc -Vv` میں شامل کریں۔
  - saída de bancada میں `IROHA_CONF_GAS_SEED` کے echo ہونے کی تصدیق کریں (bancos active seed print کرتے ہیں)۔
  - marcapasso اور confidencial verificador feature flags produção سے match ہوں (`--features confidential,telemetry` جب Telemetria کے ساتھ bancos چلائیں)۔

## Configuração e operações
- `iroha_config` میں `[confidential]` سیکشن شامل:
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
- Métricas agregadas de telemetria emitem کرتا ہے: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, اور `confidential_policy_transitions_total`, exposição de dados em texto simples
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## Estratégia de teste
- Determinismo: blocos کے اندر transação aleatória embaralhando raízes Merkle idênticas اور conjuntos anuladores de rendimento کرتا ہے۔
- Resiliência de reorganização: âncoras کے ساتھ reorganizações multibloco simulam کریں؛ anuladores estáveis ​​رہتے ہیں اور âncoras obsoletas rejeitadas ہوتے ہیں۔
- Invariantes de gás: aceleração SIMD کے ساتھ اور بغیر nós پر verificação de uso de gás idêntico کریں۔
- Teste de limite: limites de tamanho/gás پر provas, contagens máximas de entrada/saída, aplicação de tempo limite۔
- Ciclo de vida: verificador e ativação/descontinuação de parâmetros کیلئے operações de governança, testes de gastos de rotação۔
- Política FSM: transições permitidas/não permitidas, atrasos de transição pendentes, alturas efetivas کے گرد rejeição de mempool۔
- Emergências de registro: retirada emergencial `withdraw_height` پر congelamento de ativos afetados کرتا ہے اور اس کے بعد provas rejeitadas کرتا ہے۔
- Controle de capacidade: `conf_features` incompatível e blocos de validadores rejeitados کرتے ہیں؛ `assume_valid=true` e consenso dos observadores کو متاثر کئے بغیر sincronização رہتے ہیں۔
- Equivalência de estado: cadeia canônica de nós validadores/completos/observadores پر raízes de estado idênticas produzem کرتے ہیں۔
- Fuzzing negativo: provas malformadas, cargas úteis superdimensionadas, colisões anuladoras determinísticas ou rejeitadas ہوتے ہیں۔## Trabalho Excelente
- Conjuntos de parâmetros Halo2 (tamanho do circuito, estratégia de pesquisa) benchmark کریں اور نتائج manual de calibração میں ریکارڈ کریں تاکہ اگلی `confidential_assets_calibration.md` atualização کے ساتھ atualização de padrões de gás/tempo limite ہوں۔
- Políticas de divulgação de auditores اور متعلقہ APIs de visualização seletiva finalizadas کریں، اور aprovação do rascunho de governança کے بعد fluxo de trabalho aprovado کو Torii میں wire کریں۔
- Esquema de criptografia de testemunha کو saídas de vários destinatários اور memorandos em lote تک estender کریں, implementadores de SDK کیلئے documento em formato de envelope کریں۔
- Circuitos, registros, procedimentos de rotação de parâmetros, comissão de revisão de segurança externa, resultados, relatórios de auditoria interna, arquivo,
- APIs de reconciliação de gastos do auditor especificam کریں اور orientação de escopo da chave de visualização publicar کریں تاکہ fornecedores de carteira ایک جیسے semântica de atestado implementar کر سکیں۔

## Fases de implementação
1. **Fase M0 — Endurecimento Stop-Ship**
   - ✅ Derivação do nulificador اب Poseidon PRF design (`nk`, `rho`, `asset_id`, `chain_id`) siga کرتا ہے اور atualizações de razão میں ordem de compromisso determinístico impor ہوتی ہے۔
   - ✅ Limites de tamanho de prova de execução اور cotas confidenciais por transação/por bloco impor کرتا ہے، transações acima do orçamento کو erros determinísticos کے ساتھ rejeitar کرتا ہے۔
   - ✅ Handshake P2P `ConfidentialFeatureDigest` (backend digest + impressões digitais do registro) anunciar کرتا ہے اور incompatibilidades کو `HandshakeConfidentialMismatch` کے ذریعے falha determinística کرتا ہے۔
   - ✅ Caminhos de execução confidenciais میں panics remove کئے گئے اور nós não suportados کیلئے role gating add کیا گیا۔
   - ⚪ Orçamentos de tempo limite do verificador e pontos de verificação de fronteira کیلئے reorganizar limites de profundidade impor کرنا۔
     - ✅ Orçamentos de tempo limite de verificação aplicados ہوئے؛ `verify_timeout_ms` سے تجاوز کرنے والی provas e falha determinística ہوتی ہیں۔
     - ✅ Pontos de verificação de fronteira اب `reorg_depth_bound` respeito کرتے ہیں، janela configurada سے پرانے pontos de verificação podar کرتے ہوئے instantâneos determinísticos برقرار رکھتے ہیں۔
   - `AssetConfidentialPolicy`, política FSM، اور instruções de cunhagem/transferência/revelação کیلئے portões de aplicação introduzem کریں۔
   - Cabeçalhos de bloco میں `conf_features` commit کریں اور resumos de registro/parâmetro divergem ہونے پر participação do validador recusar کریں۔
2. **Fase M1 — Registros e Parâmetros**
   - `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` operações de governança de registros, ancoragem de gênese e gerenciamento de cache کے ساتھ terreno کریں۔
   - Syscall کو pesquisas de registro, IDs de programação de gás, hashing de esquema, e verificações de tamanho exigem کرنے کیلئے wire کریں۔
   - Formato de carga útil criptografada v1, vetores de derivação de chave de carteira, e gerenciamento de chave confidencial کیلئے navio de suporte CLI کریں۔
3. **Fase M2 — Gás e Desempenho**
   - Cronograma de gás determinístico, contadores por bloco, telemetria e chicotes de benchmark implementam کریں (verificar latência, tamanhos de prova, rejeições de mempool)۔
   - Pontos de verificação CommitmentTree, carregamento LRU, índices anuladores e cargas de trabalho de vários ativos کیلئے endurecer کریں۔
4. **Fase M3 – Rotação e Ferramentas de Carteira**
   - Habilitação de aceitação de prova multiparâmetro e multiversão کریں؛ ativação/descontinuação orientada por governança کو runbooks de transição کے ساتھ suporte کریں۔
   - Fluxos de migração Wallet SDK/CLI, fluxos de trabalho de verificação de auditores, e ferramentas de reconciliação de gastos entregam کریں۔
5. **Fase M4 — Auditoria e Operações**
   - Auditoria de principais fluxos de trabalho, APIs de divulgação seletiva e runbooks operacionais.
   - Cronograma de revisão externa de criptografia/segurança کریں اور descobertas `status.md` میں publicar کریں۔

ہر marcos do roteiro de fase اور متعلقہ testes de atualização کرتی ہے تاکہ rede blockchain کیلئے garantias de execução determinísticas برقرار رہیں۔