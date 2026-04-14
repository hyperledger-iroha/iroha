<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guia de conta universal

Este guia descreve os requisitos de implementação do UAID (Universal Account ID) de
o roteiro Nexus e os empacota em um passo a passo focado no operador + SDK.
Abrange derivação UAID, inspeção de portfólio/manifesto, modelos reguladores,
e as evidências que devem acompanhar cada manifesto do diretório espacial do aplicativo `iroha
publicar` run (roadmap reference: `roadmap.md:2209`).

## 1. Referência rápida do UAID- UAIDs são literais `uaid:<hex>`, onde `<hex>` é um resumo Blake2b-256 cujo
  LSB está definido como `1`. O tipo canônico vive em
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Os registros de conta (`Account` e `AccountDetails`) agora carregam um `uaid` opcional
  campo para que os aplicativos possam aprender o identificador sem hash personalizado.
- Políticas de identificadores de função oculta podem vincular entradas normalizadas arbitrárias
  (números de telefone, e-mails, números de contas, strings de parceiros) para IDs `opaque:`
  sob um namespace UAID. As peças da corrente são `IdentifierPolicy`,
  `IdentifierClaimRecord` e o índice `opaque_id -> uaid`.
- O Space Directory mantém um mapa `World::uaid_dataspaces` que vincula cada UAID
  às contas de espaço para dados referenciadas por manifestos ativos. Torii reutiliza isso
  mapa para as APIs `/portfolio` e `/uaids/*`.
