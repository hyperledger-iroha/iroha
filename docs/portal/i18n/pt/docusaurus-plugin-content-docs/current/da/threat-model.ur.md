---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronização رکھیں۔
:::

# Modelo de ameaça à disponibilidade de dados Sora Nexus

_آخری جائزہ: 2026-01-19 -- اگلا شیڈول شدہ جائزہ: 2026-04-19_

Cadência de manutenção: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). ہر ریویژن
`status.md` میں لازمی درج ہو، اور اس میں tíquetes de mitigação ativos
artefatos de simulação

## مقصد اور دائرہ کار

Disponibilidade de dados (DA) para transmissões Taikai, blobs de pista Nexus e governança
artefatos bizantinos, نیٹ ورک, اور آپریٹر کی خرابیوں میں بھی قابل بازیافت
رکھتا ہے۔ یہ modelo de ameaça DA-1 (arquitetura e modelo de ameaça) کیلئے انجینئرنگ
کام کی بنیاد ہے اور tarefas DA downstream (DA-2 ou DA-10) کیلئے linha de base ہے۔

Componentes no escopo:
- Extensão de ingestão Torii DA e gravadores de metadados Norito.
- Árvores de armazenamento de blob apoiadas por SoraFS (camadas quentes/frias) e políticas de replicação.
- Compromissos de bloco Nexus (formatos de transmissão, provas, APIs de cliente leve).
- Ganchos de aplicação de PDP/PoTR e cargas úteis de DA کیلئے مخصوص ہیں.
- Fluxos de trabalho do operador (fixação, despejo, corte) e pipelines de observabilidade.
- Aprovações de governança جو Operadores DA اور conteúdo کو admitir یا despejar کرتے ہیں۔

Fora do escopo deste documento:
- Modelo de modelagem econômica (fluxo de trabalho DA-7).
- Protocolos base SoraFS جو پہلے سے Modelo de ameaça SoraFS میں شامل ہیں.
- Ergonomia do Client SDK além das considerações de superfície de ameaça.

## Visão Geral da Arquitetura

1. **Envio:** Blobs de clientes کو Torii DA ingest API کے ذریعے submit کرتے ہیں۔
   Blobs de nó کو pedaços میں بانٹتا ہے, Norito manifesta codificação codificada کرتا ہے (tipo de blob,
   faixa, época, sinalizadores de codec) ، اور pedaços کو camada SoraFS quente میں محفوظ کرتا ہے۔
2. **Anúncio:** Pin intents e registro de dicas de replicação (mercado SoraFS)
   کے ذریعے provedores de armazenamento تک جاتے ہیں, tags de política کے ساتھ جو retenção quente/frio
   alvos
3. **Compromisso:** Compromissos de blob de sequenciadores Nexus (CID + raízes KZG opcionais)
   bloco canônico میں شامل کرتے ہیں۔ Hash de compromisso de clientes leves anunciado
   metadados کی بنیاد پر verificação de disponibilidade کرتے ہیں۔
4. **Replicação:** Nós de armazenamento atribuídos a compartilhamentos/pedaços کھینچتے ہیں, PDP/PoTR
   desafios مکمل کرتے ہیں، اور política کے مطابق camadas quentes/frias کے درمیان dados
   promover کرتے ہیں۔
5. **Busca:** Consumidores SoraFS یا Gateways com reconhecimento de DA کے ذریعے busca de dados کرتے ہیں،
   verificações de provas کرتے ہیں، اور réplicas غائب ہونے پر solicitações de reparo اٹھاتے ہیں۔
6. **Governança:** Parlamento e operadores do comitê de supervisão do DA, cronogramas de aluguel,
   اور escalações de aplicação کو aprovar کرتی ہے۔ Artefatos de governança اسی DA
   caminho سے گزرتے ہیں تاکہ transparência برقرار رہے۔

## Ativos e Proprietários

Escala de impacto: **Crítico** segurança/vivacidade do razão توڑتا ہے؛ **Alto** preenchimento de DA
یا clientes کو بلاک کرتا ہے؛ **Moderada** qualidade کم کرتا ہے مگر recuperável؛
**Baixo** محدود اثر۔

| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Proprietário |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Taikai, pista, bolhas de governança em SoraFS | Crítico | Crítico | Moderado | DA WG / Equipe de Armazenamento |
| Manifestos DA Norito | Metadados digitados que descrevem blobs | Crítico | Alto | Moderado | GT de Protocolo Central |
| Bloquear compromissos | CIDs + raízes KZG dentro de blocos Nexus | Crítico | Alto | Baixo | GT de Protocolo Central |
| Cronogramas PDP/PoTR | Cadência de aplicação para réplicas DA | Alto | Alto | Baixo | Equipe de armazenamento |
| Cadastro de operadores | Provedores e políticas de armazenamento aprovados | Alto | Alto | Baixo | Conselho de Governança |
| Registros de aluguel e incentivos | Lançamentos contábeis para aluguel e multas de DA | Alto | Moderado | Baixo | GT Tesouraria |
| Painéis de observabilidade | SLOs DA, profundidade de replicação, alertas | Moderado | Alto | Baixo | SRE / Observabilidade |
| Intenções de reparação | Pedidos para reidratar pedaços perdidos | Moderado | Moderado | Baixo | Equipe de armazenamento |

## Adversários e Capacidades| Ator | Capacidades | Motivações | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Blobs malformados enviam کرنا, manifestos obsoletos, replay کرنا, ingerem پر DoS کی کوشش۔ | Transmissões Taikai interrompem کرنا۔, injeção de dados inválidos کرنا۔ | Chaves privilegiadas |
| Nó de armazenamento bizantino | Réplicas atribuídas são descartadas, provas PDP/PoTR forjadas, conspiradas | Retenção de DA کم کرنا، aluguel سے بچنا، refém de dados بنانا۔ | Credenciais de operador válidas |
| Sequenciador comprometido | Compromissos omitem blocos, equivocam, reordenam metadados, | Submissões DA چھپانا، inconsistência پیدا کرنا۔ | Maioria de consenso |
| Operador interno | Abuso de acesso de governança کرنا, adulteração de políticas de retenção کرنا, vazamento de credenciais کرنا۔ | Ganho econômico, sabotagem۔ | Infraestrutura de nível quente/frio تک رسائی۔ |
| Adversário da rede | Partição de nós کرنا, atraso de replicação کرنا, injeção de tráfego MITM کرنا۔ | Disponibilidade کم کرنا۔, SLOs degradam کرنا۔ | TLS نہیں توڑ سکتا مگر links lentos/quedas کر سکتا ہے۔ |
| Atacante de observabilidade | Painéis/alertas adulteram کرنا۔, incidentes suprimem کرنا۔ | Interrupções no DA | Pipeline de telemetria |

## Limites de confiança

- **Limite de entrada:** Cliente سے Torii extensão DA۔ Autenticação em nível de solicitação, limitação de taxa
  Para validação de carga útil
- **Limite de replicação:** Pedaços de nós de armazenamento e troca de provas کرتے ہیں۔ Nós
  باہمی autenticado ہیں مگر Bizantino برتاؤ ممکن ہے۔
- **Limite do razão:** Dados de bloco confirmados بمقابلہ armazenamento fora da cadeia۔ Consenso
  guarda de integridade کرتا ہے، مگر disponibilidade کیلئے aplicação fora da cadeia ضروری ہے۔
- **Limite de governança:** Conselho/Parlamento کے فیصلے operadores, orçamentos, cortes
  aprovar کرتے ہیں۔ یہاں خرابی Implantação DA کو براہ راست متاثر کرتی ہے۔
- **Limite de observabilidade:** Coleta de métricas/logs e ferramentas de painéis/alertas
  کو exportar ہونا۔ Interrupções de adulteração یا ataques چھپا سکتا ہے۔

## Cenários e controles de ameaças

### Ingerir ataques de caminho

**Cenário:** cargas úteis Norito de cliente malicioso malformadas e envio de blobs grandes
کرتا ہے تاکہ esgotamento de recursos ہوں یا metadados inválidos شامل ہو۔

