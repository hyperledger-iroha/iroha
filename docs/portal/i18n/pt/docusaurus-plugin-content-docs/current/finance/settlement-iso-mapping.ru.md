---
lang: pt
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mapeamento iso de liquidação
título: Acordo ↔ Mapeamento de Campo ISO 20022
sidebar_label: Acordo ↔ ISO 20022
descrição: Mapeamento canônico entre os fluxos de liquidação Iroha e a ponte ISO 20022.
---

:::nota Fonte Canônica
:::

## Acordo ↔ Mapeamento de Campo ISO 20022

Esta nota captura o mapeamento canônico entre as instruções de liquidação Iroha
(`DvpIsi`, `PvpIsi`, fluxos colaterais de recompra) e as mensagens ISO 20022 exercidas
pela ponte. Ele reflete a estrutura de mensagens implementada em
`crates/ivm/src/iso20022.rs` e serve como referência na produção ou
validando cargas úteis Norito.

### Política de Dados de Referência (Identificadores e Validação)

Esta política agrupa as preferências do identificador, as regras de validação e os dados de referência
obrigações que a ponte Norito ↔ ISO 20022 deve cumprir antes de emitir mensagens.

**Pontos de ancoragem dentro da mensagem ISO:**
- **Identificadores do instrumento** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (ou o campo de instrumento equivalente).
- **Partes/agentes** → `DlvrgSttlmPties/Pty` e `RcvgSttlmPties/Pty` para `sese.*`,
  ou as estruturas de agente em `pacs.009`.
- **Contas** → Elementos `…/Acct` para contas de custódia/dinheiro; espelhar o livro-razão
  `AccountId` em `SupplementaryData`.
- **Identificadores proprietários** → `…/OthrId` com `Tp/Prtry` e espelhado em
  `SupplementaryData`. Nunca substitua identificadores regulamentados por identificadores proprietários.

#### Preferência de identificador por família de mensagens

##### `sese.023` / `.024` / `.025` (liquidação de títulos)

- **Instrumento (`FinInstrmId`)**
  - Preferencial: **ISIN** sob `…/ISIN`. É o identificador canônico para CSDs/T2S.[^anna]
  - Subsídios:
    - **CUSIP** ou outro NSIN sob `…/OthrId/Id` com `Tp/Cd` definido a partir do ISO externo
      lista de códigos (por exemplo, `CUSP`); incluir o emissor em `Issr` quando obrigatório.[^iso_mdr]
    - **ID do ativo Norito** como proprietário: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` e
      registre o mesmo valor em `SupplementaryData`.
  - Descritores opcionais: **CFI** (`ClssfctnTp`) e **FISN** quando suportados para facilitar
    reconciliação.[^iso_cfi][^iso_fisn]
- **Partes (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Preferencial: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback: **LEI** onde a versão da mensagem expõe um campo LEI dedicado; se
    ausente, carregue IDs proprietários com rótulos `Prtry` claros e inclua BIC nos metadados.[^iso_cr]
- **Local de liquidação/local** → **MIC** para o local e **BIC** para o CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` e `colr.007` (gestão de garantias)

- Siga as mesmas regras do instrumento `sese.*` (preferencialmente ISIN).
- As partes usam **BIC** por padrão; **LEI** é aceitável onde o esquema o expõe.[^swift_bic]
- Os valores em dinheiro devem usar códigos de moeda **ISO 4217** com unidades menores corretas.[^iso_4217]

##### `pacs.009` / `camt.054` (financiamento e declarações PvP)

- **Agentes (`InstgAgt`, `InstdAgt`, agentes devedores/credores)** → **BIC** com opcional
  LEI onde permitido.[^swift_bic]
- **Contas**
  - Interbancário: identificar por **BIC** e referências de contas internas.
  - Declarações voltadas para o cliente (`camt.054`): inclua **IBAN** quando presente e valide-o
    (comprimento, regras do país, soma de verificação mod-97).[^swift_iban]
