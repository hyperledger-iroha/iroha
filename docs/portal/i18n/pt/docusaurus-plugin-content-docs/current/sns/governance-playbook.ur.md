---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/governance_playbook.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# Sora Name Service گورننس پلی بک (SN-6)

**حالت:** 2026-03-24 مسودہ - SN-1/SN-6 تیاری کے لئے زندہ حوالہ  
**روڈمیپ لنکس:** SN-6 "Conformidade e resolução de disputas", SN-7 "Resolver e sincronização de gateway", ADDR-1/ADDR-5 ایڈریس پالیسی  
**پیشگی شرائط:** رجسٹری اسکیمہ [`registry-schema.md`](./registry-schema.md) میں, رجسٹرار API معاہدہ [`registrar-api.md`](./registrar-api.md) میں, ایڈریس UX رہنمائی [`address-display-guidelines.md`](./address-display-guidelines.md) میں, اور اکاؤنٹ اسٹرکچر قواعد [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) میں۔

یہ پلی بک بتاتی ہے کہ Sora Name Service (SNS) کی گورننس باڈیز کیسے چارٹر اپناتی
ہیں, رجسٹریشن منظور کرتی ہیں, تنازعات کو escalar کرتی ہیں, اور ثابت کرتی ہیں کہ
resolvedor ou gateway que não funciona یہ روڈمیپ کی اس ضرورت کو پورا
کرتی ہے کہ `sns governance ...` CLI, Norito manifestos e artefatos de auditoria N1
(عوامی لانچ) سے پہلے ایک ہی آپریٹر ریفرنس شیئر کریں۔

## 1. دائرہ کار اور سامعین

یہ دستاویز ان کے لئے ہے:

- Conselho de Governança کے اراکین جو چارٹرز, sufixo پالیسیوں اور تنازعہ نتائج پر ووٹ دیتے ہیں۔
- Quadro guardião کے اراکین جو ہنگامی congela جاری کرتے ہیں اور reversões کا جائزہ لیتے ہیں۔
- Administradores de sufixo جو registrador کی قطاریں چلاتے ہیں, leilões منظور کرتے ہیں, اور divisões de receita سنبھالتے ہیں۔
- Resolver/gateway آپریٹرز جو SoraDNS پھیلاؤ, GAR اپڈیٹس اور guardrails de telemetria کے ذمہ دار ہیں۔
- Compliance, tesouraria ou suporte ٹیمیں جنہیں دکھانا ہوتا ہے کہ ہر گورننس کارروائی نے auditoria کے قابل Norito artefatos چھوڑے۔

یہ `roadmap.md` میں درج بند-بیٹا (N0), عوامی لانچ (N1) اور توسیع (N2) مراحل کو
کور کرتی ہے، ہر ورک فلو کو درکار شواہد, painéis de controle e escalonamento راستوں سے
جوڑتی ہے۔

