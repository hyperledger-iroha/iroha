---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: storage-capacity-marketplace
כותרת: Marketplace de capacidade de armazenamento SoraFS
sidebar_label: Marketplace de capacidade
תיאור: Plano SF-2c עבור השוק של ה-capacidade, סדרת העתקים, טלמטריה ו-hooks de governanca.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os locais alinhados enquanto a documentacao herdada permanecer ativa.
:::

# Marketplace de capacidade de armazenamento SoraFS (Rascunho SF-2c)

O פריט SF-2c לעשות מפת הדרכים היכרות עם השוק שליטת ספקים
armazenamento declaram capacidade comprometida, recebem ordens de replicacao e
ganham fees proporcionais עניין של disponibilidade. Este documento delimita
os entregaveis exigidos para a primeira release e os divide em trilhas acionavis.

## אובייקטיביות

- Expressar compromissos de capacidade (bytes totalais, limites por lane, expiracao)
  הם פורמטים אימות צרכניים לממשלה, תחבורה SoraNet ו Torii.
- Alocar pins entre providers de acordo com capacidade declarada, stake e
  restricoes de politica mantendo comportamento deterministico.
- Medir entrega de armazenamento (סוצץ דה רפליקאו, זמן פעולה, הוכחות דה
  אינטגריד) e exportar telemetria para distribuicao de fees.
- Prover processos de revogacao e disputa para que providers desonestos sejam
  penalisados ou removidos.

## Conceitos de dominio

| קונסיטו | תיאור | Entregavel inicial |
|--------|--------|------------------------|
| `CapacityDeclarationV1` | מטען Norito פרסם את זיהוי הספק, תמיכה בפרופיל של chunker, פתרונות GiB, מגבלות מסלול, רמזים לתמחור, פשרה על הימור ותפוגה. | Esquema + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucao emitida pela governanca que atribui um CID de manifest a um ou mais ספקים, incluindo nivel de redundancia e metricas SLA. | Esquema Norito compartilhado com Torii + API של חוזה חכם. |
| `CapacityLedger` | רישום על-שרשרת/מחוץ לשרשרת que rastreia declaracoes de capacidade ativas, orders de replicacao, מדדי ביצועים ואגרות צבירת. | מודול חוזה חכם או תמצית שירות מחוץ לשרשרת com תמונת מצב דטרמיניסטית. |
| `MarketplacePolicy` | פוליטיקה דה גוברננקה מגדירה את הימור המינימלי, דרישות האודיטוריה והקימורים של הפנדלאדה. | מבנה התצורה של `sorafs_manifest` + תעודה ממשלתית. |

### Esquemas implementados (סטטוס)

## Desdobramento de trabalho

### 1. סכימת רישום אלקטרונית| טארפה | Responsavel(is) | Notas |
|------|----------------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | צוות אחסון / ממשל | Usar Norito; כולל גרסה סמנטית e references de capacidade. |
| מודולים מבצעיים של מנתח + validador em `sorafs_manifest`. | צוות אחסון | תעודות איפור מונוטוניות, מגבלות קיבולת, דרישות סיכון. |
| Estender metadata do chunker registry com `min_capacity_gib` por perfil. | Tooling WG | Ajuda מציע דרישות מינימליות לחומרה לפי פרופיל. |
| רדיר את המסמכים `MarketplacePolicy` מעקות בטיחות של קבלה ולוח שנה. | מועצת ממשל | Publicar em docs junto com defaults de politica. |