**Controles**
- Validação de esquema Norito com negociação rigorosa de versão; bandeiras desconhecidas rejeitadas۔
- Endpoint de ingestão Torii com limitação de taxa e autenticação
- SoraFS chunker کے ذریعے limites de tamanho de pedaço e codificação determinística۔
- Pipeline de admissão صرف correspondência de soma de verificação de integridade ہونے کے بعد os manifestos persistem کرے۔
- Cache de repetição determinístico (`ReplayCache`) `(lane, epoch, sequence)` trilha do Windows
  کرتا ہے, disco com marcas d’água altas پر persistir کرتا ہے, اور duplicatas/replays obsoletos
  rejeitar کرتا ہے؛ propriedade Fuzz aproveita impressões digitais divergentes fora de ordem
  capa de submissões کرتے ہیں۔ [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuais**
- Torii ingest کو admissão de cache de repetição میں thread کرنا اور cursores de sequência
  reinicia کے پار persist کرنا ضروری ہے۔
- Esquemas DA Norito کے لئے chicote fuzz dedicado (`fuzz/da_ingest_schema.rs`) موجود
  ہے؛ painéis de cobertura کو regressão پر alerta کرنا چاہئے۔

### Retenção de replicação

**Cenário:** Atribuições de pinos de operadores de armazenamento bizantinos قبول کرتے ہیں مگر pedaços





descartar کرتے ہیں، respostas forjadas یا conluio سے Desafios PDP/PoTR aprovados کرتے ہیں۔

**Controles**
- Cronograma de desafio PDP / PoTR Cargas úteis DA تک estender ہے اور cobertura por época دیتا ہے۔
- Replicação de várias fontes com limites de quorum; buscar fragmentos ausentes do orquestrador
  detectar o gatilho de reparo کرتا ہے۔
- Governança reduzindo provas falhas e réplicas ausentes vinculadas ہے۔
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) ingere recibos
  کو compromissos DA (SignedBlockWire/`.norito`/JSON) سے comparar کرتا ہے، governança
  کیلئے pacote de evidências JSON emit کرتا ہے، اور tickets ausentes/incompatíveis پر falhar
  ہو کر Alertmanager کو página کرنے دیتا ہے۔

**Lacunas residuais**
- Chicote de simulação `integration_tests/src/da/pdp_potr.rs` کا (testes:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) conluio e partição
  cenários DA-5 کے ساتھ اسے نئی superfícies de prova کیلئے مزید بڑھائیں۔
- Política de despejo de nível frio کیلئے trilha de auditoria assinada درکار ہے تاکہ quedas secretas
  روکے جا سکیں۔

### Adulteração de compromisso

**Cenário:** Compromissos DA do sequenciador comprometidos omitem یا alter کرتا ہے، جس سے
falhas de busca یا inconsistências de cliente leve پیدا ہوتی ہیں۔**Controles**
- Propostas de bloco de consenso کو Filas de envio de DA سے verificação cruzada کرتا ہے؛ pares
  compromissos faltantes والی propostas rejeitadas کرتے ہیں۔
- Provas de inclusão de clientes leves verificam کرتے ہیں قبل از buscar identificadores۔
- Recibos de envio e compromissos de bloco e trilha de auditoria۔
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) ingere recibos
  کو compromissos سے comparar کرتا ہے، governança کیلئے pacote de evidências JSON emitir کرتا ہے،
  اور tickets faltantes/incompatíveis پر página Alertmanager ہوتا ہے۔

**Lacunas residuais**
- Trabalho de reconciliação + gancho Alertmanager سے capa؛ pacotes de governança padrão میں
  Ingestão de pacote de evidências JSON

### Partição de rede e censura

**Cenário:** Partição de rede de replicação adversária کرتا ہے، جس سے nós atribuídos
pedaços حاصل نہیں کر پاتے یا Desafios PDP/PoTR کا جواب نہیں دے پاتے۔

**Controles**
- O provedor multirregional exige diversos caminhos de rede یقینی بناتے ہیں۔
- Janelas de desafio com jitter e fallback de canal de reparo fora de banda شامل ہے۔
- Profundidade de replicação dos painéis de observabilidade, desafio ao sucesso, latência de busca e
  limites de alerta کے ساتھ monitor کرتے ہیں۔

**Lacunas residuais**
- Eventos ao vivo Taikai کیلئے simulações de partição ابھی نہیں؛ testes de imersão
- Política de reserva de largura de banda de reparo ابھی codificada نہیں۔

### Abuso interno

