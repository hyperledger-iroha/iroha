---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Atividades confidenciais e ZK-переводы
descrição: Блюпринт Fase C para circuitos blindados, controles e controles operacionais.
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Дизайн конфиденциальных активов и ZK-perevодов

## Motivação
- Essas ações protegidas opt-in são ativadas, seus domínios podem solicitar a transação privada, sem nenhuma transação circulação.
- Сохранять детерминированное исполнение на разнородном железе валидаторов и совместимость с Norito/Kotodama ABI v1.
- Utilize um ciclo de controle de áudio e operação (ativação, rotação, abertura) para circuitos e parâmetros de criptografia.

## Modelo угроз
- Валидаторы честные-но-любопытные (honesto-mas-curioso): исполняют консенсус корректно, но пытаются изучать razão/estado.
- Наблюдатели сети видят данные блоков и fofocas транзакции; приватные fofocas-каналы não предполагаются.
- Seus projetos: análise de tráfego off-ledger, proteção de contas (definido no roteiro PQ), e registro de entrega.

## Обзор дизайна
- Активы могут объявлять *piscina blindada* помимо существующих прозрачных балансов; циркуляция представляется compromissos de criptografia blindados.
- Notas inseridas `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, sem notas adicionais.
  - Carga criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Транзакции несут Norito-кодированные payloads `ConfidentialTransfer`, содержащие:
  - Entradas públicas: âncora Merkle, anuladores, compromissos novos, id de ativo, circuito de versão.
  - Cargas úteis para получателей e опциональных аудиторов.
  - Prova de conhecimento zero, garantia de segurança, propriedade e autorização.
- Verificação de chaves e parâmetros de controle do registro no razão com confirmação de ativação; узлы отказываются валидировать provas, которые ссылаются на неизвестные ou отозванные записи.
- Заголовки консенсуса коммитятся к активному digest конфиденциальных возможностей, поэтому блоки принимаются только при совпадении состояния registro e parâmetros.
- Построение provas использует стек Halo2 (Plonkish) sem configuração confiável; Groth16 e outras variantes do SNARK não foram incluídas na v1.

### Determinar as figuras

Конфиденциальные memo-конверты теперь поставляются с каноническим фикстуром в `fixtures/confidential/encrypted_payload_v1.json`. Você pode usar o software v1-converter e instalar o SDK sem sucesso, usando o SDK instalado проверять паритет парсинга. Testes de modelo de dados de ferrugem (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e suíte Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) fornecem uma configuração completa, garantida, isso Norito-кодирование, поверхности ошибок e регрессионное покрытие остаются согласованными por mais эволюции кодека.Os SDKs do Swift podem ser usados para usar o escudo-инструкции com cola JSON sob medida: соберите `ShieldRequest` com 32-байтным compromisso não, carga útil e metadados de débito, verifique `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`), verifique e execute a transação `/v1/pipeline/transactions`. Para ajudar a validar compromissos de длины, пропускает `ConfidentialEncryptedPayload` через codificador Norito e layout de layout `zk::Shield`, описанный Não, seus cabos estão sincronizados com Rust.

## Коммитменты консенсуса e gating возможностей
- Conjunto de blocos `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; digest участвует в хэше консенсуса и должен совпадать с локальным представлением registro para принятия блока.
- Governança pode ser implementada, programa `next_conf_features` com `activation_height`; Faça isso para que você possa criar um bloco de compilação para publicar o resumo.
- Валидаторские узлы ДОЛЖНЫ funciona com `confidential.enabled = true` e `assume_valid = false`. O teste inicial não é usado na validação, exceto no local `conf_features` расходятся.
- O handshake P2P é definido como `{ enabled, assume_valid, conf_features }`. Peers, рекламирующие несовместимые возможности, отклоняются с `HandshakeConfidentialMismatch` e никогда не входят в ротацию consciência.
- Результаты совместимости между валидаторами, observadores e устаревшими peers фиксируются в матрице handshake em разделе [Node Capability Negociação](#node-capability-negotiation). Ошибки handshake проявляются как `HandshakeConfidentialMismatch` e держат peer вне ротации консенсуса, пока digest не совпадет.
- Observadores, не являющиеся валидаторами, могут выставить `assume_valid = true`; Se você precisar de um acordo confidencial, não precisará de um conselho confiável.## Políticas ativas
- A ativação da ativação não é `AssetConfidentialPolicy`, установленную создателем или через governança:
  - `TransparentOnly`: configuração para uso; разрешены только прозрачные инструкции (`MintAsset`, `TransferAsset` e т.д.), uma operação blindada отклоняются.
  - `ShieldedOnly`: você tem instruções de configuração e transferência de dados; `RevealConfidential` foi fechado, mas as balanças não estão disponíveis.
  - `Convertible`: pode ser usado para proteger a proteção e proteção blindada, isolá-la rampa de ativação/desativação não.
- Политики следуют ограниченному FSM, isso não é bloqueado:
  - `TransparentOnly → Convertible` (piscina blindada sem uso).
  - `TransparentOnly → ShieldedOnly` (требует перехода и окна конверсии).
  -`Convertible → ShieldedOnly` (referência mínima).
  - `ShieldedOnly → Convertible` (нужен план миграции, чтобы notas protegidas оставались тратимыми).
  - `ShieldedOnly → TransparentOnly` запрещен, если blindado pool não пуст или governança не кодирует миграцию, которая раскрывает оставшиеся notas.
- Instrução de governança para `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` que ISI `ScheduleConfidentialPolicyTransition` e pode definir a configuração `CancelConfidentialPolicyTransition`. Para validar a garantia do mempool, o que não significa que sua transação não seja necessária para sua transferência e configuração детерминированно, если проверка политики изменилась бы посреди блока.
- Запланированные переходы применяются автоматически при открытии нового блока: когда высота входит в окно conversão (para апгрейдов `ShieldedOnly`) ou download `effective_height`, runtime обновляет `AssetConfidentialPolicy`, обновляет metadados `zk.policy` e очищает entrada pendente. A configuração de tempo de execução `ShieldedOnly` permite a configuração de execução, configuração de tempo de execução e login предупреждение, оставляя прежний режим.
- Os botões de configuração `policy_transition_delay_blocks` e `policy_transition_window_blocks` permitem uma ocupação mínima e períodos de carência, o que é possível кошелькам конвертировать notas вокруг переключения.
- `pending_transition.transition_id` também identificador de auditoria; governança é uma iniciativa que visa o financiamento ou o desenvolvimento de operações, operações que podem ser realizadas rampa de ativação/desativação.
- `policy_transition_window_blocks` para uso 720 (cerca de 12 horas por período de 60 segundos). Você está desenvolvendo a governança, пытающиеся задать более короткое уведомление.
- Gênesis manifesta e CLI потоки показывают текущие и ожидающие политики. Логика admissão читает политику во время исполнения, подтверждая, что каждая конфиденциальная инструкция разрешена.
- Checklist миграции — см. “Sequenciamento de migração” é uma opção para o plano de negócios integrado, que foi adicionado ao Milestone M0.

#### Мониторинг переходов через ToriiКошельки и аудиторы опрашивают `GET /v1/confidential/assets/{definition_id}/transitions`, чтобы проверить активную `AssetConfidentialPolicy`. A carga útil JSON é usada para identificar o ID do ativo canônico, a configuração do seu bloco, `current_mode` política, regra, эффективный на этой высоте (окна конверсии временно отдают `Convertible`), e ожидаемые идентификаторы параметров `vk_set_hash`/Poseidon/Pedersen. Para a implementação da governança, você verá o seguinte:

- `transition_id` — identificador de auditoria, возвращенный `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e вычисленный `window_open_height` (você pode usar o cabo de conversão ShieldedOnly).

Exemplo:

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

Verifique `404`, mas a ativação não é adequada. Se o produto não for instalado, o pólo `pending_transition` é `null`.

### Máquina política| Regime técnico | Regras de segurança | Produções | Ativação effect_height | Nomeação |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | Governança ativa o verificador/parâmetro do registro. Verifique `ScheduleConfidentialPolicyTransition` com `effective_height ≥ current_height + policy_transition_delay_blocks`. | A primeira vez foi feita em `effective_height`; piscina blindada становится доступен сразу.                   | Coloque-o em contato com a configuração de segurança da solução de problemas.               |
| Somente Transparente | Somente blindado | Para isso, o que você precisa, mais `policy_transition_window_blocks ≥ 1`.                                                         | Runtime автоматически входит в `Convertible` на `effective_height - policy_transition_window_blocks`; instale `ShieldedOnly` em `effective_height`. | Certifique-se de determinar uma conversão através das instruções de programação abertas.   |
| Conversível | Somente blindado | Instale o dispositivo em `effective_height ≥ current_height + policy_transition_delay_blocks`. A governança DEVE подтвердить (`transparent_supply == 0`) через metadados de auditoria; o tempo de execução prova isso no cut-over. | Essa é a semântica correta, é isso que você precisa. Se a configuração não for válida para `effective_height`, execute a operação com `PolicyTransitionPrerequisiteFailed`. | Ative a atividade na rede de confirmação de segurança.                                     |
| Somente blindado | Conversível | Запланированный переход; retirada de emergência ativa (`withdraw_height` не задан).                                    | Configuração de configuração em `effective_height`; revelar-rampas снова открываются, пока notas protegidas остаются валидными.                           | Use um certificado de verificação ou um teste de auditoria.                                          |
| Somente blindado | Somente Transparente | A governança é fornecida pelo `shielded_supply == 0` ou pelo plano de governança `EmergencyUnshield` (nuжны подписи аудиторов). | O tempo de execução foi definido como `Convertible` por `effective_height`; nestas instruções de configuração você será provado e ativado no modo de operação operação operacional. | Você pode encontrar uma instância. Переход автоматически отменяется, если какая-либо конфиденциальная note расходуется в окне. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` очищает ожидающее изменение.                                                        | `pending_transition` não foi instalado.                                                                          | Сохраняет статус-кво; показано для полноты.                                             |Antes, você não pode se preocupar com a governança. Runtime проверяет предпосылки прямо перед применением запланированного перехода; Você não deve ativar a atividade na configuração padrão e abrir o `PolicyTransitionPrerequisiteFailed` no bloco de telefone e de rede.

### Последовательность миграции

1. **Restrições de segurança:** Ative o verificador e os parâmetros para ativar a política de segurança. Se você usar o `conf_features`, seus pares podem fornecer informações confiáveis.
2. **Explicação:** Selecione `ScheduleConfidentialPolicyTransition` com `effective_height`, use `policy_transition_delay_blocks`. Ao usar `ShieldedOnly`, você pode usar um conversor (`window ≥ policy_transition_window_blocks`).
3. **Instruções para operação:** Verifique o runbook `transition_id` e распространить runbook on/off-ramp. Кошельки и аудиторы подписываются на `/v1/confidential/assets/{id}/transitions`, чтобы узнать высоту открытия окна.
4. **Primенение окна:** Когда окно открывается, runtime переключает политику в `Convertible`, эмитирует `PolicyTransitionWindowOpened { transition_id }` и начинает отклонять конфликтующие governança-запросы.
5. **Finalizar ou corrigir:** No tempo de execução `effective_height`, você pode testar a operação (nova operação) предложение, отсутствие retiradas de emergência e т.п.). Успех переключает политику в запрошенный режим; ошибка эмитирует `PolicyTransitionPrerequisiteFailed`, очищает transição pendente e оставляет политику без изменений.
6. **Обновления схемы:** После успешного перехода governança повышает версию схемы актива (por exemplo, `asset_definition.v2`), uma ferramenta CLI требует `confidential_policy` при сериализации manifestos. Документы по апгрейду genesis инструктируют операторов добавить настройки политик отпечатки registro перед перезапуском validado.