- **Moeda** → **ISO 4217** Código de três letras, respeitando arredondamentos de unidades menores.[^iso_4217]
- **Ingestão de Torii** → Enviar trechos de financiamento PvP via `POST /v1/iso20022/pacs009`; a ponte
  requer `Purp=SECU` e agora impõe faixas de pedestres BIC quando os dados de referência são configurados.

#### Regras de validação (aplicadas antes da emissão)| Identificador | Regra de validação | Notas |
|------------|-----------------|-------|
| **ISIN** | Dígito de verificação Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` e Luhn (mod-10) de acordo com ISO 6166 Anexo C | Rejeite antes da emissão da ponte; prefiro enriquecimento upstream.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` e modulus-10 com 2 ponderações (mapeamento de caracteres para dígitos) | Somente quando o ISIN não estiver disponível; mapa via faixa de pedestres ANNA/CUSIP uma vez obtido.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` e dígito de verificação mod-97 (ISO 17442) | Valide em relação aos arquivos delta diários da GLEIF antes da aceitação.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Código de ramificação opcional (últimos três caracteres). Confirme o status ativo nos arquivos RA.[^swift_bic] |
| **MIC** | Manter a partir do arquivo ISO 10383 RA; garantir que os locais estejam ativos (sem sinalizador de terminação `!`) | Sinaliza MICs desativados antes da emissão.[^iso_mic] |
| **IBAN** | Comprimento específico do país, alfanumérico maiúsculo, mod-97 = 1 | Utilizar registro mantido pela SWIFT; rejeitar IBANs estruturalmente inválidos.[^swift_iban] |
| **IDs de conta/parte proprietárias** | `Max35Text` (UTF-8, ≤35 caracteres) com espaço em branco cortado | Aplica-se aos campos `GenericAccountIdentification1.Id` e `PartyIdentification135.Othr/Id`. Rejeite entradas que excedam 35 caracteres para que as cargas úteis da ponte estejam em conformidade com os esquemas ISO. |
| **Identificadores de conta proxy** | `Max2048Text` não vazio em `…/Prxy/Id` com códigos de tipo opcionais em `…/Prxy/Tp/{Cd,Prtry}` | Armazenado junto com o IBAN principal; a validação ainda requer IBANs ao aceitar identificadores de proxy (com códigos de tipo opcionais) para espelhar trilhos PvP. |
| **Finanças** | Código de seis caracteres, letras maiúsculas usando a taxonomia ISO 10962 | Enriquecimento opcional; certifique-se de que os caracteres correspondam à classe do instrumento.[^iso_cfi] |
| **FISN** | Até 35 caracteres, alfanuméricos maiúsculos e pontuação limitada | Opcional; truncar/normalizar de acordo com a orientação ISO 18774.[^iso_fisn] |
| **Moeda** | Código ISO 4217 de três letras, escala determinada por unidades menores | Os valores devem ser arredondados para casas decimais permitidas; impor no lado Norito.[^iso_4217] |

#### Obrigações de faixa de pedestres e manutenção de dados- Manter **ISIN ↔ Norito ID de ativo** e faixas de pedestres **CUSIP ↔ ISIN**. Atualizar todas as noites de
  Os feeds ANNA/DSB e a versão controlam os instantâneos usados pelo CI.[^anna_crosswalk]
- Atualize os mapeamentos **BIC ↔ LEI** dos arquivos de relacionamento público da GLEIF para que a ponte possa
  emita ambos quando necessário.[^bic_lei]
- Armazene **definições de MIC** junto com os metadados da ponte para que a validação do local seja
  determinístico mesmo quando os arquivos RA mudam ao meio-dia.[^iso_mic]
- Registrar a proveniência dos dados (timestamp + fonte) em metadados de ponte para auditoria. Persista o
  identificador de instantâneo junto com as instruções emitidas.
- Configure `iso_bridge.reference_data.cache_dir` para persistir uma cópia de cada conjunto de dados carregado
  juntamente com metadados de proveniência (versão, origem, carimbo de data/hora, soma de verificação). Isso permite que os auditores
  e operadores para diferenciar feeds históricos mesmo após a rotação dos snapshots upstream.
- Instantâneos de faixa de pedestres ISO são ingeridos por `iroha_core::iso_bridge::reference_data` usando
  o bloco de configuração `iso_bridge.reference_data` (caminhos + intervalo de atualização). Medidores
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` e
  `iso_reference_refresh_interval_secs` expõe a integridade do tempo de execução para alertas. O Torii
  bridge rejeita envios `pacs.008` cujos BICs de agente estão ausentes do configurado
  faixa de pedestres, revelando erros determinísticos `InvalidIdentifier` quando uma contraparte é
  desconhecido.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- As vinculações IBAN e ISO 4217 são aplicadas na mesma camada: pacs.008/pacs.009 flui agora
  emitir erros `InvalidIdentifier` quando os IBANs do devedor/credor não possuem aliases configurados ou quando
  a moeda de liquidação está faltando em `currency_assets`, evitando ponte malformada
  instruções cheguem ao livro-razão. A validação do IBAN também se aplica a dados específicos do país
  comprimentos e dígitos de verificação numéricos antes da aprovação ISO 7064 mod-97 são tão estruturalmente inválidos
  os valores são rejeitados antecipadamente.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- Os ajudantes de liquidação CLI herdam os mesmos guarda-corpos: passe
  `--iso-reference-crosswalk <path>` junto com `--delivery-instrument-id` para ter o DvP
  visualize os IDs dos instrumentos antes de emitir o instantâneo XML `sese.023`.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (e o wrapper CI `ci/check_iso_reference_data.sh`) fiapos
  instantâneos e luminárias da faixa de pedestres. O comando aceita `--isin`, `--bic-lei`, `--mic` e
  `--fixtures` sinaliza e retorna aos conjuntos de dados de amostra em `fixtures/iso_bridge/` quando executado
  sem argumentos.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- O auxiliar IVM agora ingere envelopes XML ISO 20022 reais (head.001 + `DataPDU` + `Document`)
  e valida o cabeçalho do aplicativo de negócios por meio do esquema `head.001` para `BizMsgIdr`,
  Os agentes `MsgDefIdr`, `CreDt` e BIC/ClrSysMmbId são preservados deterministicamente; XMLDSig/XAdES
  os blocos permanecem ignorados intencionalmente. 