## 2. کردار اور رابطہ نقشہ| کردار | بنیادی ذمہ داریاں | artefactos materiais e telemetria | Escalação |
|------|--------------------|------------------------------------------|-----------|
| Conselho de Governança | چارٹرز, sufixo پالیسیوں, تنازعہ فیصلوں اور rotações de mordomo کی تدوین و توثیق۔ | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, cédulas do conselho e `sns governance charter submit` سے محفوظ ہوتے ہیں۔ | Presidente do conselho + rastreador de súmula de governança۔ |
| Conselho Tutelar | congelamentos suaves / fortes, cânones ہنگامی, اور 72 h comentários جاری کرتا ہے۔ | Tickets do Guardian جو `sns governance freeze` سے نکلتے ہیں، substituir manifestos جو `artifacts/sns/guardian/*` میں لاگ ہوتے ہیں۔ | Rotação de plantão do guardião (<=15 min ACK)۔ |
| Sufixo Administradores | filas de registradores, leilões, níveis de preços e comunicações com o cliente. reconhecimentos de conformidade | Políticas do administrador `SuffixPolicyV1` میں, folhas de referência de preços, reconhecimentos do administrador e memorandos regulatórios کے ساتھ محفوظ ہوتے ہیں۔ | Líder do programa Steward + sufixo مخصوص PagerDuty۔ |
| Operações de registro e cobrança | Endpoints `/v2/sns/*` چلاتے ہیں, reconciliação de pagamentos کرتے ہیں, emissão de telemetria کرتے ہیں, e instantâneos CLI برقرار رکھتے ہیں۔ | API do registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, comprovantes de pagamento e `artifacts/sns/payments/*` میں محفوظ ہیں۔ | Gerente de registro e contato com tesouraria۔ |
| Operadores de resolução e gateway | SoraDNS, GAR e estado de gateway کو eventos de registrador کے ساتھ alinhados رکھتے ہیں؛ fluxo de métricas de transparência کرتے ہیں۔ | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE de plantão + ponte de operações de gateway۔ |
| Tesouraria e Finanças | Divisão de receita 70/30, divisões de referência, registros fiscais/do tesouro e atestados de SLA نافذ کرتے ہیں۔ | Manifestos de acumulação de receitas, Exportações Stripe/tesouraria, اور سہ ماہی Apêndices KPI `docs/source/sns/regulatory/` میں۔ | Controlador financeiro + diretor de compliance۔ |
| Contato de Conformidade e Regulamentação | عالمی ذمہ داریوں (EU DSA وغیرہ) کو ٹریک کرتا ہے, convênios KPI اپڈیٹ کرتا ہے, اور divulgações فائل کرتا ہے۔ | Memorandos regulatórios `docs/source/sns/regulatory/` میں, decks de referência, e ensaios de mesa کے لئے entradas `ops/drill-log.md`۔ | Líder do programa de compliance۔ |
| Suporte / SRE de plantão | incidentes (colisões, desvio de faturamento, interrupções no resolvedor) | Modelos de incidentes, `ops/drill-log.md`, evidências de laboratório encenadas, transcrições de Slack/sala de guerra e `incident/` میں محفوظ ہیں۔ | Rotação de plantão SNS + gerenciamento SRE۔ |