Novas coisas, начинающие с включенной конфиденциальностью, кодируют желаемую политику непосредственно в gênese. Qual é a melhor maneira de verificar se você está usando as regras de execução de uma nova operação, faça uma conversão correta детерминированными, а кошельки успевали адаптироваться.

### Manifestos de versão e ativação Norito- Genesis manifesta ДОЛЖНЫ включать `SetParameter` para кастомного ключа `confidential_registry_root`. Carga útil - Norito JSON, соответствующий `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: опускайте поле (`null`), когда активных verificador entradas net, иначе Use uma estrutura hexadecimal de 32 bits (`0x…`). Quando você inicia o sistema, esse parâmetro é excluído ou não é atualizado com o registro de registro.
- On-wire `ConfidentialFeatureDigest::conf_rules_version` встраивает версию manifesto de layout. Para a versão v1 em ДОЛЖЕН, instale `Some(1)` e `iroha_config::parameters::defaults::confidential::RULES_VERSION`. O conjunto de regras эволюционирует, увеличьте константу, пересоздайте manifestos e разверните бинарники синхронно; A versão correta mostra o bloco de abertura de validação com `ConfidentialFeatureDigestMismatch`.
- Manifestos de ativação ДОЛЖНЫ связывать обновления registro, изменения жизненного цикла параметров и переходы политик, чтобы digest consistência garantida:
  1. Verifique o registro de configuração do plano (`Publish*`, `Set*Lifecycle`) em офлайн-снимке состояния e вычислите digest post post A ativação é `compute_confidential_feature_digest`.
  2. Эмитируйте `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`, используя вычисленный хэш, чтобы отстающие peers могли восстановить resumo correto, há apenas uma proposta de instruções de registro.
  3. Insira as instruções `ScheduleConfidentialPolicyTransition`. Каждая инструкция должна цитировать выданный governança `transition_id`; manifestos, que são executados, são executados em tempo de execução.
  4. Сохраните байты manifest, SHA-256 отпечаток и digest, использованный в plano de ativação. O operador irá verificar se três artefatos são perfeitos para que você possa realizar a operação corretamente.