**Cenário:** Acesso ao registro e políticas de retenção do operador manipulam کرتا ہے،
provedores maliciosos کو whitelist کرتا ہے، یا alertas suprimir کرتا ہے۔

**Controles**
- Ações de governança, assinaturas multipartidárias اور Norito registros autenticados مانگتی ہیں۔
- Monitoramento de alterações de política e registros de arquivo e eventos بھیجتی ہیں۔
- Logs Norito somente anexados ao pipeline de observabilidade com aplicação de encadeamento de hash کرتا ہے۔
- Manifesto/reprodução de automação de revisão de acesso trimestral (`cargo xtask da-privilege-audit`)
  dirs (caminhos fornecidos pelo operador) scan کرتا ہے, ausente/não-diretório/gravável mundialmente
  sinalizador de entradas کرتا ہے، اور painéis de pacote JSON assinados کیلئے emit کرتا ہے۔

**Lacunas residuais**
- Evidência de violação do painel کیلئے instantâneos assinados درکار ہیں۔

## Registro de Risco Residual

| Risco | Probabilidade | Impacto | Proprietário | Plano de Mitigação |
| --- | --- | --- | --- | --- |
| Cache de sequência DA-2 سے پہلے Reprodução de manifestos DA | Possível | Moderado | GT de Protocolo Central | DA-2 میں cache de sequência + implementação de validação de nonce کریں؛ testes de regressão |
| >f nós comprometem o conluio PDP/PoTR | Improvável | Alto | Equipe de armazenamento | Amostragem entre provedores کے ساتھ نیا cronograma de desafio deriva کریں؛ chicote de simulação سے validar کریں۔ |
| Lacuna na auditoria de despejo nas camadas frias | Possível | Alto | Equipe SRE/Armazenamento | Despejos کیلئے registros de auditoria assinados + recibos na rede anexados کریں؛ painéis de controle ou monitor کریں۔ |
| Latência de detecção de omissão do sequenciador | Possível | Alto | GT de Protocolo Central | Recibos noturnos `cargo xtask da-commitment-reconcile` versus compromissos (SignedBlockWire/`.norito`/JSON) compare کر کے governança کو página کرے۔ |
| Transmissões ao vivo de Taikai کیلئے resiliência de partição | Possível | Crítico | Rede TL | Exercícios de partição reparar reserva de largura de banda کریں؛ documento SOP de failover |
| Desvio de privilégio de governança | Improvável | Alto | Conselho de Governança | `cargo xtask da-privilege-audit` trimestral (diretórios de manifesto/reprodução + caminhos extras) com JSON assinado + portão de painel; artefatos de auditoria کو âncora na cadeia کریں۔ |

## Acompanhamentos necessários

1. DA ingerir esquemas Norito e vetores de exemplo publicar کریں (DA-2 میں لے جائیں)۔
2. Cache de repetição کو Torii DA ingest میں thread کریں اور os cursores de sequência são reiniciados
   کے پار persistir کریں۔
3. **Concluído (05/02/2026):** Chicote de simulação PDP/PoTR e conluio + partição
   cenários e exercícios de modelagem de backlog de QoS دیکھیں
   `integration_tests/src/da/pdp_potr.rs` (testes: `integration_tests/tests/da/pdp_potr_simulation.rs`)۔
4. **Concluído (29/05/2026):** `cargo xtask da-commitment-reconcile` ingerir recibos
   کو Compromissos DA (SignedBlockWire/`.norito`/JSON) سے comparar کر کے
   `artifacts/da/commitment_reconciliation.json` emite کرتا ہے اور Alertmanager/
   pacotes de governança کیلئے com fio ہے (`xtask/src/da.rs`)۔
5. **Concluído (29/05/2026):** Spool de manifesto/repetição `cargo xtask da-privilege-audit`
   (Caminhos fornecidos pelo operador) walk کرتا ہے، ausente/não-diretório/gravável mundialmente
   sinalizador de entradas کرتا ہے، اور painéis de governança de pacote JSON assinados کیلئے بناتا
   ہے (`artifacts/da/privilege_audit.json`)۔

**Onde procurar em seguida:**- DA-2 com cache de repetição e persistência do cursor آچکی ہے۔ Implementação
  `crates/iroha_core/src/da/replay_cache.rs` (lógica de cache) e integração Torii
  `crates/iroha_torii/src/da/ingest.rs` میں، جو `/v2/da/ingest` کے ذریعے impressão digital
  verifica o tópico کرتا ہے۔