## 3. Artefatos کینونیکل اور ڈیٹا ذرائع| Artefato | مقام | مقصد |
|----------|------|------|
| Carta + Adendos de KPI | `docs/source/sns/governance_addenda/` | ورژن کنٹرول شدہ دستخط شدہ چارٹرز, convênios KPI, اور گورننس فیصلے, e votos CLI سے ریفرنس ہوتے ہیں۔ |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Cartão Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`)۔ |
| Contrato de registrador | [`registrar-api.md`](./registrar-api.md) | Cargas úteis REST/gRPC, métricas `sns_registrar_status_total` e gancho de governança |
| Guia de UX de endereço | [`address-display-guidelines.md`](./address-display-guidelines.md) | کینونیکل I105 (ترجیحی) / compactado (`sora`) (`sora`, segundo melhor) renderizações جو carteiras/exploradores میں جھلکتے ہیں۔ |
| Documentos SoraDNS/GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivação de host determinística, fluxo de trabalho de alfaiataria de transparência e regras de alerta |
| Memorandos regulamentares | `docs/source/sns/regulatory/` | Notas de admissão jurisdicional (مثلا EU DSA), reconhecimentos de administrador, anexos de modelo۔ |
| Registro de perfuração | `ops/drill-log.md` | Caos اور ensaios IR کا ریکارڈ جو saídas de fase سے پہلے لازم ہیں۔ |
| Armazenamento de artefatos | `artifacts/sns/` | Provas de pagamento, tickets de guardião, diferenças de resolução, exportações de KPI, `sns governance ...` ou saída CLI assinada |

تمام گورننس اقدامات کو اوپر کی جدول میں سے کم از کم ایک artefato کا حوالہ دینا
چاہیے تاکہ auditores 24 گھنٹوں میں trilha de decisão دوبارہ بنا سکیں۔

## 4. لائف سائیکل پلی بکس

### 4.1 Estatuto e moções do administrador

| مرحلہ | Mal | CLI / Evidência | Não |
|-------|------|----------------|------|
| Rascunho de adendo e deltas de KPI | Relator do Conselho + administrador líder | Modelo de Markdown `docs/source/sns/governance_addenda/YY/` میں محفوظ | IDs de convênio KPI, ganchos de telemetria e condições de ativação |
| Envio de proposta | Presidente do conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` بناتا ہے) | CLI Norito manifesto `artifacts/sns/governance/<id>/charter_motion.json` میں محفوظ کرتی ہے۔ |
| Vote no reconhecimento do guardião | Conselho + tutores | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Minutos hash e provas de quorum منسلک کریں۔ |
| Aceitação do administrador | Programa de administrador | `sns governance steward-ack --proposal <id> --signature <file>` | Políticas de sufixo تبدیل کرنے سے پہلے لازم؛ `artifacts/sns/governance/<id>/steward_ack.json` envelope de envelope ریکارڈ کریں۔ |
| Ativação | Operações de registrador | `SuffixPolicyV1` اپڈیٹ کریں, caches de registrador ریفریش کریں, `status.md` میں نوٹ شائع کریں۔ | Carimbo de data e hora de ativação `sns_governance_activation_total` میں لاگ ہوتا ہے۔ |
| Registro de auditoria | Conformidade | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` Um registro de perfuração de mesa para mesa | Painéis de telemetria e diferenças de política |

### 4.2 Registro, leilão e aprovações de preços1. **Preflight:** Registrador `SuffixPolicyV1` کو دیکھ کر nível de preços, termos de termos e اور
   janelas de graça/redenção کی تصدیق کرتا ہے۔ قیمت کی شیٹس کو roteiro میں درج
   3/4/5/6-9/10+ nível ٹیبل (nível base + coeficientes de sufixo) کے مطابق رکھیں۔
2. **Leilões de lance selado:** Pools premium com ciclo de confirmação de 72 horas / ciclo de revelação de 24 horas
   `sns governance auction commit` / `... reveal` کے ذریعے چلائیں۔ cometer
   (Hashes) `artifacts/sns/auctions/<name>/commit.json` میں شائع کریں تاکہ
   verificação de aleatoriedade dos auditores
3. **Verificação de pagamento:** Registradores `PaymentProofV1` کو divisões de tesouraria کے خلاف
   validar کرتے ہیں (70% tesouraria / 30% administrador, separação de referência 72 horas, picos de erro de registrador, desvio de ARPU) ou incidente
  gatilho

### 4.4 Congelamentos, disputas e apelações| مرحلہ | Mal | Ação ou Evidência | SLA |
|-------|------|--------------------|-----|
| Solicitação de congelamento suave | Administrador / suporte | Bilhete `SNS-DF-<id>` فائل کریں جس میں comprovantes de pagamento, referência de título de disputa, اور seletor(es) afetado(s) ہوں۔ | <=4 h de ingestão سے۔ |
| Bilhete do guardião | Quadro guardião | `sns governance freeze --selector <I105> --reason <text> --until <ts>` `GuardianFreezeTicketV1` بناتا ہے۔ Bilhete JSON `artifacts/sns/guardian/<id>.json` میں رکھیں۔ | <=30 min ACK, <=2 h de execução۔ |
| Ratificação do Conselho | Conselho de governação | Congelar aprovação/rejeição کریں, bilhete de guardião اور resumo de títulos de disputa کے ساتھ فیصلہ ڈاکیومنٹ کریں۔ | اگلا sessão do conselho یا votação assíncrona۔ |
| Painel de arbitragem | Conformidade + administrador | 7 painel de jurados (roteiro کے مطابق) بلائیں، cédulas com hash `sns governance dispute ballot` کے ذریعے جمع کریں۔ Pacote de incidentes de recibos de voto anônimos میں لگائیں۔ | Veredicto <=7 dias بعد depósito de títulos۔ |
| Recurso | Tutor + conselho | Título de apelação کو دوگنا کرتی ہیں اور processo de jurado دہراتی ہیں؛ Manifesto Norito `DisputeAppealV1` ریکارڈ کریں اور ticket primário ریفرنس کریں۔ | <=10 dias۔ |
| Descongelar e remediar | Operações de registrador + resolvedor | `sns governance unfreeze --selector <I105> --ticket <id>` چلائیں، status do registrador اپڈیٹ کریں, اور GAR/resolver diffs propagate کریں۔ | Veredicto کے فوراً بعد۔ |

Cânones de emergência (congelamentos acionados pelo guardião <=72 h) بھی اسی fluxo کو فالو کرتے ہیں
Revisão retroativa do conselho e `docs/source/sns/regulatory/` Transparência
nota درکار ہے۔

### 4.5 Resolver e propagação de gateway

1. **Gancho de evento:** fluxo de eventos do resolvedor de eventos de registro (`tools/soradns-resolver` SSE)
   emitir ہوتا ہے۔ Assinatura de operações do resolvedor para alfaiate de transparência
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) کے ذریعے diffs ریکارڈ کرتے ہیں۔
2. **Atualização do modelo GAR:** Gateways کو `canonical_gateway_suffix()` سے consulte ہونے والے modelos GAR
   اپڈیٹ کرنے ہوں گے اور `host_pattern` lista کو دوبارہ sinal کرنا ہوگا۔ Diferenças
   `artifacts/sns/gar/<date>.patch` میں رکھیں۔
3. **Publicação do arquivo de zona:** `roadmap.md` میں بیان کردہ esqueleto do arquivo de zona (nome, ttl, cid, prova)
   Selecione o cartão Torii/SoraFS por push Norito JSON
   `artifacts/sns/zonefiles/<name>/<version>.json` arquivo de arquivo کریں۔
4. **Verificação de transparência:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   چلائیں تاکہ alertas verdes رہیں۔ Saída de texto Prometheus کو relatório de transparência semanal کے ساتھ منسلک کریں۔
5. **Auditoria de gateway:** Amostras de cabeçalho `Sora-*` (política de cache, CSP, resumo GAR) ریکارڈ کریں اور انہیں
   log de governança کے ساتھ anexar کریں تاکہ آپریٹرز ثابت کر سکیں کہ gateway نے نیا نام
   مطلوبہ guarda-corpos کے ساتھ servir کیا۔

## 5. Relatórios de telemetria e| Sinal | Fonte | Descrição / Ação |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` | Manipuladores de registradores Torii | رجسٹریشن, renovações, congelamentos, transferências کے لئے contador de sucesso/erro؛ جب Sufixo `result="error"` کے حساب سے بڑھ جائے تو alert۔ |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métricas Torii | Manipuladores de API são SLOs de latência; `torii_norito_rpc_observability.json` سے بنے dashboards e feed کرتا ہے۔ |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Resolvedor de transparência | پرانے provas یا GAR drift کو detectar کرتا ہے؛ guarda-corpos `dashboards/alerts/soradns_transparency_rules.yml` میں definir ہیں۔ |
| `sns_governance_activation_total` | CLI de governança | ہر ativação de fretamento / adendo پر بڑھنے والا contador؛ decisões do conselho کو adendos publicados کے ساتھ reconciliar کرنے میں استعمال۔ |
| Medidor `guardian_freeze_active` | Guardião CLI | ہر seletor کے لئے soft/hard freeze windows track کرتا ہے؛ اگر `1` SLA سے زیادہ رہے تو SRE کو página کریں۔ |
| Painéis anexos de KPI | Finanças / Documentos | Rollups de ماہانہ e memorandos regulatórios کے ساتھ شائع ہوتے ہیں؛ portal تک پہنچیں۔ |