- Когда rollout требует отложенного cut-over, запишите целевую высоту в сопутствующий кастомный parâmetro (por exemplo, `custom.confidential_upgrade_activation_height`). Este é o auditor Norito-кодированное доказательство того, что валидаторы соблюли окно уведомления до вступления изменения resumo.## Жизненный цикл verificador e parâmetros
### Registro ZK
- Ledger хранит `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`, где `proving_system` сейчас фиксирован на `Halo2`.
- Par `(circuit_id, version)` global único; registro pode ser usado para indicar o circuito do metadado. Попытки зарегистрировать дубликат отклоняются при admissão.
- `circuit_id` não pode ser usado, e `public_inputs_schema_hash` é compatível (обычно Blake2b-32 хэш канонического verificador público). Admissão отклоняет записи без этих полей.
- Инструкции governança включают:
  - `PUBLISH` para a entrada `Proposed` contém metadados.
  - `ACTIVATE { vk_id, activation_height }` para planejamento de ativação de entrada na granizo эпохи.
  - `DEPRECATE { vk_id, deprecation_height }` para obter mais informações, suas provas podem ser registradas na entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para abertura de configuração; затронутые активы замораживают конфиденциальные траты после retirar altura, пока не активируются novas entradas.
- Genesis manifesta автоматически эмитируют кастомный parâmetro `confidential_registry_root` com `vk_set_hash`, совпадающим с активными entradas; валидация кросс-проверяет этот digest с локальным состоянием registro para uso em консенсус.
- Регистрация или обновление verificador требует `gas_schedule_id`; provando isso, verifique o registro como `Active`, verifique o índice `(circuit_id, version)` e obtenha provas Halo2 содержали `OpenVerifyEnvelope`, у которого `circuit_id`, `vk_hash` e `public_inputs_schema_hash` совпадают совпадают с записью registro.

