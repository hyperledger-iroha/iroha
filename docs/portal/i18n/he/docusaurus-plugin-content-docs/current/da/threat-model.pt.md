---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Espelha `docs/source/da/threat_model.md`. Mantenha as duas versoes em
:::

# Modelo de ameacas de Data Availability da Sora Nexus

_Ultima revisao: 2026-01-19 -- Proxima revisao programada: 2026-04-19_

Cadencia de manutencao: קבוצת עבודה זמינות נתונים (<=90 dias). Cada revisao

deve aparecer em `status.md` com קישורים para tickets de mitigacao ativos e
artefatos de simulacao.

## Proposito e escopo

O programa de Data Availability (DA) מעביר טאיקאי, כתמי נתיב
Nexus e artefatos de governanca recuperaveis sob falhas bizantinas, de rede e de
אופרדורים. Este modelo de ameacas ancora o trabalho de engenharia para DA-1
(arquitetura e modelo de ameacas) e serve como baseline para tarefas DA
posteriores (DA-2 a DA-10).

Componentes no escopo:
- Extensao de ingestao DA do Torii e writers de metadata Norito.
- Arvores de armazenamento de blobs suportadas por SoraFS (רמות חם/קר) e
  politicas de replicacao.
- Commitments de blocos Nexus (פורמטים של חוטים, הוכחות, APIs de cliente level).
- Hooks deforcement PDP/PoTR especificos para payloads DA.
- זרימות עבודה של מפעילים (הצמדה, פינוי, חיתוך) וצינורות
  observabilidade.
- Aprovacoes de governanca que admitem ou removem operadores e conteudo DA.

Fora do escopo deste documento:
- Modelagem economica completa (capturada no workstream DA-7).
- בסיס פרוטוקולים SoraFS ו-cobertos modelo de ameacas SoraFS.
- Ergonomia de SDK de cliente alem de consideracoes de superficie de ameaca.

## Visao arquitetural

1. **Submissao:** Clientes submetem blobs דרך API de ingestao DA do Torii. O
   כתמי חלוקת צומתים, גילויי קודיפיקה Norito (טיפו דה גוש, נתיב, עידן, דגלים
   de codec), e armazena chunks ללא tier hot do SoraFS.
2. **Anuncio:** כוונות סיכות e hints de replicacao propagam para provedores de
   אחסון באמצעות רישום (שוק SoraFS) com tags de politica que indicam
   metas de retencao חם/קר.
3. **מחויבות:** Sequenciadores Nexus כוללות התחייבויות של כתמים (CID + שורשים
   KZG opcionais) ללא בלוקו קאנוניקו. Clientes leves dependem do hash de
   התחייבות e da metadata הודעה עבור אימות זמינות.
4. **רפליקאו:** Nodos de armazenamento puxam shares/chunks atribuidos, atendem
   desafios PDP/PoTR, וקידום אתרים ראשוניים חמים וקרים בהתאם לפוליטיקה.
5. **אחזר:** Consumidores buscam dados דרך SoraFS או שערים DA-aware,
   verificando הוכחות e emitindo pedidos de reparo quando replicas desaparecem.
6. **Governanca:** Parlamento e o comite de supervisao DA Aprovam Operatores,
   לוחות זמנים להשכרה e escalacoes de enforcement. Artefatos de governanca sao
   armazenados pela mesma rota DA para garantir transparencia do processo.

## תשובות ותשובות

Escala de impacto: **Critico** quebra seguranca/vivacidade do ledger; **אַלט**bloqueia backfill DA ou clientes; **Moderado** degrada qualidade mas permanece
החלמה; **באיקסו** אפייטו מוגבל.

| אטיו | תיאור | אינטגרייד | Disponibilidade | סודיות | שו"ת |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (נתחים + מניפסטים) | Blobs Taikai, lane e governanca armazenados em SoraFS | ביקורת | ביקורת | Moderado | DA WG / צוות אחסון |
| Manifests Norito DA | Metadata tipada descrevendo blobs | ביקורת | אלטו | Moderado | Core Protocol WG |
| התחייבויות דה בלוקו | CIDs + roots KZG dentro de blocos Nexus | ביקורת | אלטו | Baixo | Core Protocol WG |
| לוחות זמנים PDP/PoTR | Cadencia de enforcement para replicas DA | אלטו | אלטו | Baixo | צוות אחסון |
| Registry de Operadores | בדיקות אחסון ופוליטיקה | אלטו | אלטו | Baixo | מועצת ממשל |
| Registros de Rent e Incentivos | ספר חשבונות Entradas para rent DA e penalidades | אלטו | Moderado | Baixo | משרד האוצר |
| לוחות מחוונים דה observabilidade | SLOs DA, profundidade de replicacao, alertas | Moderado | אלטו | Baixo | SRE / צפיות |
| Intents de reparo | Pedidos para reidratar chunks ausentes | Moderado | Moderado | Baixo | צוות אחסון |

