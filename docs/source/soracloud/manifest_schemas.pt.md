<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Esquemas de manifesto SoraCloud V1

Esta página define os primeiros esquemas Norito determinísticos para SoraCloud
implantação em Iroha 3:

-`SoraContainerManifestV1`
-`SoraServiceManifestV1`
-`SoraStateBindingV1`
-`SoraDeploymentBundleV1`
-`AgentApartmentManifestV1`
-`FheParamSetV1`
-`FheExecutionPolicyV1`
-`FheGovernanceBundleV1`
-`FheJobSpecV1`
-`DecryptionAuthorityPolicyV1`
-`DecryptionRequestV1`
-`CiphertextQuerySpecV1`
-`CiphertextQueryResponseV1`
-`SecretEnvelopeV1`
-`CiphertextStateRecordV1`

As definições de Rust residem em `crates/iroha_data_model/src/soracloud.rs`.

Os registros de tempo de execução privado do modelo carregado são intencionalmente uma camada separada de
esses manifestos de implantação de SCR. Eles deveriam estender o plano modelo Soracloud
e reutilize `SecretEnvelopeV1` / `CiphertextStateRecordV1` para bytes criptografados
e estado nativo de texto cifrado, em vez de ser codificado como novo serviço/contêiner
manifesta. Consulte `uploaded_private_models.md`.

## Escopo

Esses manifestos são projetados para o `IVM` + Sora Container Runtime personalizado
(SCR) (sem WASM, sem dependência Docker na admissão em tempo de execução).- `SoraContainerManifestV1` captura identidade do pacote executável, tipo de tempo de execução,
  política de capacidade, recursos, configurações de investigação de ciclo de vida e
  exportações de configuração necessária para o ambiente de tempo de execução ou revisão montada
  árvore.
- `SoraServiceManifestV1` captura a intenção de implantação: identidade do serviço,
  hash/versão do manifesto do contêiner referenciado, roteamento, política de implementação e
  vinculações estaduais.
- `SoraStateBindingV1` captura escopo e limites determinísticos de gravação de estado
  (prefixo de namespace, modo de mutabilidade, modo de criptografia, cotas de item/total).
- `SoraDeploymentBundleV1` acopla contêiner + serviço manifesta e impõe
  verificações determinísticas de admissão (ligação hash-manifesto, alinhamento de esquema e
  capacidade/consistência de ligação).
- `AgentApartmentManifestV1` captura a política de tempo de execução do agente persistente:
  limites de ferramentas, limites de políticas, limites de gastos, cota estadual, saída de rede e
  comportamento de atualização.