- Simulações de streaming PDP / PoTR chicote de fluxo de prova کے ذریعے چلتی ہیں:
  `crates/sorafs_car/tests/sorafs_cli.rs`۔ یہ Fluxos de solicitação PoR/PDP/PoTR
  cobertura de cenários de falha کرتا ہے جو modelo de ameaça میں بیان ہیں۔
- Capacidade de absorção de reparo `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  میں ہیں, جبکہ Sumeragi matriz de imersão `docs/source/sumeragi_soak_matrix.md` میں ہے
  (variantes localizadas شامل ہیں)۔ یہ registro de risco residual de artefatos کے exercícios
  کو capturar کرتے ہیں۔
- Reconciliação + automação de auditoria de privilégios `docs/automation/da/README.md` اور
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` میں ہے؛
  pacotes de governança کیلئے evidência anexar کرتے وقت `artifacts/da/` کی padrão
  saídas

## Evidência de simulação e modelagem de QoS (2026-02)

Acompanhamento DA-1 #3 مکمل کرنے کیلئے ہم نے `integration_tests/src/da/pdp_potr.rs`
( `integration_tests/tests/da/pdp_potr_simulation.rs` سے coberto ) میں


chicote determinístico de simulação PDP/PoTR یہ aproveitar nós e regiões
میں alocar کرتا ہے، probabilidades de roteiro کے مطابق partições/injeção de conluio
کرتا ہے، PoTR lateness track کرتا ہے, اور modelo de backlog de reparo کو feed کرتا ہے جو
orçamento de reparo de nível quente کی عکاسی کرتا ہے۔ Cenário padrão (12 épocas, 18 PDP
desafios + 2 janelas PoTR por época) سے یہ métricas نکلے:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Falhas de PDP detectadas | 48/49 (98,0%) | Partições اب بھی gatilho de detecção کرتے ہیں؛ ایک falha não detectada jitter honesto سے ہے۔ |
| PDP significa latência de detecção | 0,0 épocas | Falhas originando época کے اندر ظاہر ہوتے ہیں۔ |
| Falhas PoTR detectadas | 28/77 (36,4%) | Detecção تب gatilho ہوتی ہے جب nó >=2 janelas PoTR perdidas کرے، زیادہ تر واقعات registro de risco residual میں رہتے ہیں۔ |
| PoTR significa latência de detecção | Épocas 2.0 | Escalação de arquivamento میں شامل دو limite de atraso de época سے correspondência کرتا ہے۔ |
| Pico na fila de reparos | 38 manifestos | Backlog تب بڑھتا ہے جب partições چار reparos/época سے تیز جمع ہوں۔ |
| Latência de resposta p95 | 30.068ms | Janela de desafio de 30 s Amostragem QoS کیلئے +/-75 ms jitter کو refletir کرتا ہے۔ |
<!-- END_DA_SIM_TABLE -->

یہ saídas اب protótipos de painel DA کو unidade کرتے ہیں اور roteiro میں حوالہ
Critérios de aceitação de "chinês de simulação + modelagem QoS" پورے کرتے ہیں۔

Automação `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
کے پیچھے ہے، جو chicote compartilhado کو chamada کرتا ہے اور padrão طور پر Norito JSON
`artifacts/da/threat_model_report.json` میں emite کرتا ہے۔ Arquivo de trabalhos noturnos یہ
consumir کر کے atualização de matrizes de documentos کرتی ہیں اور taxas de detecção, reparar
filas, amostras de QoS, desvio, alerta, alerta

Docs کیلئے اوپر کی atualização de tabela کرنے کو `make docs-da-threat-model` چلائیں، جو
`cargo xtask da-threat-model-report` invocar کرتا ہے،
`docs/source/da/_generated/threat_model_report.json` regenerar کرتا ہے، اور
`scripts/docs/render_da_threat_model_tables.py` کے ذریعے یہ seção reescrita کرتا
ہے۔ Espelho `docs/portal` (`docs/portal/docs/da/threat-model.md`) بھی اسی passe میں
atualizar ہوتا ہے تاکہ دونوں cópias sincronizadas رہیں۔