## יריבים וכוחות

| אטור | Capacidades | Motivacoes | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Submeter כתמים malformados, replay de manifests antigos, Tentar DoS אין בליעה. | Interromper משדר Taikai, injetar dados invalidos. | סם שואב פריבילגיאדות. |
| Nodo de armazenamento bizantino | Drop de replicas atribuidas, forjar הוכחות PDP/PoTR, coludir. | Reduzir retencao DA, evitar rent, reter dados. | Possui credenciais validas de operador. |
| Sequenciador comprometido | השמט מחויבויות, גושיות מעורפלות, מטא נתונים מחדש של קוביות. | הגשת עיניים של DA, חוסר עקביות. | Limitado pela maioria de consenso. |
| Operador interno | Abusar acesso de governanca, פוליטיקה מניפולרית דה retencao, vazar credenciais. | Ganho Economico, חבלה. | Acesso a infraestrutura חם/קר. |
| Adversario de Red | צמתים פרטיים, העתק אטרסאר, MITM injetar trafego. | זמינות רזולוציה, SLOs מדרדר. | קישורי Nao quebra TLS mas pode dropar/atrasar. |
| Atacante de observabilidade | לוחות מחוונים / התראות מניפולאריים, אירועי סופרמיר. | הפסקות עיניים DA. | בקש גישה או צינור טלמטריה. |

## Fronteiras de confianca- **Fronteira de ingress:** Cliente para extensao DA do Torii. בקש אישור
  בקשה, הגבלת שיעור e validacao de payload.
- **Fronteira de replicacao:** Nodos de storage trocam chunks e הוכחות. צמתי הפעלה
  se autenticam mutuamente mas podem se comportar de forma bizantina.
- **Fronteira do Ledger:** Dados de bloco commitados לעומת אחסון מחוץ לרשת.
  הסכמה מבטיחה אינטגריד, זמינות דורשת אכיפה מחוץ לרשת.
- **Fronteira de governanca:** מחליטים על ידי המועצה/הפרלמנט,
  orcamentos e slashing. Falhas aqui impactam diretamente o deploy de DA.
- **Fronteira de observabilidade:** Coleta de metrics/logs exportada para
  לוחות מחוונים / כלי התראה. התעסקות עם הפסקות או תקלות.

## Cenarios de ameaca e controls

### Ataques no caminho de ingestao

**תרחיש:** לקוחות מזדמנים מטעים תת-מטענים Norito פגמים או כתמים
superdimensionados para exaurir recursos או inerir metadata invalida.

**בקרות**
- Validacao de schema Norito com negociacao estrita de versao; דגלי rejeitar
  desconhecidos.
- מגבלת שיעור e autenticacao no endpoint de ingestao Torii.
- מגבלות את גודל הנתח e קידוד deterministico forcos pelo chunker SoraFS.
- Pipeline de admissao so persiste מתבטא apos checksum de integridade
  coincidir.
- הפעלה חוזרת של מטמון determinista (`ReplayCache`) rastreia janelas `(נתיב, עידן,
  רצף)`, להתמיד בסימני מים גבוהים בדיסקו, e rejeita duplicados/replays
  מיושנים; רתמות de propriedade e fuzz cobrem טביעות אצבע divergentes e
  envios fora de ordem. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuais**
- Torii לצרוך deve encadear או מטמון להשמעה חוזרת ולהתעדכן ולהמשיך לסמן את
  רצף durante reinicios.
- סכימות Norito DA agora possuem um fuzz harness dedicado
  (`fuzz/da_ingest_schema.rs`) לצורכי קידוד/פענוח; OS
  לוחות המחוונים של קוברטורה פיתחו התראה לסירוגין היעד.

### Retencao por inhibition de replicacao

**תרחיש:** Operadores de storage bizantinos aceitam סיכות mas dropam chunks,
passando desafios PDP/PoTR דרך respostas forjadas או colusao.

**בקרות**
- לוח הזמנים של PDP/PoTR ראה את המטענים של DA com cobertura por
  תקופה.
- Replicacao ריבוי מקורות com thresholds de quorum; o מתזמר detecta רסיסי
  faltantes e dispara reparo.
- Slashing de governanca vinculado a proofs falhas e replicas faltantes.
- Job de reconciliacao automatizado (`cargo xtask da-commitment-reconcile`) que
  השוואת קבלות de ingestao com התחייבויות DA (SignedBlockWire, `.norito` ou
  JSON), Emit Bundle JSON de evidencia para governanca, e falha em tickets
  faltantes ou divergentes para que Alertmanager page por mismission/tampering.**Lacunas residuais**