### Provando Chaves
- Chaves de prova остаются off-ledger, но на них ссылаются идентификаторы endereçados ao conteúdo (`pk_cid`, `pk_hash`, `pk_len`), опубликованные вместе с verificador de metadados.
- Wallet SDKs armazena PK данные, fornece segurança e proteção local.

### Parâmetros de Pedersen e Poseidon
- Registro Отдельные (`PedersenParams`, `PoseidonParams`) зеркалируют контроль жизненного цикла verificador, каждая с `params_id`, geradores/constantes, ativações, depreciação e retirada.
- Compromissos e хэши доменно разделяют `params_id`, поэтому ротация параметров никогда не переиспользует битовые padrões de устаревших наборов; ID inserido em notas de compromissos e domínio anulador.

## Determinação de métodos e nulificadores
- A função ativa pode ser `CommitmentTree` com `next_leaf_index`; Os blocos estabelecem compromissos na determinação do objetivo: iteração da transação no bloco; внутри каждой транзакции — saídas blindadas para conexão serial `output_idx`.
- `note_position` é removido do arquivo anterior, mas não **не** é usado no anulador; он используется только для путей adesão à testemunha de prova.
- Стабильность nullifier при reorg обеспечивается дизайном PRF; em PRF связывает `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, uma âncoras ссылаются на história raízes Merkle, ограниченные `max_anchor_age_blocks`.## Livro razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - Política de ativação ativa `Convertible` ou `ShieldedOnly`; admissão проверяет autoridade ativa, извлекает текущий `params_id`, сэмплирует `rho`, эмитирует compromisso, обновляет árvore Merkle.
   - Эмитирует `ConfidentialEvent::Shielded` com novo compromisso, Merkle root delta e hash вызова транзакции para trilha de auditoria.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - Syscall VM fornece prova de registro; host убеждается, что nulificadores não использованы, compromissos добавлены детерминированно, uma âncora свежий.
   - O razão registra entradas `NullifierSet`, armazena cargas úteis para получателей/аудиторов e эмитирует `ConfidentialEvent::Transferred`, anuladores de resumo, saídas de desempenho, hash de prova e raízes de Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - Доступно только для активов `Convertible`; prova подтверждает, что значение nota равно раскрытой сумме, razão начисляет прозрачный баланс e сжигает nota blindada, помечая anulador как потраченный.
   - Emita `ConfidentialEvent::Unshielded` com uma soma pública, anuladores de uso, prova de identificação e hash de transação.

## Modelo de dados de desenvolvimento
- `ConfidentialConfig` (nova configuração de configuração) com display de seta, `assume_valid`, botões газа/лимитов, âncora correta e verificador de backend.
- `ConfidentialNote`, `ConfidentialTransfer` e `ConfidentialMint` são os conjuntos Norito com sua própria versão (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` оборачивает AEAD memo bytes em `{ version, ephemeral_pubkey, nonce, ciphertext }`, по умолчанию `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para раскладки XChaCha20-Poly1305.
- Канонические векторы derivação de chave лежат в `docs/source/confidential_key_vectors.json`; e CLI e Torii são registrados no endpoint por uma determinada configuração.
- `asset::AssetDefinition` é usado para `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` сохраняет привязку `(backend, name, commitment)` para verificadores de transferência/desproteção; usando provas abertas, as chaves referenciadas ou a chave de verificação in-line não são suportadas pelo compromisso de segurança.
- `CommitmentTree` (para ativação em pontos de controle de fronteira), `NullifierSet` com `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` хранятся no estado mundial.
- Mempool fornece estruturas de configuração `NullifierIndex` e `AnchorIndex` para execução de downloads e testes âncora возраста.
- Обновления схемы Norito включают канонический порядок entradas públicas; testes de ida e volta garantem a codificação de determinação.
- Testes unitários de cargas úteis de ida e volta (`crates/iroha_data_model/src/confidential.rs`). Os arquivos de áudio vetorial fornecem transcrições AEAD canônicas para auditores. `norito.md` documenta o cabeçalho on-wire para o envelope.## Integração com IVM e syscall
- Verifique o syscall `VERIFY_CONFIDENTIAL_PROOF`, exemplo:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` e результирующий `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall verifica o verificador de metadados do registro, determina os limites de espaço / número, especifica a determinação de gás e fornece delta успехе prova.
- Host предоставляет traço somente leitura `ConfidentialLedger` para получения снимков Merkle root e статуса nullifier; A biblioteca Kotodama fornece ajudantes para testemunhar e validar o esquema.
- Документация ponteiro-ABI обновлена, чтобы прояснить buffer de prova de layout e identificadores de registro.

## Согласование возможностей узлов
- Aperto de mão entre `feature_bits.confidential` e `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Участие валидатора требует `confidential.enabled=true`, `assume_valid=false`, идентичных идентификаторов verificador de backend e совпадающих digest; Não há aperto de mão com `HandshakeConfidentialMismatch`.
- Конфигурация поддерживает `assume_valid` только для observer узлов: когда выключено, встреча конфиденциальных инструкций дает детерминированный `UnsupportedInstruction` sem pânico; когда включено, os observadores fornecem provas de deltas de estado объявленные.
- Mempool oferece transação confidencial, exceto capacidade local. Filtros de fofoca избегают отправки транзакций несовместимым peers, но слепо пересылают неизвестные IDs de verificador em пределах лимитов размера.