## 6. Evidências e requisitos de auditoria

| Ação | Provas a arquivar | Armazenamento |
|--------|---------|---------|
| Carta/mudança de política | Manifesto Norito assinado, transcrição CLI, diferença de KPI, reconhecimento do administrador ۔ | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registo/renovação | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prova de pagamento۔ | `artifacts/sns/payments/<tx>.json`, registros da API do registrador۔ |
| Leilão | Confirmar/revelar manifestos, semente de aleatoriedade, planilha de cálculo do vencedor۔ | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket do guardião, hash de votação do conselho, URL de registro de incidentes, modelo de comunicação do cliente۔ | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagação do resolvedor | Zonefile/GAR diff, trecho JSONL personalizado, instantâneo Prometheus۔ | `artifacts/sns/resolver/<date>/` + relatórios de transparência۔ |
| Ingestão regulamentar | Memorando de entrada, rastreador de prazo, reconhecimento do administrador, resumo de alterações de KPI ۔ | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Lista de verificação do portão de fase

| Fase | Critérios de saída | Pacote de evidências |
|-------|---------------|-----------------|
| N0 - Beta fechado | Esquema de registro SN-1/SN-2, CLI do registrador manual, exercício de guardião مکمل۔ | Movimento de fretamento + administrador ACK, registros de simulação do registrador, relatório de transparência do resolvedor, entrada `ops/drill-log.md`۔ |
| N1 - Lançamento público | Leilões + níveis de preço fixo ativos para `.sora`/`.nexus`, registrador de autoatendimento, sincronização automática de resolução, painéis de faturamento۔ | Diferença da folha de preços, resultados de CI do registrador, anexo de pagamento/KPI, saída do alfaiate de transparência, notas de ensaio de incidentes۔ |
| N2 - Expansão | `.dao`, APIs de revendedor, portal de disputas, scorecards de administrador, painéis analíticos۔ | Capturas de tela do portal, métricas de SLA de disputa, exportações de scorecard de administrador, carta de governança atualizada referenciando políticas de revendedor ۔ |