- O harness de simulacao em `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) exercita colusao e
  particao, validando que o לוח זמנים PDP/PoTR detecta comportamento bizantino
  de forma deterministica. המשך estendendo junto com DA-5 para cobrir novas
  שטחיות דה הוכחה.
- פוליטיקה דה פינוי בדרגה קרה לבקשת ביקורת מסלול assinado para evitar drops
  אנקוברטוס.

### התחייבויות מניפולאקאו

**סנריו:** Sequenciador comprometido publica blocos omitindo ou alterando
התחייבויות DA, causando falhas de להביא או חוסר עקביות לרמות הלקוחות.

**בקרות**
- Consenso cruza propostas de bloco com filas de submissao DA; עמיתים rejeitam
  propostas sem מחויבויות requeridos.
- לקוחות רמות אימות הכללה הוכחות לפני היציאה של ידיות אחזור.
- נתיב ביקורת השוואת קבלות de submissao com התחייבויות de bloco.
- Job de reconciliacao automatizado (`cargo xtask da-commitment-reconcile`) que
  השוואת קבלות de ingestao com התחייבויות DA (SignedBlockWire, `.norito` ou
  JSON), Emit Bundle JSON de evidencia para governanca, e falha em tickets
  faltantes ou divergentes para que Alertmanager page por mismission/tampering.

**Lacunas residuais**
- Coberto pelo job de reconciliacao + וו Alertmanager; פאקוטס דה גוברננקה
  agora ingerem o bundle JSON de evidencia for default.

### Particao de Red e Censura

**Cenario:** Adversario particiona a rede de replicacao, impedindo nodes de
אוטר chunks atribuidos או מגיב ל-PDP/PoTR.

**בקרות**
- דרישות הספקים מרובות אזורים מתחייבים למגוון רחב של תחומים.
- Janelas de desafio incluem jitter e fallback para canais de reparo fora de
  בנדה.
- לוחות מחוונים של observabilidade monitoram profundidade de replicacao, sucesso de
  desafios e latencia de fetch com thresholds de alerta.

**Lacunas residuais**
- Simulacoes de particao para eventos Taikai live ainda faltam; כה נחוצים
  בדיקות השרייה.
- Politica de reserva de banda de reparo ainda nao esta codificada.

### Abuso interno

**סנריו:** Operador com acesso ao manipula politicas de retencao,
רשימת היתרים של ספקים זדוניות, או התראות מעולות.

**בקרות**
- Acoes de governanca requerem assinaturas רישום רב-צדדי Norito
  נוטריזאדו.
- Mudancas de politica emitem eventos para monitoring e logs de arquivo.
- Pipeline de observabilidade aplica logs Norito צרף רק com hash שרשור.
- A automacao de revisao trimestral (`cargo xtask da-privilege-audit`) percorre
  diretorios de manifest/replay (mais paths fornecidos por operadores), marca
  entradas faltantes/nao diretorio/world-writable, emite bundle JSON assinado
  עבור לוחות מחוונים דה גוברננקה.

**Lacunas residuais**
- Evidencia de tamper עם לוחות המחוונים דורשים תמונות מצב של מתעסקים.

## Registro de Riscos Residais| ריסקו | הסתברות | אימפקטו | בעלים | Plano de mitigacao |
| --- | --- | --- | --- | --- |
| Replay de manifests DA antes do sequence cache DA-2 | פוסיבל | Moderado | Core Protocol WG | מטמון רצף מיושם + validacao de nonce em DA-2; adicionar testes de regressao. |
| Colusao PDP/PoTR quando >f nodes sao comprometidos | Improvavel | אלטו | צוות אחסון | Derivar novo schema de desafios com דגימה צולבת ספקים; validar via harness de simulacao. |
| Gap de auditoria de eviction cold-tier | פוסיבל | אלטו | SRE / צוות אחסון | Anexar רושם assinados e receipts on-chain para evictions; ניטור באמצעות לוחות מחוונים. |
| Latencia de deteccao de omissao de sequenciador | פוסיבל | אלטו | Core Protocol WG | `cargo xtask da-commitment-reconcile` לא ניתן להשוות קבלות לעומת התחייבויות (SignedBlockWire/`.norito`/JSON) ועמודי הממשל בכרטיסים משתנים. |
| Resiliencia a particao para streams Taikai בשידור חי | פוסיבל | ביקורת | רשתות TL | תרגילי מנהל; reservar banda de reparo; דוקומנטרי SOP de failover. |
| Deriva de privilegios de governanca | Improvavel | אלטו | מועצת ממשל | `cargo xtask da-privilege-audit` trimestral (מניפסט/שידור חוזר של dirs + תוספות נתיבים) com JSON assinado + gate de dashboard; ancorar artefatos de auditoria על השרשרת. |

## דרישות מעקב

1. סכימות פומביות Norito de ingestao DA e vetores de exemplo (carregado em
   DA-2).
2. Encadear או מטמון הפעלה חוזרת ב-DA do Torii e persistir cursores de
   רצף durante reinicios de nodes.
3. **Concluido (2026-02-05):** O רתמה של סימולאקאו PDP/PoTR agora exercita
   colusao + particao com modelagem de backlog QoS; ויזה
   `integration_tests/src/da/pdp_potr.rs` (com בודק אותם
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para a implementacao e
   resumos deterministas capturados abaixo.
4. **Concluido (2026-05-29):** `cargo xtask da-commitment-reconcile` השוואה
   Receipts de ingestao com התחייבויות DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, e esta ligado a
   Alertmanager/pacotes de governanca para alertas de emmission/tabuling
   (`xtask/src/da.rs`).
5. **Concluido (2026-05-29):** `cargo xtask da-privilege-audit` percorre o סליל
   de manifest/replay (mais paths fornecidos por operadores), marca entradas
   faltantes/nao diretorio/world-writable, e produz bundle JSON assinado para
   לוחות מחוונים/revisoes de governanca (`artifacts/da/privilege_audit.json`),
   fechando a lacuna de automacao de acesso.

**Onde olhar a seguir:**- O replay cache e a persistencia de cursores aterrissaram em DA-2. Veja א
  implementacao em `crates/iroha_core/src/da/replay_cache.rs` (logica do cache)
  e a integracao Torii em `crates/iroha_torii/src/da/ingest.rs`, que encadeia checks de
  טביעת אצבע דרך `/v1/da/ingest`.
- כמו סימולציות של הזרמת PDP/PoTR או תרגול באמצעות זרם הוכחה או לרתום
  em `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo fluxos de requisicao
  PoR/PDP/PoTR e cenarios de falha animados no modelo de ameacas.