### Aperto de mão de Matriца совместимости

| Armazenamento de armazenamento | Ferramenta para validação de aplicativos | Operador de referência |
|----------------------------|----------------------------------------|----------------------|
| `enabled=true`, `assume_valid=false`, suporte de backend, suporte de resumo | Imprimir | Peer достигает состояния `Ready` e участвует в proposta, votação e RBC fan-out. Ручных действий не требуется. |
| `enabled=true`, `assume_valid=false`, backend совпадает, digest устарел ou отсутствует | Отклонен (`HandshakeConfidentialMismatch`) | Controle remoto para ativar o registro/parâmetros ou ativar o registro `activation_height`. Embora não seja um problema, você não pode fazer nada, não precisa se preocupar com a rotatividade. |
| `enabled=true`, `assume_valid=true` | Fechadura (`HandshakeConfidentialMismatch`) | Валидаторы требуют проверку provas; Instale o observador remoto com entrada para o Torii ou configure o `assume_valid=false` para obter a configuração correta. |
| `enabled=false`, um handshake de reconhecimento (imagem acima) ou verificador de backend obtido | Отклонен (`HandshakeConfidentialMismatch`) | Os pares de pares não podem ser negociados em conjunto. Обновите их текущего релиза и убедитесь, что backend + digest совпадают до переподключения. |

Observador usado, намеренно пропускающие проверку provas, não должны открывать консенсусные соединения с валидаторами, работающими с portas de capacidade. Você pode usar o bloco Torii ou a API de configuração, mas não há nenhuma solução объявят совместимые возможности.

### Политика удаления revela e anula a retençãoLedgers confidenciais são fornecidos com registros históricos para doação de notas e воспроизведения governança-аудитов. Política de segurança, exemplo `ConfidentialLedger`, seguinte:

- **Nulificador de execução:** хранить потраченные nulificadores минимум `730` дней (24 meses) после высоты траты или дольше, если требуется регулятором. O operador pode usar a chave `confidential.retention.nullifier_days`. Nullifiers, которые моложе окна, ДОЛЖНЫ оставаться доступными через Torii, чтобы аудиторы могли доказать отсутствие gasto duplo.
- **Удаление revelar:** прозрачные revelar (`RevealConfidential`) удаляют соответствующие compromissos немедленно после финализации блока, но использованный nullifier остается под правилом retenção выше. События revelado (`ConfidentialEvent::Unshielded`) фиксируют публичную сумму, получателя и hash prova, чтобы реконструкция histórico revelar не требовала удаленного texto cifrado.
- **Pontos de verificação de fronteira:** compromissos de fronteira permitem pontos de verificação contínuos, aumento de segurança em `max_anchor_age_blocks` e retenção de oкна. Use a compactação para verificar quais pontos de verificação são mais importantes do que os nullifiers no intervalo.
- **Ремедиация устаревшего digest:** если `HandshakeConfidentialMismatch` поднят из-за дрейфа digest, операторам следует (1) проверить, что окна retenção nullifier совпадают по кластеру, (2) запустить `iroha_cli app confidential verify-ledger` para пересчета digest по сохраненному набору nullifier, e (3) переразвернуть manifesto обновленный. Любые nulificadores, удаленные раньше срока, должны быть восстановлены из холодного хранения перед повторным входом в sim.

Documente a implementação local no runbook de operações; governança-politica, расширяющие окно retenção, должны синхронно обновлять конфигурацию узлов и planos архивного хранения.

###Processamento de vida e recuperação

