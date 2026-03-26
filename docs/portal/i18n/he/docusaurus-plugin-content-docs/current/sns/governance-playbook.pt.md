---
lang: he
direction: rtl
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sns/governance_playbook.md` e agora serve como a
Copia canonica do פורטל. O arquivo fonte permanece para PRs de traducao.
:::

# Playbook de governanca do Sora Name Service (SN-6)

**סטטוס:** Redigido 2026-03-24 - referencia viva para a prontidao SN-1/SN-6  
**קישורים למפת הדרכים:** SN-6 "תאימות ופתרון מחלוקות", SN-7 "Resolver & Gateway Sync", politica de endereco ADDR-1/ADDR-5  
**דרישות מוקדמות:** Esquema do registro em [`registry-schema.md`](./registry-schema.md), contrato da API do registrer em [`registrar-api.md`](./registrar-api.md), guia UX de enderecos em [`address-display-guidelines.md`](./address-display-guidelines.md), e regras de estrutura de contas em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este Playbook מגדיר את como os corpos de governanca do Sora Name Service (SNS)
adotam cartas, aprovam registros, escalam disputas e provam que os estados de
resolver e gateway permanecem em sincronia. כל מה שצריך לעשות מפת הדרכים
que a CLI `sns governance ...`, מניפסטים Norito e artefatos de auditoria
compartilhem uma unica referencia voltada ao operador antes do N1 (lancamento
פובליקו).

## 1. Escopo e publico

O documento se destina a:

- Membros do Conselho de Governanca que votam em cartas, politicas de sufixo e
  resultados de disputa.
- Membros do conselho de guardians que emitem congelamentos de emergencia e
  revisam מתהפך.
- Stewards de Sufixo que operam filas do registrar, aprovam leiloes e gerenciam
  divisao de receitas.
- תגובת מפעילי פתרון/שער הדרכה על SoraDNS, אטואליזacoes
  GAR e מעקות בטיחות de telemetria.
- Equipes de conformidade, tesouraria e suporte que devem demonstrar que toda
  acao de governanca deixou artefatos Norito auditavis.

Ele cobre as fases beta fechada (N0), lancamento publico (N1) e expansao (N2)
listadas em `roadmap.md`, vinculando cada fluxo de trabalho as evidencias
נחוצים, לוחות מחוונים e caminhos de escalonamento.

## 2. Papeis e mapa de contato| פאפל | Responsabilidades principais | Artefatos e telemetria principais | Escalacao |
|-------|-------------------------------|--------------------------------|-----------|
| Conselho de Governanca | Redige e ratifica cartas, politicas de sufixo, vereditos de disputa e rotacoes de steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos do conselho armazenados via `sns governance charter submit`. | Presidente do conselho + rastreador de agenda de governanca. |
| Conselho de guardians | Emite congelamentos רך/קשה, canones de emergencia e revisoes de 72 h. | אפוטרופוס הכרטיסים emitidos por `sns governance freeze`, מניפסטים לעקוף את הרישומים על `artifacts/sns/guardian/*`. | אפוטרופוס תורן של Rotacao (<=15 דקות ACK). |
| דיילים דה סופיקסו | Operam filas do registrar, leiloes, niveis de preco e comunicacao com clientes; reconhecem conformidades. | פוליטיקה של דייל עם `SuffixPolicyV1`, תזכורת מוקדמת, תודות של דייל ארמאזנדו ותזכירים תקנות. | Lider do programa steward + PagerDuty por sufixo. |
| Operacoes de registrar e cobranca | נקודות קצה של אופרה `/v1/sns/*`, התאמה בין עמודים, טלמטריה וצילומי מצב של CLI. | ממשק API לרשם ([`registrar-api.md`](./registrar-api.md)), מדדים `sns_registrar_status_total`, הוכחות ל-Pagamento arquivadas em `artifacts/sns/payments/*`. | מנהל תורן לעשות רשם e liaison da tesouraria. |
| Operadores de resolver e gateway | Mantem SoraDNS, GAR e estado do gateway alinhados com eventos do do registrar; transmitem metricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | פותר SRE ב-call + ponte ops לעשות שער. |
| Tesouraria e financas | Aplica divisao de receita 70/30, Carve-outs de referral, registros fiscais/tesouraria e atestacoes SLA. | Manifestos de acumulacao de receita, יצוא Stripe/tesouraria, apendices KPI trimestrais em `docs/source/sns/regulatory/`. | בקר פיננסי + אוficial de conformidade. |
| Liaison de conformidade e regulacao | Acompanha obrigacoes globais (EU DSA, וכו'), התחייבויות KPI e registra divulgacoes. | תזכירים רגולטוריים של `docs/source/sns/regulatory/`, חפיסות רפרנס, מקורות `ops/drill-log.md` עבור שולחן עבודה. | Lider do programa de conformidade. |
| תמיכה / SRE כוננות | Lida com incidentes (colisoes, drift de cobranca, quedas de resolver), coordena mensagens a clientes e e dono dos runbooks. | Templates de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcricoces Slack/war-room arquivadas em `incident/`. | Rotacao כוננות SNS + gestao SRE. |

## 3. Artefatos canonicos e fontes de dados| ארטפטו | Localizacao | פרופוזיטו |
|--------|------------|--------|
| Carta + תוספת KPI | `docs/source/sns/governance_addenda/` | Cartas assinadas com control de versao, covenants KPI ecisoes de governanca referenciadas por votos da CLI. |
| Esquema do registro | [`registry-schema.md`](./registry-schema.md) | Estruturas Norito canonicas (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato do רשם | [`registrar-api.md`](./registrar-api.md) | מטענים REST/gRPC, מדדים `sns_registrar_status_total` והוק ציפיות לממשל. |
| Guia UX de enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizacoes canonicas i105 (preferido) e comprimidas (segunda melhor opcao) refletidas por ארנקים/חוקרים. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivacao deterministica de hosts, fluxo do tailer de transparencia e regras de alerta. |
| תזכירים רגולטוריים | `docs/source/sns/regulatory/` | Notas de entrada por jurisdicao (לדוגמה, DSA של האיחוד האירופי), תודות של דייל, מצורפים לתבנית. |
| יומן מקדחה | `ops/drill-log.md` | Registro de ensaios de caos e IR requeridos antes de saidas de fase. |
| Armazenamento de artefatos | `artifacts/sns/` | Provas de pagamento, שומר כרטיסים, פתרון הבדלים, יצוא KPI e saida de CLI assinada produzida por `sns governance ...`. |

Todas as acoes de governanca devem referenciar pelo menos um artefato na tabela
acima para que auditores reconstruam o Rastro de decisao em 24 horas.

## 4. Playbooks de ciclo de vida

### 4.1 דייל מוצלח

| אטאפה | שו"ת | CLI / Evidencia | Notas |
|-------|-------------|----------------|------|
| ספרו מחדש את התוספת והדלתות KPI | Relator do conselho + lider steward | תבנית Markdown armazenado em `docs/source/sns/governance_addenda/YY/` | כלול מזהים של אמנה KPI, ווים של טלמטריה ו-Condicoes de Ativacao. |
| Enviar proposta | Presidente do conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (מוצר `CharterMotionV1`) | מניפסט CLI emite Norito salvo em `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto e אישור אפוטרופוס | Conselho + אפוטרופוסים | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Anexar atas hasheadas e provas de quorum. |
| דייל Aceitacao | Programa de steward | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatorio antes de mudar politicas de sufixo; מעטפת הרשם em `artifacts/sns/governance/<id>/steward_ack.json`. |
| אטיבקאו | Ops do רשם | Atualizar `SuffixPolicyV1`, מטמון אטואליזר לעשות רשם, פרסם לא עם `status.md`. | חותמת זמן של אטivacao registrado em `sns_governance_activation_total`. |
| Log de auditoria | Conformidade | מצורפים ל-`docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` וללא יומן מקדחה עם משטח שולחן. | כולל הפניות ללוחות מחוונים של טלמטריה והבדל פוליטי. |

### 4.2 Aprovacoes de registro, leilao e preco1. ** טיסה מוקדמת:** O רשם ייעוץ `SuffixPolicyV1` para confirmar nivel de
   preco, termos disponiveis e janelas de graca/redencao. Mantenha folhas de
   preco sincronizadas com a tabela de niveis 3/4/5/6-9/10+ (בסיס nivel +
   coeficientes de sufixo) documentada ללא מפת דרכים.
2. **Leiloes הצעה חתומה:** Para pools premium, execute o ciclo 72 h commit /
   חשיפה של 24 שעות דרך `sns governance auction commit` / `... reveal`. פרסום א
   list de commits (apenas hashes) em `artifacts/sns/auctions/<name>/commit.json`
   para que auditores verifiquem a aleatoriedade.
3. **Verificacao de pagamento:** הרשמים validam `PaymentProofV1` contra a
   divisao de tesouraria (70% tesouraria / 30% דייל com carve-out de reference <=10%).
   Armazene o JSON Norito em `artifacts/sns/payments/<tx>.json` e vincule-o na
   Reposta do registrar (`RevenueAccrualEventV1`).
4. **Hook de governanca:** Anexe `GovernanceHookV1` para nomes premium/guarded com
   referencia a ids de proposta do conselho e assinaturas de steward. ווים
   ausentes resultam em `sns_err_governance_missing`.
5. **Ativacao + סנכרון לפתרון:** Assim que Torii emitir or evento de registro,
   acione o tailer de transparencia do resolver para confirmar que o novo estado
   GAR/zone se propagou (veja 4.5).
6. **Divulgacao ao cliente:** Atualize o Ledger voltado ao cliente (ארנק/חוקר)
   via OS fixtures compartilhados em [`address-display-guidelines.md`](./address-display-guidelines.md),
   garantindo que renderizacoes i105 e comprimidas correspondam and orientacoes de copy/QR.

### 4.3 Renovacoes, cobranca e reconciliacao da tesouraria

- **Fluxo de renovacao:** הרשמים אפליקאם a janela de graca de 30 dias + janela
  de redencao de 60 dias especificadas em `SuffixPolicyV1`. אפוס 60 דיאס, א
  sequencia de reabertura holandesa (7 חולצות, taxa 10x decaindo 15%/dia) e
  acionada automaticamente דרך `sns governance reopen`.
- **Divisao de receita:** Cada renovacao ou transferencia cria um
  `RevenueAccrualEventV1`. Exports de tesouraria (CSV/Parquet) devem מתאים
  esses eventos diariamente; anexe provas em `artifacts/sns/treasury/<date>.json`.
- **הסתייגויות של הפניה:** אחוזי הפניה אופציונאיים סאו rastreados por
  sufixo ao adicionar `referral_share` א פוליטיקה דייל. הרשמים פולטים א
  divisao final e armazenam manifestos de referral ao lado da prova de pagamento.
- **Cadencia de relatorios:** Financas publica anexos KPI mensais (registros,
  renovacoes, ARPU, uso de disputas/bond) em `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  לוחות מחוונים devem puxar das mesmas tabelas exportadas para que os numeros de
  Grafana batam com as Evidencias do Ledger.
- **Revisao KPI mensal:** O checkpoint da primeira terca-feira junta o lider de
  financas, steward de plantao e PM do programa. Abra o [לוח מחוונים SNS KPI](./kpi-dashboard.md)
  (הטמע את הפורטל של `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  ייצא כטבלאות תפוקה + קבלת רשם, רשם דלתות מס
  anexo e anexe os artefatos ao memo. Acione um incidente se a revisao encontrar
  quebras de SLA (Janelas de freeze >72 שעות, picos dero do do registrar, drift de ARPU).### 4.4 Congelamentos, disputas e apelacoes

| פאזה | שו"ת | Acao e evidencia | SLA |
|------|-------------|----------------|-----|
| Pedido de freeze soft | דייל / תמיכה | Abrir כרטיס `SNS-DF-<id>` com provas de pagamento, referencia do bond de disputa e seletor(es) afetados. | <=4 שעות בהתחלה. |
| אפוטרופוס כרטיס | אפוטרופוס קונסלו | `sns governance freeze --selector <i105> --reason <text> --until <ts>` פרודוז `GuardianFreezeTicketV1`. Armazene o JSON לעשות כרטיס עם `artifacts/sns/guardian/<id>.json`. | <=30 דקות ACK, <=2 שעות ביצוע. |
| Ratificacao do conselho | Conselho de governanca | Aprovar ou rejeitar congelamentos, documentar decisao com link ao guardian e digest do bond de disputa. | Proxima sessao do conselho או voto assincrono. |
| Painel de arbitragem | Conformidade + דייל | Convocar painel de 7 jurads (תאם מפת הדרכים) com cedulas hasheadas דרך `sns governance dispute ballot`. Anexar recibos de voto anonimizados ao pacote de incidente. | Veredito <=7 dias apos deposito do bond. |
| אפלסאו | אפוטרופוס + קונסלהו | Apelacoes dobram o bond e repetem o processo de jurados; מניפסט רשם Norito `DisputeAppealV1` e referenciar ticket primario. | <=10 dias. |
| Descongelar e remediar | רשם + ops de resolver | מפעיל `sns governance unfreeze --selector <i105> --ticket <id>`, התקן את סטטוס הרשם וההפצה של GAR/פותר. | Imediatamente apos o veredito. |

Canones de emergencia (congelamentos acionados por guardian <=72 h) seguem o mesmo
fluxo, mas exigem revisao retroativa do conselho e uma not de transparencia em
`docs/source/sns/regulatory/`.

### 4.5 Propagacao de resolver e gateway

1. **Hook de evento:** Cada evento de registro emite para o stream de eventos do
   פותר (`tools/soradns-resolver` SSE). Ops de resolver se inscrevem e
   registram diffs דרך o tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Atualizacao de template GAR:** Gateways devem atualizar templates GAR
   התייחסות ל-`canonical_gateway_suffix()` והרשימה מחדש
   `host_pattern`. Armazene diffs em `artifacts/sns/gar/<date>.patch`.
3. **Publicacao de zonefile:** השתמש בתיאור שלד אזור ב-`roadmap.md`
   (שם, ttl, cid, הוכחה) e envie para Torii/SoraFS. Arquive o JSON Norito em
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Cheque de transparencia:** בצע את `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas sigam verdes. Anexe a saida de texto do
   Prometheus או יחס סממני של שקיפות.
5. **Auditoria de gateway:** Registre amostras de headers `Sora-*` (politica de
   cache, CSP, digest GAR) e anexe-as ao log de governanca para que operadores
   possam provar que o gateway serviu o novo nome com os מעקות בטיחות esperados.

## 5. Telemetria e relatorios| סינאל | פונטה | Descricao / Acao |
|-------|-------|------------------------|
| `sns_registrar_status_total{result,suffix}` | מטפלים עושים רשם Torii | Contador de sucesso/erro para registros, renovacoes, congelamentos, transferencias; alerta quando `result="error"` aumenta por sufixo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Metricas Torii | SLOs de latencia para handlers de API; לוחות מחוונים של alimenta baseados em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Tailer de transparencia do resolver | Detecta provas obsoletas ou drift de GAR; מעקות בטיחות definidos em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de governanca | Contador incrementado quando um charter/addendum ativa; usado para reconciliar decisoes do conselho vs addenda publicadas. |
| מד `guardian_freeze_active` | אפוטרופוס CLI | Acompanha janelas de freeze soft/hard por seletor; page SRE se o valor ficar `1` alem do SLA declarado. |
| לוחות מחוונים של anexos KPI | Financas / Docs | רגולפים mensais publicados junto מזכרים regulatorios; o פורטל מערכת ההפעלה באמצעות [לוח המחוונים של SNS KPI](./kpi-dashboard.md) עבור דיילים ומפקחים לקבל את הרשאת ויזת Grafana. |

## 6. Requisitos de evidencia e auditoria

| Acao | Evidencia a arquivar | Armazenamento |
|------|----------------------|-------------|
| Mudanca de carta / politica | מניפסט Norito assinado, תמליל CLI, הבדל של KPI, אישור דייל. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovacao | מטען `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prova de pagamento. | `artifacts/sns/payments/<tx>.json`, יומני ממשק API לרשם. |
| לילאו | מניפסטים מתחייבים/חושפים, סמנטה דה aleatoriedade, planilha de calculo do vencedor. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | אפוטרופוס כרטיס, hash de voto do conselho, URL de log de incidente, template de comunicacao com cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagacao de resolver | Diff zonefile/GAR, trecho JSONL do tailer, תמונת מצב Prometheus. | `artifacts/sns/resolver/<date>/` + relatorios de transparencia. |
| רגולציית צריכת | תזכיר הכנסה, מעקב אחר מועדים, הודאה לדייל, רזומה של מודאנקאס KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. רשימת רשימות לשער שלב| פאזה | קריטריונים דה אמרה | Bundle de evidencia |
|------|------------------------|------------------------|
| N0 - Beta fechada | Esquema de registro SN-1/SN-2, CLI de registrar manual, מקדחה אפוטרופוס מלאה. | Motion de carta + דייל ACK, יומני ריצה יבשים לעשות רשם, relatorio de transparencia do resolver, entrada em `ops/drill-log.md`. |
| N1 - Lancamento publico | Leiloes + tiers de preco fixo ativos para `.sora`/`.nexus`, שירות עצמי של רשם, סנכרון אוטומטי של פותר, לוחות מחוונים של cobranca. | Diff de folha de preco, resultados CI do registrar, anexo de pagamento/KPI, saida do tailer de transparencia, notas de ensaio de incidente. |
| N2 - Expansao | `.dao`, ממשקי API למפיץ, פורטל דירוג, כרטיסי ניקוד של דייל, לוחות מחוונים של אנליטיקה. | קביעות פורטל, מדדי SLA deputa, ייצוא כרטיסי ניקוד של דייל, כרטיס ממשלתי אטואליזאד com politicas de reseller. |

כפי שאמרתי מקדחות שולחן עבודה (fluxo feliz de registro,
הקפאה, הפסקת פתרון) com artefatos anexados em `ops/drill-log.md`.

## 8. תגובה לאירועים escalonamento

| Gatilho | Severidade | Dono imediato | Acoes obrigatorias |
|--------|--------|--------------|------------------|
| Drift de resolver/GAR ou provas obsoletas | סב 1 | פותר SRE + אפוטרופוס קונסלהו | היכנס ל-Congelar do resolver, לכידת כתובה לעשות tailer, קבע את התפתחות ההגדרות שלנו, סטטוס פרסום עד 30 דקות. |
| Queda de registrar, falha de cobranca, ou errors API generalizados | סב 1 | מנהל חובה לעשות רשם | Pare novos leiloes, mude para CLI manual, notifique stewards/tesouraria, anex logs do Torii ao doc de incidente. |
| Disputa de nome unico, חוסר התאמה של פגאמנטו, או escalonamento de cliente | סב' 2 | דייל + lider de suporte | Colete provas de pagamento, לקבוע את ההקפאה רך והכרחי, להגיב או להגיש בקשה לשירות SLA, לרשום או תוצאות ללא מעקב אחר מחלוקת. |
| Achado de auditoria de conformidade | סב' 2 | Liaison de conformidade | Redigir plano de remediacao, arquivar memo em `docs/source/sns/regulatory/`, agenda sessao de conselho de acompanhamento. |
| Drill ou ensaio | סוו 3 | PM do programa | בצע את אירועי התפעול של `ops/drill-log.md`, ארכיון פריטים, פערי מותגים כמו מפת דרכים. |

Todos OS incidentes devem criar `incident/YYYY-MM-DD-sns-<slug>.md` עם טבלאות
דה בעלות, logs de comandos e referencias as evidencias produzidas ao longo
חוברת משחק.

## 9. התייחסויות

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (Secos SNS, DG, ADDR)

Mantenha este playbook atualizado semper que o texto das cartas, as superficies
de CLI ou os contratos de telemetria mudarem; כמו entradas לעשות מפת הדרכים que
referenciam `docs/source/sns/governance_playbook.md` devem semper corresponder a
ultima revisao.