- `POST /v1/accounts/onboard` publica um manifesto padrão do Space Directory para
  o espaço de dados global quando não existe nenhum, então o UAID é imediatamente vinculado.
  As autoridades de integração devem possuir `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- Todos os SDKs expõem auxiliares para canonizar literais UAID (por exemplo,
  `UaidLiteral` no Android SDK). Os ajudantes aceitam resumos brutos de 64 hexadecimais
  (LSB = 1) ou literais `uaid:<hex>` e reutilize os mesmos codecs Norito para que o
  o resumo não pode variar entre idiomas.

## 1.1 Políticas de identificadores ocultos

Os UAIDs são agora a âncora para uma segunda camada de identidade:- Um `IdentifierPolicyId` global (`<kind>#<business_rule>`) define o
  namespace, metadados de compromisso público, chave de verificação do resolvedor e o
  modo de normalização de entrada canônica (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` ou `AccountNumber`).
- Uma reclamação vincula um identificador `opaque:` derivado a exatamente um UAID e um
  canônico `AccountId` sob essa política, mas a cadeia só aceita o
  reclamação quando acompanhada por um `IdentifierResolutionReceipt` assinado.
- A resolução permanece um fluxo `resolve -> transfer`. Torii resolve o opaco
  manipula e retorna o `AccountId` canônico; as transferências ainda visam o
  conta canônica, não literais `uaid:` ou `opaque:` diretamente.
- As políticas agora podem publicar parâmetros de criptografia de entrada BFV por meio de
  `PolicyCommitment.public_parameters`. Quando presente, Torii os anuncia em
  `GET /v1/identifier-policies`, e os clientes podem enviar entradas encapsuladas em BFV
  em vez de texto simples. As políticas programadas envolvem os parâmetros BFV em um
  pacote canônico `BfvProgrammedPublicParameters` que também publica o
  público `ram_fhe_profile`; cargas úteis BFV brutas herdadas são atualizadas para esse
  pacote canônico quando o compromisso é reconstruído.
- As rotas do identificador passam pelo mesmo token de acesso e limite de taxa Torii
  verifica como outros endpoints voltados para o aplicativo. Eles não são um desvio do normal
  Política de API.

## 1.2 Terminologia

A divisão de nomenclatura é intencional:- `ram_lfe` é a abstração externa de função oculta. Abrange a política
  registro, compromissos, metadados públicos, recibos de execução e
  modo de verificação.
- `BFV` é o esquema de criptografia homomórfica Brakerski/Fan-Vercauteren usado por
  alguns back-ends `ram_lfe` para avaliar a entrada criptografada.
- `ram_fhe_profile` são metadados específicos do BFV, não um segundo nome para o todo
  recurso. Descreve a máquina programada de execução de BFV que carteiras e
  os verificadores devem direcionar quando uma política usa o back-end programado.

Em termos concretos:

- `RamLfeProgramPolicy` e `RamLfeExecutionReceipt` são tipos de camada LFE.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` e
  `BfvRamProgramProfile` são tipos de camada FHE.
- `HiddenRamFheProgram` e `HiddenRamFheInstruction` são nomes internos para
  o programa BFV oculto executado pelo backend programado. Eles ficam no
  Lado FHE porque descrevem o mecanismo de execução criptografado em vez de
  a política externa ou abstração de recibo.

## 1.3 Identidade da conta versus aliases

A implementação da conta universal não altera o modelo de identidade da conta canônica:- `AccountId` continua sendo o assunto da conta canônica e sem domínio.
- Os valores `AccountAlias` são ligações SNS separadas sobre esse assunto. Um
  alias qualificado pelo domínio, como `merchant@hbl.sbp` e um alias raiz do espaço de dados
  como `merchant@sbp` podem resolver para o mesmo `AccountId` canônico.
- O registro da conta canônica é sempre `Account::new(AccountId)`/
  `NewAccount::new(AccountId)`; não há domínio qualificado ou materializado por domínio
  caminho de registro.
- Propriedade de domínio, permissões de alias e outros comportamentos no escopo do domínio em tempo real
  em seu próprio estado e APIs, e não na própria identidade da conta.
- A pesquisa de conta pública segue essa divisão: as consultas de alias permanecem públicas, enquanto
  a identidade da conta canônica permanece um `AccountId` puro.

Regra de implementação para operadores, SDKs e testes: comece do canônico
`AccountId` e, em seguida, adicione concessões de alias, permissões de espaço para dados/domínio e quaisquer
estado de propriedade do domínio separadamente. Não sintetize uma conta falsa derivada de um alias
ou espere qualquer campo de domínio vinculado nos registros da conta apenas porque um alias ou
rota carrega um segmento de domínio.

Rotas Torii atuais:| Rota | Finalidade |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lista as políticas do programa RAM-LFE ativas e inativas, além de seus metadados de execução pública, incluindo parâmetros BFV `input_encryption` opcionais e o back-end programado `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Aceita exatamente um de `{ input_hex }` ou `{ encrypted_input }` e retorna o `RamLfeExecutionReceipt` sem estado mais `{ output_hex, output_hash, receipt_hash }` para o programa selecionado. O tempo de execução Torii atual emite recibos para o backend BFV programado. |
| `POST /v1/ram-lfe/receipts/verify` | Valida apátrida um `RamLfeExecutionReceipt` em relação à política do programa on-chain publicada e, opcionalmente, verifica se um `output_hex` fornecido pelo chamador corresponde ao recibo `output_hash`. |
| `GET /v1/identifier-policies` | Lista namespaces de política de função oculta ativos e inativos, além de seus metadados públicos, incluindo parâmetros BFV `input_encryption` opcionais, o modo `normalization` necessário para entrada criptografada do lado do cliente e `ram_fhe_profile` para políticas BFV programadas. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Aceita exatamente um de `{ input }` ou `{ encrypted_input }`. O texto simples `input` é normalizado no lado do servidor; O BFV `encrypted_input` já deve estar normalizado de acordo com o modo de política publicado. O endpoint então deriva o identificador `opaque:` e retorna um recibo assinado que `ClaimIdentifier` pode enviar na cadeia, incluindo o `signature_payload_hex` bruto e o `signature_payload` analisado. || `POST /v1/identifiers/resolve` | Aceita exatamente um de `{ input }` ou `{ encrypted_input }`. O texto simples `input` é normalizado no lado do servidor; O BFV `encrypted_input` já deve estar normalizado de acordo com o modo de política publicado. O ponto de extremidade resolve o identificador em `{ opaque_id, receipt_hash, uaid, account_id, signature }` quando existe uma declaração ativa e também retorna a carga canônica assinada como `{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Procura o `IdentifierClaimRecord` persistente vinculado a um hash de recibo determinístico para que operadores e SDKs possam auditar a propriedade da declaração ou diagnosticar falhas de reprodução/incompatibilidade sem verificar o índice completo do identificador. |

O tempo de execução de execução em processo do Torii está configurado em
`torii.ram_lfe.programs[*]`, codificado por `program_id`. O identificador roteia agora
reutilizar o mesmo tempo de execução RAM-LFE em vez de um `identifier_resolver` separado
superfície de configuração.

Suporte atual do SDK:- `normalizeIdentifierInput(value, normalization)` corresponde à ferrugem
  canonicalizadores para `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address` e `account_number`.
- `ToriiClient.listIdentifierPolicies()` lista metadados de política, incluindo BFV
  metadados de criptografia de entrada quando a política os publica, além de um decodificado
  Objeto de parâmetro BFV via `input_encryption_public_parameters_decoded`.
  As políticas programadas também expõem o `ram_fhe_profile` decodificado. Esse campo é
  intencionalmente com escopo BFV: permite que as carteiras verifiquem o registro esperado
  contagem, contagem de faixas, modo de canonização e módulo mínimo de texto cifrado para
  o back-end FHE programado antes de criptografar a entrada do lado do cliente.
-`getIdentifierBfvPublicParameters(policy)` e
  Ajuda `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })`
  Chamadores JS consomem metadados BFV publicados e criam solicitações com reconhecimento de política
  órgãos sem reimplementar regras de identificação de políticas e normalização.
-`encryptIdentifierInputForPolicy(policy, input, { seedHex? })` e
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` agora deixe
  As carteiras JS constroem o envelope de texto cifrado BFV Norito completo localmente a partir de
  parâmetros de política publicados em vez de enviar hexadecimal de texto cifrado pré-construído.
-`ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  resolve um identificador oculto e retorna a carga útil do recibo assinado,
  incluindo `receipt_hash`, `signature_payload_hex` e
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, {policyId, input |
  criptografadoInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifica o retornado
  recibo contra a chave do resolvedor de política no lado do cliente, e`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` busca o
  registro de declaração persistente para fluxos de auditoria/depuração posteriores.
- `IrohaSwift.ToriiClient` agora expõe `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  e `getIdentifierClaimByReceiptHash(_)`, mais
  `ToriiIdentifierNormalization` para o mesmo telefone/e-mail/número de conta
  modos de canonização.
- `ToriiIdentifierLookupRequest` e o
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  Os auxiliares `.encryptedRequest(...)` fornecem a superfície de solicitação Swift digitada para
  resolver e reivindicar chamadas de recebimento, e as políticas Swift agora podem derivar o BFV
  texto cifrado localmente via `encryptInput(...)` / `encryptedRequest(input:...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` valida isso
  os campos de recibo de nível superior correspondem à carga assinada e verificam o
  assinatura do resolvedor do lado do cliente antes do envio.
- `HttpClientTransport` no Android SDK agora expõe
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, entrada,
  criptografadoInputHex)`, `issueIdentifierClaimReceipt(accountId, policyId,
  entrada, criptografadaInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  mais `IdentifierNormalization` para as mesmas regras de canonização.
- `IdentifierResolveRequest` e o
  `IdentifierPolicySummary.plaintextRequest(...)` /
  Os auxiliares `.encryptedRequest(...)` fornecem a superfície de solicitação Android digitada,
  enquanto `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` deriva o envelope de texto cifrado BFV
  localmente a partir de parâmetros de política publicados.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifica o retornado
  assinatura do resolvedor do lado do cliente.

Conjunto de instruções atual:-`RegisterIdentifierPolicy`
-`ActivateIdentifierPolicy`
- `ClaimIdentifier` (vinculado ao recebimento; reivindicações `opaque_id` brutas são rejeitadas)
-`RevokeIdentifier`

Agora existem três back-ends em `iroha_crypto::ram_lfe`:

- o PRF `HKDF-SHA3-512` vinculado ao compromisso histórico, e
- um avaliador afim secreto apoiado por BFV que consome identificador criptografado por BFV
  slots diretamente. Quando `iroha_crypto` é construído com o padrão
  Recurso `bfv-accel`, a multiplicação do anel BFV usa um determinístico exato
  Back-end CRT-NTT internamente; desabilitar esse recurso volta para o
  caminho escalar do livro escolar com saídas idênticas, e
- um avaliador programado secreto apoiado por BFV que deriva uma instrução orientada
  Rastreamento de execução no estilo RAM sobre registros criptografados e memória de texto cifrado
  pistas antes de derivar o identificador opaco e o hash de recebimento. O programado
  back-end agora requer um piso de módulo BFV mais forte do que o caminho afim, e
  seus parâmetros públicos são publicados em um pacote canônico que inclui o
  Perfil de execução RAM-FHE consumido por carteiras e verificadores.

Aqui, BFV significa o esquema Brakerski/Fan-Vercauteren FHE implementado em
`crates/iroha_crypto/src/fhe_bfv.rs`. É o mecanismo de execução criptografada
usado pelos back-ends afins e programados, não o nome do oculto externo
abstração de função.Torii usa o back-end publicado pelo compromisso de política. Quando o back-end do BFV
está ativo, as solicitações de texto simples são normalizadas e criptografadas no lado do servidor antes
avaliação. As solicitações BFV `encrypted_input` para o back-end afim são avaliadas
diretamente e já deve estar normalizado do lado do cliente; o back-end programado
canoniza a entrada criptografada de volta ao BFV determinístico do resolvedor
envelope antes de executar o programa RAM secreto para que os hashes de recebimento permaneçam
estável em textos cifrados semanticamente equivalentes.

## 2. Derivação e verificação de UAIDs

Existem três maneiras suportadas de obter um UAID:

1. **Leia-o no estado mundial ou nos modelos SDK.** Qualquer `Account`/`AccountDetails`
   carga útil consultada via Torii agora tem o campo `uaid` preenchido quando o
   participante optou por contas universais.
2. **Consulte os registros UAID.** Torii expõe
   `GET /v1/space-directory/uaids/{uaid}` que retorna as ligações do espaço de dados
   e metadados de manifesto que o host do Space Directory persiste (consulte
   `docs/space-directory.md` §3 para amostras de carga útil).
3. **Deduza-o de forma determinística.** Ao inicializar novos UAIDs off-line, hash
   a semente canônica do participante com Blake2b-256 e prefixe o resultado com
   `uaid:`. O trecho abaixo reflete o auxiliar documentado em
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```Sempre armazene o literal em letras minúsculas e normalize os espaços em branco antes do hash.
Ajudantes CLI, como `iroha app space-directory manifest scaffold` e Android
O analisador `UaidLiteral` aplica as mesmas regras de corte para que as revisões de governança possam
verificar valores sem scripts ad hoc.

## 3. Inspecionar acervos e manifestos da UAID

O agregador de portfólio determinístico em `iroha_core::nexus::portfolio`
apresenta cada par de ativo/espaço de dados que faz referência ao UAID. Operadores e SDKs
pode consumir os dados por meio das seguintes superfícies:

| Superfície | Uso |
|--------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Retorna espaço de dados → ativo → resumos de saldo; descrito em `docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | Lista IDs de espaço de dados + literais de conta vinculados ao UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Fornece o histórico completo do `AssetPermissionManifest` para auditorias. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Atalho CLI que envolve o endpoint de vinculações e, opcionalmente, grava o JSON no disco (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Busca o pacote JSON do manifesto para pacotes de evidências. |

Exemplo de sessão CLI (URL Torii configurada via `torii_api_url` em `iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Armazene os instantâneos JSON junto com o hash do manifesto usado durante as revisões; o
O observador do Space Directory reconstrói o mapa `uaid_dataspaces` sempre que se manifesta
ativar, expirar ou revogar, portanto, esses instantâneos são a maneira mais rápida de provar
quais ligações estavam ativas em uma determinada época.## 4. Publicação de manifestos de capacidade com evidências

Use o fluxo CLI abaixo sempre que uma nova licença for lançada. Cada passo deve
aterrissam no pacote de evidências registrado para aprovação da governança.

1. **Codifique o manifesto JSON** para que os revisores vejam o hash determinístico antes
   submissão:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Publique o subsídio** usando a carga útil Norito (`--manifest`) ou
   a descrição JSON (`--manifest-json`). Registre o recibo Torii/CLI mais
   o hash de instrução `PublishSpaceDirectoryManifest`:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture evidências de SpaceDirectoryEvent.** Inscreva-se para
   `SpaceDirectoryEvent::ManifestActivated` e inclua a carga útil do evento em
   o pacote para que os auditores possam confirmar quando a mudança ocorreu.

4. **Gere um pacote de auditoria** vinculando o manifesto ao seu perfil de espaço de dados e
   ganchos de telemetria:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Verifique as ligações via Torii** (`bindings fetch` e `manifests fetch`) e
   arquive esses arquivos JSON com o pacote hash + acima.

Lista de verificação de evidências:

- [ ] Hash do manifesto (`*.manifest.hash`) assinado pelo aprovador da alteração.
- [ ] recibo CLI/Torii para a chamada de publicação (stdout ou artefato `--json-out`).
- [] `SpaceDirectoryEvent` carga útil comprovando ativação.
- [] Diretório do pacote de auditoria com perfil de espaço para dados, ganchos e cópia do manifesto.
- [] Bindings + instantâneos de manifesto obtidos da pós-ativação Torii.Isso reflete os requisitos em `docs/space-directory.md` §3.2, ao mesmo tempo que fornece ao SDK
proprietários uma única página para apontar durante as análises de lançamento.

## 5. Modelos de manifesto regulador/regional

Use os fixtures in-repo como pontos de partida ao criar manifestos de capacidade
para reguladores ou supervisores regionais. Eles demonstram como definir o escopo de permissão/negação
regras e explicar as notas de política que os revisores esperam.

| Fixação | Finalidade | Destaques |
|--------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | Feed de auditoria da ESMA/ESRB. | Permissões somente leitura para `compliance.audit::{stream_reports, request_snapshot}` com negação de ganhos em transferências de varejo para manter os UAIDs reguladores passivos. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | Pista de supervisão JFSA. | Adiciona uma permissão `cbdc.supervision.issue_stop_order` limitada (janela PerDay + `max_amount`) e uma negação explícita em `force_liquidation` para impor controles duplos. |

Ao clonar esses fixtures, atualize:

1. IDs `uaid` e `dataspace` para corresponder ao participante e à pista que você está habilitando.
2. Janelas `activation_epoch`/`expiry_epoch` com base no cronograma de governança.
3. Campos `notes` com as referências políticas do regulador (artigo MiCA, JFSA
   circular, etc.).
4. Janelas de permissão (`PerSlot`, `PerMinute`, `PerDay`) e opcionais
   `max_amount` limita para que os SDKs imponham os mesmos limites que o host.

## 6. Notas de migração para consumidores de SDKAs integrações de SDK existentes que referenciam IDs de conta por domínio devem migrar para
as superfícies centradas no UAID descritas acima. Use esta lista de verificação durante atualizações:

  IDs de conta. Para Rust/JS/Swift/Android isso significa atualizar para a versão mais recente
  caixas de espaço de trabalho ou regeneração de ligações Norito.
- **Chamadas de API:** substitua consultas de portfólio com escopo de domínio por
  `GET /v1/accounts/{uaid}/portfolio` e os pontos de extremidade de manifesto/ligações.
  `GET /v1/accounts/{uaid}/portfolio` aceita uma consulta `asset_id` opcional
  parâmetro quando as carteiras precisam apenas de uma única instância de ativo. Ajudantes de clientes como
  como `ToriiClient.getUaidPortfolio` (JS) e o Android
  `SpaceDirectoryClient` já envolve essas rotas; prefira-os em vez de sob medida
  Código HTTP.
- **Cache e telemetria:** entradas de cache por UAID + espaço de dados em vez de bruto
  IDs de conta e emitir telemetria mostrando o literal UAID para que as operações possam
  alinhar logs com evidências do Space Directory.
- **Tratamento de erros:** novos endpoints retornam erros estritos de análise de UAID
  documentado em `docs/source/torii/portfolio_api.md`; trazer à tona esses códigos
  literalmente para que as equipes de suporte possam fazer a triagem dos problemas sem etapas de reprodução.
- **Testes:** Conecte os equipamentos mencionados acima (além de seus próprios manifestos UAID)
  em conjuntos de testes SDK para provar viagens de ida e volta Norito e avaliações de manifesto
  corresponder à implementação do host.

## 7. Referências- `docs/space-directory.md` — manual do operador com detalhes mais detalhados do ciclo de vida.
- `docs/source/torii/portfolio_api.md` — Esquema REST para portfólio UAID e
  pontos de extremidade manifestos.
- `crates/iroha_cli/src/space_directory.rs` — implementação CLI referenciada em
  este guia.
- `fixtures/space_directory/capability/*.manifest.json` — regulador, varejo e
  Modelos de manifesto CBDC prontos para clonagem.