- תוצאות קיבולת ותיקון להשרות חיים
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  Sumeragi mais ampla e acompanhada em `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluidas). Esses artefatos capturam os drills de longa
  duracao referenciados no registro de riscos residuais.
- A automacao de reconciliacao + privilege-audit vive em
  `docs/automation/da/README.md` e nos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; להשתמש
  כפי שאמר פדראו יבפח `artifacts/da/` ao anexar evidencia a pacotes de
  governanca.

## Evidencia de simulacao e modelagem QoS (2026-02)

עבור מעקב אחר DA-1 #3, קוד תקינה של רתמת סימולאקאו PDP/PoTR
determinista sob `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O רתום aloca nodes em
tres regioes, injeta particoes/colusao תואמים כמפת הדרכים,
acompanha איחור PoTR, e alimenta um modelo de backlog de reparo que reflete o
orcamento de reparo לעשות tier חם. ברירת המחדל של רודאר (12 עידנים, 18 עידנים
PDP + 2 Janelas PoTR por epoch) produziu as seguintes metricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| מטריקה | חיל | Notas |
| --- | --- | --- |
| Falhas PDP detectadas | 48 / 49 (98.0%) | Particoes ainda disparam deteccao; uma falha nao detectada vem de jitter honesto. |
| Latencia media de deteccao PDP | 0.0 עידנים | Falhas surgem dentro do epoch de origem. |
| Falhas PoTR detectadas | 28 / 77 (36.4%) | Deteccao dispara quando um node perde >=2 janelas PoTR, deixando a maioria dos eventos no registro de riscos residuais. |
| Latencia media de deteccao PoTR | 2.0 עידנים | קורספונדה אאו לימיאר דה אטראסו דה דויס epochs embutido na escalacao de arquivo. |
| Pico da fila de reparo | 38 מניפסטים | צבר הצטברות הצטברות חלקים אמפילהם מאיס רפידו que quatro reparos disponiveis por epoch. |
| Latencia de resposta p95 | 30,068 אלפיות השנייה | Reflete a janela de desafio de 30 s com jitter de +/-75 ms aplicado no sampling QoS. |
<!-- END_DA_SIM_TABLE -->

Esses פלט agora alimentam os prototipos de dashboard DA e satisfazem os
קריטריונים של aceitacao של "רתמת הדמיה + דוגמנות QoS"
אין מפת דרכים.

A automacao agora vive por tras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que chama o רתמה compartilhado e emite Norito JSON para
`artifacts/da/threat_model_report.json` לפי ברירת מחדל. Jobs noturnos consomem este
arquivo para atualizar as matrizes Nest Documento e Alertar sobre deriva em

taxas de deteccao, filas de reparo או דגימות QoS.בצע את `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, e reescreve esta secao via
`scripts/docs/render_da_threat_model_tables.py`. O espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) e atualizado no mesmo passo para que
כמו duas copias fiquem em sync.