Saídas de fase کو ریکارڈ شدہ brocas de mesa (caminho feliz de registro, congelamento, interrupção do resolvedor) درکار ہیں جن کے
artefatos `ops/drill-log.md` میں منسلک ہوں۔## 8. Resposta a incidentes e escalonamento

| Gatilho | Gravidade | Proprietário imediato | Ações obrigatórias |
|--------|----------|-----------------|------------------|
| Desvio do Resolver/GAR e provas obsoletas | 1º de setembro | Resolver SRE + quadro guardião | resolvedor de plantão کو página کریں, captura de saída do tailer کریں, متاثرہ نام congelar کرنے کا فیصلہ کریں, ہر 30 min پر atualização de status دیں۔ |
| Interrupção do registrador, falha no faturamento e erros generalizados de API | 1º de setembro | Gerente de serviço de registrador | نئی leilões روکیں, manual CLI پر سوئچ کریں, stewards/tesouraria کو اطلاع دیں, Torii logs کو documento de incidente میں anexar کریں۔ |
| Disputa de nome único, incompatibilidade de pagamento, escalonamento de clientes | 2 de setembro | Administrador + líder de suporte | comprovantes de pagamento جمع کریں, soft freeze کی ضرورت طے کریں, SLA کے اندر solicitante کو جواب دیں, rastreador de disputa میں نتیجہ لاگ کریں۔ |
| Constatação de auditoria de conformidade | 2 de setembro | Contato de conformidade | plano de remediação تیار کریں, `docs/source/sns/regulatory/` میں memorando فائل کریں, sessão do conselho de acompanhamento شیڈول کریں۔ |
| Ensaio de treino | 3 de setembro | Programa PM | `ops/drill-log.md` کے cenário com script کو چلائیں, arquivo de artefatos کریں, lacunas کو tarefas de roteiro کے طور پر لیبل کریں۔ |

Os incidentes são `incident/YYYY-MM-DD-sns-<slug>.md` e as tabelas de propriedade
logs de comando, isso é uma evidência کے referências ہوں۔

## 9. حوالہ جات

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (seções SNS, DG, ADDR)

جب بھی redação do regulamento, superfícies CLI یا contratos de telemetria بدلیں تو اس پلی بک کو
اپڈیٹ رکھیں؛ entradas de roteiro جو `docs/source/sns/governance_playbook.md` کو
ریفرنس کرتی ہیں انہیں ہمیشہ تازہ ترین ورژن سے میل کھانا چاہیے۔