#### Definicoes de schema (implementadas)- `CapacityDeclarationV1` תופסים פשרות עבור הספק, כולל ידיות קנוניות לעשות chunker, הפניות לנפח, אופציות כובעים לנתיב, רמזים לתמחור, אימות מידע ומטא נתונים. A validacao garante הימור נאו אפס, מטפל canonicos, כינוי deduplicados, caps por lane dentro do total declarado e contabilidade de GiB monotonicamente crescente. [crates/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` vincula מציגה atribuicoes emitidas pela governanca com metas de redundancia, limiares de SLA e garantias por atribuicao; validadores impoem מטפל ב-canonicos, ספקים ייחודיים והגבלת דדליין לפני Torii או רישום ingerirem a orderem. [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` תמונת מצב מפורשת בתקופה האחרונה (GiB declarados vs utilizados, contadores de replicacao, percentuais de uptime/PoR) que alimentam and distribuicao desers. Checagens de limites mantem utilizacao dentro das declaracoes e percentuais dentro de 0-100%. [crates/sorafs_manifest/src/capacity.rs:476]
- עוזרים להשוות (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/atribuicao/SLA) validacao deterministica de chaves e report de erro reutilizavel por CI e tooling downstream. [crates/sorafs_manifest/src/capacity.rs:230]
- `PinProviderRegistry` תצוגה מקדימה או תמונת מצב על-שרשרת דרך `/v2/sorafs/capacity/state`, שילוב של ספקים והכרזות בעלות פנקס חשבונות לפי מאיו של Norito JSON קובע. [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- אכיפת אכיפת קוברטורה דה מטפלת ב-canonicos, deteccao de duplicados, limites por lane, guardas de atribuicao de replicacao e checks de range de telemetria para que regressoes aparecam imediatamente no CI. [crates/sorafs_manifest/src/capacity.rs:792]
- כלי עבודה עבור מפעילים: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` המרת מפרט למידע על מטענים Norito canonicos, blobs base64 ו-JSON קורות חיים עבור מתקנים מכינים להפעלה של `/v2/sorafs/capacity/declare`, I100NI000 חזרות validacao מקומי. [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] Fixtures de referencia vivem em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) I0180 gerados via.

### 2. Integracao do plano de control| טארפה | Responsavel(is) | Notas |
|------|----------------|-------|
| מטפלים Adicionar Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` מטענים com Norito JSON. | צוות Torii | Espelhar logica do validador; reutilizar helpers Norito JSON. |
| תצלומי מצב של Propagar `CapacityDeclarationV1` עבור מטא נתונים לעשות לוח תוצאות לעשות מתזמר e planos de fetch do gateway. | צוות Tooling WG / תזמורת | Estender `provider_metadata` com referencias de capacidade עבור ציון ריבוי מקורות הפוגה מגבלות על הנתיב. |
| רמזים על כשל אוריינטרי עם לקוחות מתזמר/שער עבור אוריינטרים. | Networking TL / Gateway צוות | הו בונה לעשות לוח תוצאות לצרוך ordens assinadas pela governanca. |
| כלי עבודה CLI: estender `sorafs_cli` com `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Fornecer JSON קבע + לוח התוצאות. |

### 3. Politica de marketplace e governanca

| טארפה | Responsavel(is) | Notas |
|------|----------------|-------|
| Ratificar `MarketplacePolicy` (מינימום הימור, רב-תכלית, קדנציה של אודיטוריה). | מועצת ממשל | Publicar em docs, capturar historico de revisoes. |
| Adicionar hooks de governanca para que o Parlamento possa aprovar, renovar e revogar declaracoes. | מועצת ממשל / צוות חוזה חכם | Usar eventos Norito + ingestao de manifests. |
| לוח שנה מיושם (reducao de fees, slashing de bond) ligado a violacoes de SLA telegrafadas. | מועצת ממשל / האוצר | Alinhar com outputs de settlement do `DealEngine`. |
| תהליך תיעודי של מחלוקת ומטריז דה אסקלונמנטו. | מסמכים / ממשל | קישורים ao runbook de disputa + helpers de CLI. |

### 4. מדידה והפצת עמלות

| טארפה | Responsavel(is) | Notas |
|------|----------------|-------|
| הרחב את המדידה ל-Torii לעזר `CapacityTelemetryV1`. | צוות Torii | תוקף GiB-hour, PoR לאחר, זמן פעולה. |
| אטואליזר צינור מדידה לעשות `sorafs_node` עבור שימוש דיווח לפי סדר + SLA אסטטיסטי. | צוות אחסון | Alinhar com ordens de replicacao e handles de chunker. |
| צינור ההתיישבות: ממיר טלמטריה + רפליקציית מסמכים ב-XOR. | צוות אוצר / אחסנה | Conectar ao Deal Engine / יצוא האוצר. |
| ייצוא לוחות מחוונים/אזהרות לביצוע מדידה (פיגור של אינסטה, טלמטריה מיושן). | צפייה | Estender pack de Grafana רפרנס ל-SF-6/SF-7. |- Torii agora expoe `/v2/sorafs/capacity/telemetry` e `/v2/sorafs/capacity/state` (JSON + Norito) para que Operatores enviem snapshots de telemetria por epoca e inspetores or audit or canonicoempo ou auditores de evidencia. [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- א integracao `PinProviderRegistry` garante que ordens de replicacao sejam acessiveis pelo mesmo point end; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) agora validam e publicam telemetria a partir de runs automatizados com hashing deterministico e resolucao de aliases.
- תמונות Snapshots de metering produzem entradas `CapacityTelemetrySnapshot` fixadas ao snapshot `metering`, e יצוא Prometheus alimentam o board Grafana pronto para08NI000007 para 08NI00 em quipes I0 faturamento monitorem acumulacao de GiB-שעה, עמלות ננו-SORA פרויקטים ותאימות של SLA בזמן אמת. [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- קוואנדו או החלקה של מדידה קיימת, או תמונת מצב הכוללת `smoothed_gib_hours` ו-`smoothed_por_success_bps` עבור מפעילי השוואת ערכים ב-EMA נגד תשלומים ברוטוס בארה"ב. [crates/sorafs_node/src/metering.rs:401]

### 5. Tratamento de disputa e revogacao

| טארפה | Responsavel(is) | Notas |
|------|----------------|-------|
| הגדרת מטען `CapacityDisputeV1` (reclamante, evidencia, provider alvo). | מועצת ממשל | סכימה Norito + validador. |
| תמיכה ב-CLI למתן מענה למחלוקת (com anexos de evidencia). | Tooling WG | Garantir hashing deterministico do bundle de evidencia. |
| Adicionar בודקים אוטומטיים ל-Violacoes SLA repetidas (auto-escalada para disputa). | צפייה | Limiares de alerta e hooks de governanca. |
| ספר תעודה דה רבוגאסאו (periodo de graca, evacuacao de dados pinados). | צוות מסמכים / אחסון | קישורים או דוק דה פוליטיקה e runbook de operador. |

## Requisitos de testes e CI

- Testes unitarios para todos os novos validadores de schema (`sorafs_manifest`).
- Testes de integracao que simulam: declaracao -> ordem de replicacao -> מדידה -> תשלום.
- זרימת עבודה של CI להכרזות מחודשות/טלמטריה של קפאצידה e garantir que as assinaturas permanecam sincronizadas (estender `ci/check_sorafs_fixtures.sh`).
- Testes de carga para a API do registry (10k ספקים דומים, 100k סדרות).

## לוחות מחוונים של Telemetria e

- לוח המחוונים Paines de:
  - Capacidade declarada לעומת ספק שירות.
  - Backlog de ordens de replicacao e atraso medio de atribuicao.
  - עמידה ב-SLA (% זמן פעולה, taxa de sucesso PoR).
  - צבירה של עמלות ועונשים לארץ.
- התראות:
  - ספק abaixo da capacidade minima comprometida.
  - Ordem de replicacao travada > SLA.
  - Falhas אין צינור דה מדידה.

## Entregaveis de documentacao- Guia de Operator להכרזה על התקדמות, חידוש פשרה ושימוש במוניטור.
- Guia de governanca para aprovar declaracoes, emitir ordens e lidar com disputas.
- Referencia de API para point de capacidade e formato de ordem de replicacao.
- שאלות נפוצות לגבי מפתחים בשוק.

## רשימת רשימת מוכנות GA

O item de roadmap **SF-2c** bloqueia o השקה עם producao com base em evidencia concreta
sobre contabilidade, tratamento de disputa e onboarding. השתמש ב-os artefatos abaixo para
manter os criterios de aceitacao em sync com a implementacao.

### Contabilidade noturna e reconciliacao XOR
- ייצוא או תמונת מצב של קפאצידה או ייצוא של ספר חשבונות XOR למען מסמה ג'אנלה,
  depois לבצע:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O helper sai com codigo nao zero em settlements ou penalidades ausentes/excessivas e emite
  um resumo Prometheus עם קובץ טקסט פורמט.
- O alerta `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`)
  פערי דיווחים שונים; לוחות מחוונים ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- ארכיון או resumo JSON e hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  junto com pacotes de governanca.

### Evidencia de disputa e slashing
- ארגנו מחלוקות באמצעות `sorafs_manifest_stub capacity dispute` (בדיקות:
  `cargo test -p sorafs_car --test capacity_cli`) para manter מטענים canonicos.
- בצע את `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e כסוויטות
  de penalidade (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar
  que disputas e slashes fazem replay deterministico.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia e escalonamento;
  linke aprovacoes de strike no relatorio de validacao.

### כניסת ספקים ליציאה מבחני עשן
- Regenere artefatos de declaracao/telemetria com `sorafs_manifest_stub capacity ...` e rode os
  tests de CLI antes da submissao (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Submeta באמצעות Torii (`/v2/sorafs/capacity/declare`) ו-Ccapture `/v2/sorafs/capacity/state` mais
  צילומי מסך עושים Grafana. Siga o fluxo de saida em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Arquive artefatos assinados e outputs de reconciliacao dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias e sequenciamento

1. סיום SF-2b (פוליטיקה דה אדמיסאו) - או השוק תלוי בשירותי הספקים.
2. סכימה יישומית + רישום (este doc) ante da integracao Torii.
3. צינור מלא של מדידת תשלומים לפני תשלומים.
4. גמר אטאפה: הפיקוח על דמי שליטה
   מדידה forem verificados em staging.

O progresso deve ser rastreado no map com referencias a este documento. לממש o
מפת הדרכים assim que cada secao principal (סכמה, plano de control, integracao, מדידה,
tratamento de disputa) תכונת סטטוס atingir הושלמה.