#### Considerações regulatórias e de estrutura de mercado

- **Liquidação T+1**: os mercados acionários dos EUA/Canadá passaram para D+1 em 2024; ajustar Norito
  agendamento e alertas de SLA de acordo.[^sec_t1][^csa_t1]
- **Penalidades CSDR**: As regras de disciplina de liquidação impõem penalidades em dinheiro; garantir Norito
  metadados capturam referências de penalidade para reconciliação.[^csdr]
- **Pilotos de liquidação no mesmo dia**: o regulador da Índia está implementando faseadamente a liquidação T0/T+0; manter
  calendários de pontes atualizados à medida que os pilotos se expandem.[^india_t0]
- **Compras/retenções de garantias**: Monitore as atualizações da ESMA sobre cronogramas de recompra e retenções opcionais
  portanto, a entrega condicional (`HldInd`) está alinhada com as orientações mais recentes.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Entrega versus Pagamento → `sese.023`| Campo DvP | Caminho ISO 20022 | Notas |
|--------------------------------------------------------|----------------------------------------|-------|
| `settlement_id` | `TxId` | Identificador de ciclo de vida estável |
| `delivery_leg.asset_definition_id` (segurança) | `SctiesLeg/FinInstrmId` | Identificador canónico (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Sequência decimal; homenageia precisão de ativos |
| `payment_leg.asset_definition_id` (moeda) | `CashLeg/Ccy` | Código monetário ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | Sequência decimal; arredondado por especificação numérica |
| `delivery_leg.from` (vendedor/entregador) | `DlvrgSttlmPties/Pty/Bic` | BIC do participante entregador *(o ID canônico da conta é atualmente exportado em metadados)* |
| Identificador de conta `delivery_leg.from` | `DlvrgSttlmPties/Acct` | Formato livre; Os metadados Norito carregam o ID exato da conta |
| `delivery_leg.to` (comprador/recebedor) | `RcvgSttlmPties/Pty/Bic` | BIC do participante receptor |
| Identificador de conta `delivery_leg.to` | `RcvgSttlmPties/Acct` | Formato livre; corresponde ao ID da conta receptora |
| `plan.order` | `Plan/ExecutionOrder` | Enum: `DELIVERY_THEN_PAYMENT` ou `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Enum: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Finalidade da mensagem** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (entregar) ou `RECE` (receber); espelhos que perna a parte requerente executa. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (contra pagamento) ou `FREE` (sem pagamento). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | JSON Norito opcional codificado como UTF‑8 |

> **Qualificadores de liquidação** – a ponte reflete a prática de mercado, copiando códigos de condição de liquidação (`SttlmTxCond`), indicadores de liquidação parcial (`PrtlSttlmInd`) e outros qualificadores opcionais dos metadados Norito para `sese.023/025`, quando presentes. Aplique as enumerações publicadas nas listas de códigos externos ISO para que o CSD de destino reconheça os valores.

### Financiamento Pagamento versus Pagamento → `pacs.009`

As pernas dinheiro por dinheiro que financiam uma instrução PvP são emitidas como crédito FI-to-FI
transferências. A ponte anota esses pagamentos para que os sistemas downstream reconheçam
eles financiam uma liquidação de títulos.

| Campo de financiamento PvP | Caminho ISO 20022 | Notas |
|------------------------------------------------|-----------------------------------------------------|-------|
| `primary_leg.quantity` / {valor, moeda} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Valor/moeda debitado do iniciador. |
| Identificadores do agente da contraparte | `InstgAgt`, `InstdAgt` | BIC/LEI dos agentes emissores e receptores. |
| Finalidade da liquidação | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Defina como `SECU` para financiamento PvP relacionado a títulos. |
| Metadados Norito (IDs de conta, dados FX) | `CdtTrfTxInf/SplmtryData` | Carrega AccountId completo, carimbos de data/hora FX, dicas de plano de execução. |
| Identificador de instrução/vinculação do ciclo de vida | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Corresponde ao Norito `settlement_id` para que o lado do dinheiro se reconcilie com o lado dos títulos. |

A ponte ISO do SDK JavaScript se alinha com esse requisito padronizando o
Finalidade da categoria `pacs.009` para `SECU`; os chamadores podem substituí-lo por outro
código ISO válido ao emitir transferências de crédito não relacionadas a valores mobiliários, mas inválido
os valores são rejeitados antecipadamente.

Se uma infraestrutura exigir uma confirmação explícita de títulos, a ponte
continua a emitir `sese.025`, mas essa confirmação reflete a perna dos títulos
status (por exemplo, `ConfSts = ACCP`) em vez do “objetivo” do PvP.

### Confirmação de pagamento versus pagamento → `sese.025`| Campo PvP | Caminho ISO 20022 | Notas |
|----------------------------------------------------------|---------------------------|-------|
| `settlement_id` | `TxId` | Identificador de ciclo de vida estável |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Código da moeda do trecho primário |
| `primary_leg.quantity` | `SttlmAmt` | Montante entregue pelo iniciador |
| `counter_leg.asset_definition_id` | `AddtlInf` (carga útil JSON) | Código da moeda do contador incorporado em informações suplementares |
| `counter_leg.quantity` | `SttlmQty` | Montante do contador |
| `plan.order` | `Plan/ExecutionOrder` | Mesma enumeração definida como DvP |
| `plan.atomicity` | `Plan/Atomicity` | Mesma enumeração definida como DvP |
| Status `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` quando combinado; ponte emite códigos de falha na rejeição |
| Identificadores de contraparte | `AddtlInf` JSON | A ponte atual serializa tuplas completas de AccountId/BIC em metadados |

### Substituição de garantia de recompra → `colr.007`

| Campo / contexto do repositório | Caminho ISO 20022 | Notas |
|-------------------------------------------------|-----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Identificador do contrato de recompra |
| Substituição de garantias Identificador Tx | `TxId` | Gerado por substituição |
| Quantidade de garantia inicial | `Substitution/OriginalAmt` | Jogos prometeram garantias antes da substituição |
| Moeda de garantia original | `Substitution/OriginalCcy` | Código da moeda |
| Quantidade de garantia substituta | `Substitution/SubstituteAmt` | Montante de substituição |
| Moeda de garantia substituta | `Substitution/SubstituteCcy` | Código da moeda |
| Data efectiva (calendário da margem de governação) | `Substitution/EffectiveDt` | Data ISO (AAAA-MM-DD) |
| Classificação de corte de cabelo | `Substitution/Type` | Atualmente `FULL` ou `PARTIAL` com base na política de governança |
| Razão de governança / nota de corte de cabelo | `Substitution/ReasonCd` | Opcional, traz justificativa de governança |

### Financiamento e Demonstrativos

| Contexto Iroha | Mensagem ISO 20022 | Mapeamento de localização |
|----------------------------------|-------------------|------------------|
| Ignição / desenrolamento da perna de dinheiro Repo | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` preenchido a partir de pernas DvP/PvP |
| Declarações pós-liquidação | `camt.054` | Movimentos da perna de pagamento registados em `Ntfctn/Ntry[*]`; bridge injeta metadados de razão/conta em `SplmtryData` |

### Notas de uso

* Todos os valores são serializados usando os auxiliares numéricos Norito (`NumericSpec`)
  para garantir a conformidade de escala nas definições de ativos.
* Os valores `TxId` são `Max35Text` — impõe comprimento UTF-8 ≤35 caracteres antes
  exportando para mensagens ISO 20022.
* Os BICs devem ter 8 ou 11 caracteres alfanuméricos maiúsculos (ISO9362); rejeitar
  Metadados Norito que falham nesta verificação antes de emitir pagamentos ou liquidações
  confirmações.
* Os identificadores de conta (AccountId/ChainId) são exportados para suplementares
  metadados para que os participantes receptores possam reconciliar-se com seus registros locais.
* `SupplementaryData` deve ser JSON canônico (UTF-8, chaves classificadas, nativo de JSON
  escapando). Os ajudantes do SDK aplicam isso para que assinaturas, hashes de telemetria e ISO
  os arquivos de carga útil permanecem determinísticos durante as reconstruções.
* Os valores das moedas seguem dígitos fracionários ISO4217 (por exemplo, JPY tem 0
  decimais, USD tem 2); a ponte fixa a precisão numérica Norito de acordo.
* Os auxiliares de liquidação CLI (`iroha app settlement ... --atomicity ...`) agora emitem
  Instruções Norito cujos planos de execução mapeiam 1:1 para `Plan/ExecutionOrder` e
  `Plan/Atomicity` acima.
* O auxiliar ISO (`ivm::iso20022`) valida os campos listados acima e rejeita
  mensagens onde trechos DvP/PvP violam especificações numéricas ou reciprocidade da contraparte.

### Ajudantes do SDK Builder- O JavaScript SDK agora expõe `buildPacs008Message`/
  `buildPacs009Message` (ver `javascript/iroha_js/src/isoBridge.js`) então cliente
  automação pode converter metadados de liquidação estruturados (BIC/LEI, IBANs,
  códigos de finalidade, campos Norito suplementares) em pacs XML determinísticos
  sem reimplementar as regras de mapeamento deste guia.
- Ambos os auxiliares exigem um `creationDateTime` explícito (ISO‑8601 com fuso horário)
  portanto, os operadores devem encadear um carimbo de data/hora determinístico de seu fluxo de trabalho
  de deixar o SDK usar como padrão o horário do relógio de parede.
- `recipes/iso_bridge_builder.mjs` demonstra como conectar esses auxiliares
  uma CLI que mescla variáveis de ambiente ou arquivos de configuração JSON, imprime o
  XML gerado e, opcionalmente, envia-o para Torii (`ISO_SUBMIT=1`), reutilizando
  a mesma cadência de espera da receita da ponte ISO.


### Referências

- Exemplos de liquidação LuxCSD/Clearstream ISO 20022 mostrando `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) e `Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Especificações Clearstream DCP que abrangem qualificadores de liquidação (`SttlmTxCond`, `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- Orientação SWIFT PMPG recomendando `pacs.009` com `CtgyPurp/Cd = SECU` para financiamento PvP relacionado a títulos.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- Relatórios de definição de mensagens ISO 20022 para restrições de comprimento de identificador (BIC, Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- Orientações da ANNA DSB sobre formato ISIN e regras de soma de verificação.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### Dicas de uso

- Sempre cole o snippet Norito relevante ou o comando CLI para que o LLM possa inspecionar
  nomes exatos de campos e escalas numéricas.
- Solicite citações (`provide clause references`) para manter um registro de documentos para
  conformidade e revisão do auditor.
- Capture o resumo da resposta em `docs/source/finance/settlement_iso_mapping.md`
  (ou apêndices vinculados) para que futuros engenheiros não precisem repetir a consulta.

## Manual de ordenação de eventos (ISO 20022 ↔ Norito Bridge)

### Cenário A — Substituição de Garantia (Repo/Pledge)

**Participantes:** doador/recebedor de garantia (e/ou agentes), custodiante(s), CSD/T2S  
**Tempo:** por pontos de corte de mercado e ciclos dia/noite T2S; orquestrar as duas etapas para que sejam concluídas na mesma janela de liquidação.

#### Coreografia de mensagens
1. `colr.010` Solicitação de Substituição de Garantia → doador/recebedor ou agente da garantia.  
2. `colr.011` Resposta de substituição de garantia → aceitar/rejeitar (motivo de rejeição opcional).  
3. `colr.012` Confirmação de Substituição de Garantia → confirma o acordo de substituição.  
4. Instruções `sese.023` (duas pernas):  
   - Devolver a garantia original (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Entregar garantia substituta (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Vincule o par (veja abaixo).  
5. Avisos de status `sese.024` (aceito, correspondido, pendente, com falha, rejeitado).  
6. Confirmações `sese.025` uma vez reservadas.  
7. Delta de caixa opcional (taxas/haircut) → `pacs.009` Transferência de crédito FI para FI com `CtgyPurp/Cd = SECU`; status via `pacs.002`, retorna via `pacs.004`.

#### Agradecimentos/status necessários
- Nível de transporte: os gateways podem emitir `admi.007` ou rejeitar antes do processamento do negócio.  
- Ciclo de vida de liquidação: `sese.024` (status de processamento + códigos de motivo), `sese.025` (final).  
- Lado do caixa: `pacs.002` (`PDNG`, `ACSC`, `RJCT` etc.), `pacs.004` para devoluções.

#### Campos de condicionalidade/desenrolamento
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) para encadear as duas instruções.  
- `SttlmParams/HldInd` para manter até que os critérios sejam atendidos; liberação via `sese.030` (status `sese.031`).  
- `SttlmParams/PrtlSttlmInd` para controle de liquidação parcial (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` para condições específicas de mercado (`NOMC`, etc.).  
- Regras opcionais de entrega condicional de títulos do T2S (CoSD), quando suportadas.

#### Referências
- MDR de gestão de garantias SWIFT (`colr.010/011/012`).  
- Guias de utilização de CSD/T2S (por exemplo, DNB, ECB Insights) para ligações e estados.  
- Prática de liquidação SMPG, manuais Clearstream DCP, workshops ASX ISO.

### Cenário B — Violação da janela FX (falha no financiamento PvP)

**Participantes:** contrapartes e agentes de numerário, custodiante de títulos, CSD/T2S  
**Tempo:** Janelas FX PvP (CLS/bilateral) e limites de CSD; manter as pernas dos títulos em espera enquanto se aguarda a confirmação do dinheiro.#### Coreografia de mensagens
1. `pacs.009` Transferência de Crédito FI-FI por moeda com `CtgyPurp/Cd = SECU`; status via `pacs.002`; recuperar/cancelar via `camt.056`/`camt.029`; se já liquidado, `pacs.004` retorna.  
2. Instrução(ões) DvP `sese.023` com `HldInd=true` para que a perna dos títulos aguarde a confirmação do dinheiro.  
3. Avisos de ciclo de vida `sese.024` (aceitos/correspondidos/pendentes).  
4. Se ambas as pernas `pacs.009` atingirem `ACSC` antes que a janela expire → libere com `sese.030` → `sese.031` (status do mod) → `sese.025` (confirmação).  
5. Se a janela FX for violada → cancelar/revogar dinheiro (`camt.056/029` ou `pacs.004`) e cancelar títulos (`sese.020` + `sese.027`, ou reversão `sese.026` se já confirmado por regra de mercado).

#### Agradecimentos/status necessários
- Dinheiro: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` para devoluções.  
- Títulos: `sese.024` (motivos pendentes/falha como `NORE`, `ADEA`), `sese.025`.  
- Transporte: `admi.007` / gateway rejeita antes do processamento do negócio.

#### Campos de condicionalidade/desenrolamento
- `SttlmParams/HldInd` + `sese.030` liberação/cancelamento em caso de sucesso/falha.  
- `Lnkgs` para vincular instruções de títulos à perna de caixa.  
- Regra T2S CoSD se usar entrega condicional.  
- `PrtlSttlmInd` para evitar parciais não intencionais.  
- Em `pacs.009`, `CtgyPurp/Cd = SECU` sinaliza financiamento relacionado a títulos.

#### Referências
- Orientação PMPG/CBPR+ para pagamentos em processos de valores mobiliários.  
- Práticas de liquidação SMPG, insights do T2S sobre vinculações/retenções.  
- Manuais Clearstream DCP, documentação ECMS para mensagens de manutenção.

### pacs.004 notas de mapeamento de retorno

- Os fixtures de retorno agora normalizam `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) e motivos de devolução proprietários expostos como `TxInf[*]/RtrdRsn/Prtry`, para que os consumidores de ponte possam reproduzir atribuição de taxas e códigos de operador sem analisar novamente o XML envelope.
- Os blocos de assinatura AppHdr dentro dos envelopes `DataPDU` permanecem ignorados na ingestão; as auditorias devem basear-se na proveniência do canal em vez de campos XMLDSIG incorporados.

### Lista de verificação operacional para a ponte
- Aplicar a coreografia acima (garantia: `colr.010/011/012 → sese.023/024/025`; violação de câmbio: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- Trate os status `sese.024`/`sese.025` e os resultados `pacs.002` como sinais de ativação; `ACSC` aciona a liberação, `RJCT` força o desenrolar.  
- Codifique a entrega condicional via `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` e regras CoSD opcionais.  
- Use `SupplementaryData` para correlacionar IDs externos (por exemplo, UETR para `pacs.009`) quando necessário.  
- Parametrizar o timing de hold/unwind por calendário/cut-offs do mercado; emitir `sese.030`/`camt.056` antes dos prazos de cancelamento, recorrer a devoluções quando necessário.

### Exemplo de cargas úteis ISO 20022 (anotado)

#### Par de substituição colateral (`sese.023`) com ligação de instrução

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Envie a instrução vinculada `SUBST-2025-04-001-B` (recebimento FoP de garantia substituta) com `SctiesMvmntTp=RECE`, `Pmt=FREE` e a ligação `WITH` apontando de volta para `SUBST-2025-04-001-A`. Solte ambas as pernas com um `sese.030` correspondente assim que a substituição for aprovada.

#### Perna de títulos em espera aguardando confirmação de câmbio (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Solte quando ambas as pernas `pacs.009` alcançarem `ACSC`:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` confirma a liberação de retenção, seguido por `sese.025` assim que a perna de títulos for registrada.

#### Perna de financiamento PvP (`pacs.009` com finalidade de valores mobiliários)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` rastreia o status do pagamento (`ACSC` = confirmado, `RJCT` = rejeitado). Se a janela for violada, recupere via `camt.056`/`camt.029` ou envie `pacs.004` para devolver os fundos liquidados.