- `FheParamSetV1` captura conjuntos de parâmetros FHE gerenciados por governança:
  identificadores determinísticos de backend/esquema, perfil de módulo, segurança/profundidade
  limites e alturas do ciclo de vida (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` captura limites determinísticos de execução de texto cifrado:
  tamanhos de carga útil admitidos, fan-in de entrada/saída, limites de profundidade/rotação/bootstrap,
  e modo de arredondamento canônico.
- `FheGovernanceBundleV1` acopla um conjunto de parâmetros e uma política para determinística
  validação de admissão.- `FheJobSpecV1` captura admissão/execução de trabalho de texto cifrado determinístico
  solicitações: classe de operação, compromissos de entrada ordenados, chave de saída e limite
  demanda de profundidade/rotação/bootstrap vinculada a um conjunto de política + parâmetro.
- `DecryptionAuthorityPolicyV1` captura a política de divulgação gerenciada pela governança:
  modo de autoridade (serviço mantido pelo cliente versus serviço de limite), quórum/membros de aprovadores,
  permissão de quebra de vidro, marcação de jurisdição, exigência de evidência de consentimento,
  Limites TTL e marcação de auditoria canônica.
- `DecryptionRequestV1` captura tentativas de divulgação vinculadas a políticas:
  referência de chave de texto cifrado (`binding_name` + `state_key` + compromisso),
  justificativa, etiqueta de jurisdição, hash opcional de evidência de consentimento, TTL,
  intenção/razão de quebra de vidro e ligação de hash de governança.
- `CiphertextQuerySpecV1` captura a intenção de consulta determinística somente de texto cifrado:
  escopo de serviço/ligação, filtro de prefixo de chave, limite de resultado limitado, metadados
  nível de projeção e alternância de inclusão de prova.
- `CiphertextQueryResponseV1` captura saídas de consulta minimizadas por divulgação:
  referências de chave orientadas a resumo, metadados de texto cifrado, provas de inclusão opcionais,
  e contexto de truncamento/sequência em nível de resposta.
- `SecretEnvelopeV1` captura o próprio material de carga útil criptografado:
  modo de criptografia, identificador/versão de chave, nonce, bytes de texto cifrado e
  compromissos de integridade.
- `CiphertextStateRecordV1` captura entradas de estado nativo de texto cifrado quecombinar metadados públicos (tipo de conteúdo, tags de política, compromisso, tamanho da carga útil)
  com um `SecretEnvelopeV1`.
- Os pacotes de modelos privados enviados pelo usuário devem ser baseados nesses nativos de texto cifrado
  registros:
  pedaços criptografados de peso/configuração/processador vivem no estado, enquanto o registro do modelo,
  linhagem de peso, perfis de compilação, sessões de inferência e pontos de verificação permanecem
  registros Soracloud de primeira classe.

## Versionamento

-`SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
-`SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
-`SORA_STATE_BINDING_VERSION_V1 = 1`
-`SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
-`AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
-`FHE_PARAM_SET_VERSION_V1 = 1`
-`FHE_EXECUTION_POLICY_VERSION_V1 = 1`
-`FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
-`FHE_JOB_SPEC_VERSION_V1 = 1`
-`DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
-`DECRYPTION_REQUEST_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
-`SECRET_ENVELOPE_VERSION_V1 = 1`
-`CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

A validação rejeita versões não suportadas com
`SoraCloudManifestError::UnsupportedVersion`.

## Regras de validação determinísticas (V1)- Manifesto do contêiner:
  - `bundle_path` e `entrypoint` não devem estar vazios.
  - `healthcheck_path` (se definido) deve começar com `/`.
  - `config_exports` pode fazer referência apenas a configurações declaradas em
    `required_config_names`.
  - os destinos ambientais de exportação de configuração devem usar nomes canônicos de variáveis de ambiente
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - os destinos do arquivo de exportação de configuração devem permanecer relativos, usar separadores `/` e
    não deve conter segmentos vazios `.` ou `..`.
  - as exportações de configuração não devem ter como alvo o mesmo env var ou caminho de arquivo relativo mais
    mais de uma vez.
- Manifesto de serviço:
  - `service_version` não deve estar vazio.
  - `container.expected_schema_version` deve corresponder ao esquema do contêiner v1.
  - `rollout.canary_percent` deve ser `0..=100`.
  - `route.path_prefix` (se definido) deve começar com `/`.
  - os nomes de ligação de estado devem ser exclusivos.
- Vinculação estadual:
  - `key_prefix` não deve estar vazio e começar com `/`.
  -`max_item_bytes <= max_total_bytes`.
  - As ligações `ConfidentialState` não podem usar criptografia de texto simples.
- Pacote de implantação:
  - `service.container.manifest_hash` deve corresponder à codificação canônica
    hash de manifesto do contêiner.
  - `service.container.expected_schema_version` deve corresponder ao esquema do contêiner.
  - As ligações de estado mutáveis ​​requerem `container.capabilities.allow_state_writes=true`.
  - As rotas públicas requerem `container.lifecycle.healthcheck_path`.
- Manifesto do apartamento do agente:
  - `container.expected_schema_version` deve corresponder ao esquema do contêiner v1.
  - os nomes dos recursos das ferramentas não devem ser vazios e devem ser exclusivos.- os nomes dos recursos de política devem ser exclusivos.
  - os ativos com limite de gastos devem ser únicos e não vazios.
  - `max_per_tx_nanos <= max_per_day_nanos` para cada limite de gasto.
  - a política de rede da lista de permissões deve incluir hosts exclusivos não vazios.
- Conjunto de parâmetros FHE:
  - `backend` e `ciphertext_modulus_bits` não devem estar vazios.
  - cada tamanho de bit do módulo de texto cifrado deve estar dentro de `2..=120`.
  - a ordem da cadeia do módulo do texto cifrado deve ser não crescente.
  - `plaintext_modulus_bits` deve ser menor que o maior módulo do texto cifrado.
  -`slot_count <= polynomial_modulus_degree`.
  -`max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - a ordem da altura do ciclo de vida deve ser rigorosa:
    `activation < deprecation < withdraw` quando presente.
  - requisitos de status do ciclo de vida:
    - `Proposed` não permite alturas de depreciação/retirada.
    - `Active` requer `activation_height`.
    - `Deprecated` requer `activation_height` + `deprecation_height`.
    - `Withdrawn` requer `activation_height` + `withdraw_height`.
- Política de execução do FHE:
  -`max_plaintext_bytes <= max_ciphertext_bytes`.
  -`max_output_ciphertexts <= max_input_ciphertexts`.
  - a ligação do conjunto de parâmetros deve corresponder a `(param_set, version)`.
  - `max_multiplication_depth` não deve exceder a profundidade definida no parâmetro.
  - a admissão da política rejeita o ciclo de vida do conjunto de parâmetros `Proposed` ou `Withdrawn`.
- Pacote de governança FHE:
  - valida a compatibilidade de política + conjunto de parâmetros como uma carga útil de admissão determinística.
- Especificações do trabalho FHE:
  - `job_id` e `output_state_key` não devem estar vazios (`output_state_key` começa com `/`).- o conjunto de entrada não deve estar vazio e as chaves de entrada devem ser caminhos canônicos exclusivos.
  - as restrições específicas da operação são rigorosas (entrada múltipla `Add`/`Multiply`,
    `RotateLeft`/`Bootstrap` de entrada única, com botões de profundidade/rotação/bootstrap mutuamente exclusivos).
  - a admissão vinculada à política impõe:
    - correspondência de identificadores de política/parâmetros e versões.
    - a contagem/bytes de entrada, a profundidade, a rotação e os limites de inicialização estão dentro dos limites da política.
    - os bytes de saída projetados determinísticos se ajustam aos limites do texto cifrado da política.
- Política de autoridade de descriptografia:
  - `approver_ids` deve ser não vazio, exclusivo e classificado estritamente lexicograficamente.
  - O modo `ClientHeld` requer exatamente um aprovador, `approver_quorum=1`,
    e `allow_break_glass=false`.
  - O modo `ThresholdService` requer pelo menos dois aprovadores e
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` não deve estar vazio e não deve conter caracteres de controle.
  - `audit_tag` não deve estar vazio e não deve conter caracteres de controle.
- Solicitação de descriptografia:
  - `request_id`, `state_key` e `justification` não devem estar vazios
    (`state_key` começa com `/`).
  - `jurisdiction_tag` não deve estar vazio e não deve conter caracteres de controle.
  - `break_glass_reason` é obrigatório quando `break_glass=true` e deve ser omitido quando
    `break_glass=false`.
  - a admissão vinculada à política impõe igualdade de nome de política, não solicita TTLexcedendo `policy.max_ttl_blocks`, igualdade de etiqueta de jurisdição, quebra de vidro
    requisitos de controle e evidência de consentimento quando
    `policy.require_consent_evidence=true` para solicitações sem quebra de vidro.
- Especificação de consulta de texto cifrado:
  - `state_key_prefix` não deve estar vazio e começar com `/`.
  - `max_results` é limitado deterministicamente (`<=256`).
  - a projeção de metadados é explícita (somente resumo `Minimal` vs chave visível `Standard`).
- Resposta de consulta de texto cifrado:
  - `result_count` deve ser igual à contagem de linhas serializadas.
  - A projeção `Minimal` não deve expor `state_key`; `Standard` deve expô-lo.
  - as linhas nunca devem surgir no modo de criptografia de texto simples.
  - as provas de inclusão (quando presentes) devem incluir IDs de esquema não vazios e
    `anchor_sequence >= event_sequence`.
- Envelope secreto:
  - `key_id`, `nonce` e `ciphertext` não devem estar vazios.
  - o comprimento do nonce é limitado (bytes `<=256`).
  - o comprimento do texto cifrado é limitado (bytes `<=33554432`).
- Registro de estado de texto cifrado:
  - `state_key` não deve estar vazio e começar com `/`.
  - o tipo de conteúdo dos metadados não deve estar vazio; tags devem ser strings exclusivas e não vazias.
  - `metadata.payload_bytes` deve ser igual a `secret.ciphertext.len()`.
  - `metadata.commitment` deve ser igual a `secret.commitment`.

## luminárias canônicas

Os fixtures JSON canônicos são armazenados em:-`fixtures/soracloud/sora_container_manifest_v1.json`
-`fixtures/soracloud/sora_service_manifest_v1.json`
-`fixtures/soracloud/sora_state_binding_v1.json`
-`fixtures/soracloud/sora_deployment_bundle_v1.json`
-`fixtures/soracloud/agent_apartment_manifest_v1.json`
-`fixtures/soracloud/fhe_param_set_v1.json`
-`fixtures/soracloud/fhe_execution_policy_v1.json`
-`fixtures/soracloud/fhe_governance_bundle_v1.json`
-`fixtures/soracloud/fhe_job_spec_v1.json`
-`fixtures/soracloud/decryption_authority_policy_v1.json`
-`fixtures/soracloud/decryption_request_v1.json`
-`fixtures/soracloud/ciphertext_query_spec_v1.json`
-`fixtures/soracloud/ciphertext_query_response_v1.json`
-`fixtures/soracloud/secret_envelope_v1.json`
-`fixtures/soracloud/ciphertext_state_record_v1.json`

Testes de fixação/ida e volta:

-`crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`