1. Primeiro disque `IrohaNetwork`, você pode usar os recursos disponíveis. A luz não é compatível com `HandshakeConfidentialMismatch`; A solução é, um peer colocado na fila de descoberta não pode ser transferido para `Ready`.
2. Verifique a configuração no serviço de rede local (você não atualiza o resumo e o back-end) e Sumeragi não planeja peer para proposta ou votação.
3. Problema de operação do operador, registros e configurações de configuração (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou substitua `next_conf_features` com `activation_height`. Quando o resumo é atualizado, o aperto de mão é produzido automaticamente.
4. Если устаревший peer сумеет разослать блок (например, через архивный replay), валидаторы детерминированно отклоняют Eu tenho `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, verifique o registro de registro consistente no site.

### Fluxo de handshake seguro para repetição1. A caixa de seleção de áudio requer um material de ruído/X25519. Подписываемый carga útil de handshake (`handshake_signature_payload`) конкатенирует локальные e удаленные chaves públicas efêmeras, Norito-кодированный endereço de soquete anunciado e — если скомпилировано с `handshake_chain_id` — cadeia de identificação. Сообщение AEAD-шифруется перед отправкой.
2. Abra a carga útil com a classe peer/local e verifique o Ed25519, instalado em `HandshakeHelloV1`. Поскольку обе chaves efêmeras e endereço anunciado входят в домен подписи, replay захваченного сообщения против другого peer ou или você deve usar a solução para determinar a validade.
3. Sinalizadores de capacidade confidencial e `ConfidentialFeatureDigest` são transferidos por `HandshakeConfidentialMeta`. Organize a tupla `{ enabled, assume_valid, verifier_backend, digest }` com seu local localizado `ConfidentialHandshakeCaps`; É necessário usar o handshake com `HandshakeConfidentialMismatch` para transferir o transporte para `Ready`.
4. O operador de gerenciamento de resumo (через `compute_confidential_feature_digest`) e o administrador usam registros/políticas abertos antes повторным подключением. Pares, рекламирующие старые digests, продолжают проваливать handshake, предотвращая возвращение устаревшего состояния в набор validado.
5. O aperto de mão padrão e padrão é definido como padrão `iroha_p2p::peer` (`handshake_failure_count`, auxiliares de taxonomia de erros) e эмитят estruturar logs com ID de peer remoto e resumo de impressão digital. Ao selecionar este indicador, você verá um replay ou nenhuma configuração durante o lançamento.

## Atualizar chaves e cargas úteis
- Иерархия derivação ключей на аккаунт:
  - `sk_spend` → `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- Зашифрованные notas de cargas úteis usam AEAD com chaves compartilhadas derivadas de ECDH; As chaves de visualização do auditor opcionais podem ser configuradas para saídas em condições de política ativa.
- CLI de expansão: `confidential create-keys`, `confidential send`, `confidential export-view-key`, аудит-инструменты para расшифровки memo e auxiliar `iroha app zk envelope` para создания/инспекции Norito envelopes de memorando офлайн. Torii é usado para derivação de fluxo como `POST /v1/confidential/derive-keyset`, formato hexadecimal e base64, formato de caixa grande программно получать иерархии ключей.## Limites, limites e controles DoS
- Cronograma de gás Детерминированный:
  - Halo2 (Plonkish): базовый `250_000` gás + `2_000` gás на каждый entrada pública.
  - `5` gás à prova de água, mais por anulador (`300`) e por compromisso (`500`) начисления.
  - O operador pode executar a configuração de configuração constante (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); изменения применяются при старте ou hot-reload слоя конфигурации и детерминированно распространяются по кластеру.
- Жесткие лимиты (настраиваемые значения по умолчанию):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Provas, превышающие `verify_timeout_ms`, детерминированно прерывают инструкцию (governance-голосования эмитят `proof verification exceeded timeout`, `VerifyProof` foi removido).
- Os cabos originais são de qualidade: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` e `max_public_inputs` construir construtores de blocos; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) управляет pontos de verificação de fronteira de retenção.
- Runtime теперь отклоняет транзакции, превышающие эти por transação ou limites por bloco, эмитируя детерминированные ошибки `InvalidParameter` e não há registro de registro.
- Mempool предварительно фильтрует конфиденциальные транзакции по `vk_id`, длине prova e возрасту âncora para o verificador вызова, чтобы ограничить потребление ресурсов.
- Проверка детерминированно останавливается при timeout или нарушении лимитов; A transação é fornecida por você. Back-ends SIMD são opcionais, não há nenhuma função de contabilidade.

### Calibrações e critérios de calibração
- **Plataformas de referência.** Калибровочные прогоны ДОЛЖНЫ покрывать три профиля оборудования ниже. Os programas não têm seu perfil aberto antes da revisão.

  | Perfil | Arquitetura | CPU/Instância | Compilador de bandeiras | Atualizado |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Use as instruções do vetor; используется для настройки tabelas de custos de reserva. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | versão padrão | Valide o AVX2; Verifique se o SIMD está conectado ao gás neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versão padrão | Garantido, esse backend NEON é determinado e configurado com x86. |

- **Arnês de referência.** Quais são as opções de calibração de gás?
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para determinar as propriedades.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` é o código de operação da VM que contém o código de operação da VM.

- **Aleatoriedade fixa.** Экспортируйте `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` перед запуском бенчей, чтобы `iroha_test_samples::gen_account_in` переключился на A determinação é `KeyPair::from_seed`. Chicote de fios печатает `IROHA_CONF_GAS_SEED_ACTIVE=…` один раз; если переменная отсутствует, ревью ДОЛЖНО провалиться. Любые новые утилиты калибровки должны продолжать учитывать эту env var при введении вспомогательной случайности.- **Captura de resultados.**
  - Adicione resumos de critérios (`target/criterion/**/raw.csv`) para o perfil de lançamento do artefato.
  - Храните производные метрики (`ns/op`, `gas/op`, `ns/gas`) em [Lista de calibração de gás confidencial](./confidential-gas-calibration) em git commit e versão compilada.
  - Escolha duas linhas de base no perfil; удаляйте более старые снимки после проверки нового отчета.

- **Tolerâncias de aceitação.**
  - Дельты газа между `baseline-simd-neutral` e `baseline-avx2` ДОЛЖНЫ оставаться ≤ ±1,5%.
  - Дельты газа между `baseline-simd-neutral` e `baseline-neon` ДОЛЖНЫ оставаться ≤ ±2,0%.
  - Калибровочные предложения, выходящие за эти пороги, требуют либо корректировки расписания, либо RFC с объяснением расхождения и мерами.

- **Lista de verificação de revisão.** Confira a lista de verificação:
  - Включение `uname -a`, выдержек `/proc/cpuinfo` (modelo, passo) e `rustc -Vv` no registro de calibração.
  - Проверку, что `IROHA_CONF_GAS_SEED` отображается в выводе бенчей (бенчи печатают активный semente).
  - Убедиться, что marcapasso e sinalizadores de recurso de verificador confidencial совпадают с produção (`--features confidential,telemetry` при запуске бенчей с Telemetria).

## Configuração e operação
- `iroha_config` получает секцию `[confidential]`:
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
- Métricas de medição de temperatura: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` e `confidential_policy_transitions_total` não contêm texto simples.
- Superfícies RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## Teste de estratégia
- Детерминизм: рандомизированное перемешивание транзакций в блоках дает одинаковые raízes Merkle e conjuntos anuladores.
- Устойчивость к reorg: симуляция многоблочных reorg с âncoras; nullifiers остаются стабильными, а устаревшие âncoras отклоняются.
- Инварианты газа: одинаковый расход газа на узлах с и без SIMD ускорения.
- Граничное тестирование: provas em потолках размера/gas, максимальные входы/выходы, tempo limite de aplicação.
- Жизненный цикл: governança de operação para verificador de ativação/deatividade e parâmetros, testes de rotatividade.
- Política FSM: разрешенные/запрещенные переходы, задержки transição pendente e отклонения mempool вокруг alturas efetivas.
- Чрезвычайные ситуации registro: retirada de emergência замораживает затронутые активы на `withdraw_height` e отклоняет provas после нее.
- Gating de capacidade: валидаторы с несовпадающими `conf_features` отклоняют блоки; observadores com `assume_valid=true` fornecem suporte sem consentimento.
- Эквивалентность состояния: validador/completo/observador узлы создают идентичные raízes de estado em канонической цепи.
- Fuzzing negativo: provas de поврежденные, cargas úteis superdimensionadas e anulador de colisões детерминированно отклоняются.## Migração e migração
- Implementação controlada por recursos: para passar a Fase C3 `enabled` para o `false`; Você pode usar os recursos de teste antes de usá-los para validá-los.
- Прозрачные активы не затронуты; As instruções de configuração fornecem capacidade de registro e transferência.
- Узлы, собранные без поддержки confidencial, детерминированно отклоняют соответствующие блоки; mas não podemos nos validar, não podemos trabalhar como observadores em `assume_valid=true`.
- Genesis manifesta включают начальные entradas de registro, наборы параметров, конфиденциальные политики для активов и опциональные chaves de auditor.
- Операторы следуют опубликованным runbooks para registro de rotação, política de transferência e retirada de emergência, чтобы поддерживать детерминированные апгрейды.

## Незавершенная работа
- Verifique o benchmark dos parâmetros do Halo2 (circuito de configuração, pesquisa de estratégia) e avalie os resultados no manual de calibração, verifique os parâmetros de gás/tempo limite consulte o `confidential_assets_calibration.md`.
- Use políticas de gerenciamento de auditoria e API de visualização seletiva, fornecendo fluxo de trabalho alternativo em Torii governança одобрения.
- Crie um esquema de criptografia de testemunha para saídas de vários destinatários e memorandos em lote, distribua envelope de formato para implementadores de SDK.
- Заказать внешнюю circuitos de revisão de segurança, registro e processamento de parâmetros de rotações e arquivos de relatórios de auditoria.
- Специфицировать API сверки gastos para auditores e опубликовать orientação sobre escopo de chave de visualização, чтобы вендоры кошельков реализовали те Estas são as evidências semânticas.## Этапы реализации
1. **Fase M0 — Endurecimento Stop-Ship**
   - ✅ Anulador de derivação теперь следует дизайну Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) с детерминированным por compromissos no livro razão.
   - ✅ Исполнение применяет лимиты размера prova e квоты конфиденциальных операций por transação/por bloco, отклоняя tranзакции сверх лимитов детерминированными ошибками.
   - ✅ Handshake P2P объявляет `ConfidentialFeatureDigest` (backend digest + отпечатки registro) e детерминированно отклоняет несоответствия через `HandshakeConfidentialMismatch`.
   - ✅ Удалены panics em конфиденциальных caminhos de execução e добавлен role gating para несовместимых узлов.
   - ⚪ Применить бюджеты timeout para verificador e limites глубины reorg para pontos de verificação de fronteira.
     - ✅ Бюджеты timeout для проверки применены; provas, verifique `verify_timeout_ms`, теперь детерминированно падают.
     - ✅ Pontos de verificação de fronteira теперь учитывают `reorg_depth_bound`, pontos de verificação удаляя старше заданного окна при сохранении детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`, política FSM e portões de aplicação para instruir/transferir/revelar.
   - Verifique `conf_features` em blocos de segurança e verifique seus resumos de registro/parâmetros.
2. **Fase M1 — Registros e Parâmetros**
   - Registro de registro `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` com operações de governança, ancoragem de gênese e управлением кэшем.
   - Permite syscall para realizar pesquisas de registro, IDs de programação de gás, hash de esquema e verificações de tamanho.
   - Поставить формат зашифрованного payload v1, векторы derivação ключей кошелька и поддержку CLI para управления chaves confidenciais.
3. **Fase M2 — Gás e Desempenho**
   - Realizar determinação do cronograma de gás, configurações por bloco e chicotes de benchmark com telemetria (verificar latência, tamanhos de prova, rejeições de mempool).
   - Verifique pontos de verificação CommitmentTree, carregamento LRU e índices anuladores para vários ativos.
4. **Fase M3 – Rotação e Ferramentas de Carteira**
   - Включить поддержку provas multiparâmetros e multi-versão; fornecer ativação/descontinuação orientada por governança para runbooks de transição.
   - Faça uma migração de carteira para SDK/CLI de carteira, verificação de fluxo de trabalho, auditoria e gastos com ferramentas.
5. **Fase M4 — Auditoria e Operações**
   - Fluxos de trabalho para chaves de auditor, API de divulgação seletiva e runbooks operacionais.
   - Запланировать внешнюю криптографическую/безопасностную проверку и опубликовать выводы в `status.md`.

Каждая фаза обновляет marcos do roteiro e testes de segurança, ajuda para determinar a garantia de uso para blockchain sete.