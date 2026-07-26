---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-refactor-plan
כותרת: Plano de refatoracao do Ledger Sora Nexus
תיאור: Espelho de `docs/source/nexus_refactor_plan.md`, detalhando o trabalho de limpeza por fases para a base de codigo do Iroha 3.
---

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/nexus_refactor_plan.md`. Mantenha as duas copias alinhadas ate que a edicao multilingue chegue ao פורטל.
:::

# Plano de refatoracao do ספר חשבונות Sora Nexus

Este Documento Captura או מפת דרכים מיידית עבור Refactor עבור Sora Nexus Ledger ("Iroha 3"). ניתן לשחזר את הפריסה באופן אוטומטי ולשמור על ריגרסיה של הנהלת חשבונות בראשית/WSV, קונצנזו Sumeragi, טריגרים של חוזים חכמים, ייעוץ לתמונות, כריכות של מצביע מארח-ABI ו-codec Norito. O objetivo e convergir para uma arquitetura coerente e testavel sem tentar entregar todas as correcoes em um unico patch monolitico.

## 0. Principios guia
- Preservar comportamento determinista em heterogeneo חומרה; usar aceleracao apenas באמצעות דגלים תכונה opt-in com fallbacks identicos.
- Norito e a camada de serializacao. Qualquer mudanca de estado/schema deve כולל את הבדיקות הלוך ושוב Norito קידוד/פענוח ותקשורת.
- A configuracao flui por `iroha_config` (משתמש -> בפועל -> ברירות מחדל). Remover מחליף אד-הוק דה ambiente dos paths de producao.
- A politica ABI permanece V1 e inegociavel. המארחים מפתחים סוגים של מצביעים/syscalls desconhecidos de forma deterministica.
- `cargo test --workspace` e os golden tests (`ivm`, `norito`, `integration_tests`) continuam como gate base para cada marco.

## 1. תמונת מצב של מאגר טופולוגיות
- `crates/iroha_core`: atores Sumeragi, WSV, loader de genesis, צינורות (שאילתה, שכבת על, נתיבי zk), דבק מארח חוזים חכמים.
- `crates/iroha_data_model`: schema autoritativo para dados e queries on-chain.
- `crates/iroha`: API של לקוחות ארה"ב עבור CLI, בדיקות, SDK.
- `crates/iroha_cli`: CLI de operadores, ממשקי API של מספרי API ו-`iroha`.
- `crates/ivm`: VM de bytecode Kotodama, נקודות כניסה של אינטגראקאו מצביע-ABI לארח.
- `crates/norito`: codec de serializacao com מתאמים JSON e backends AoS/NCB.
- `integration_tests`: קביעות חוצה רכיבים cobrindo genesis/bootstrap, Sumeragi, טריגרים, paginacao וכו'.
- Docs ja delineiam metas do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementacao esta fragmentada e parcialmente defasada ao relago.

## 2. Pilares de refatoracao e marcos### שלב A - Fundacoes e observabilidade
1. **Telemetria WSV + תמונת מצב**
   - Estabelecer uma API canonica de snapshots em `state` (תכונה `WorldStateSnapshot`) ארה"ב לשאילתות, Sumeragi ו-CLI.
   - Usar `scripts/iroha_state_dump.sh` ליצירת צילומי מצב קבועים דרך `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refatorar a ingestao de genesis para flair por um unico pipeline com Norito (`iroha_core::genesis`).
   - Adicionar cobertura de integracao/regressao que reprocessa genesis mais o primeiro bloco e afirma roots WSV identicos entre arm64/x86_64 (acompanhado em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **בדיקות de fixity cross-arte**
   - הרחב את `integration_tests/tests/genesis_json.rs` ל-WSV, צינור ו-ABI עם רתמה יחידה.
   - Introduzir um scaffold `cargo xtask check-shape` עבור פאניקה עם סחיפה של סכימה (ללא צבר של כלים ב-DevEx; עם פריט פעולה עם `scripts/xtask/README.md`).

### שלב B - WSV e superficie de queries
1. **Transacoes de state storage**
   - Colapsar `state/storage_transactions.rs` הם מתאם טרנסאקציונלי que imponha ordenacao de commit e deteccao de conflitos.
   - בדיקות יחידה מאורגנות אימות של נכסים/עולם/מפעילים את החזרה לאחור.
2. **רפיטור לעשות מודלים של שאילתות**
   - מעביר את הלוגיקה/הסמן לרכיבי שימוש חוזרים ב-`crates/iroha_core/src/query/`. Alinhar representacoes Norito em `iroha_data_model`.
   - שאילתות תמונת מצב נוספות עבור טריגרים, נכסים ותפקידים com ordenacao determinista (acompanhado via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura atual).
3. **קונסיסטנציה של תמונות מצב**
   - Garantir que o CLI `iroha ledger query` השתמש o mesmo caminho de snapshot que Sumeragi/מחזירים.
   - בדיקות רגרסאו של תמונת מצב ללא CLI vivem em `tests/cli/state_snapshot.rs` (לנטוס לריצות תכונה מגודרות).

### Fase C - Pipeline Sumeragi
1. **Topologia e gestao de epocas**
   - Extrair `EpochRosterProvider` עבור תכונה com יישום הבסיס לתמונות בזק של WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` מציעים את הקונסטרוטור הפשוטים והאמיגובלים עבור ספסלים/בדיקות.
2. **Simplificacao do fluxo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` עם מודולים: `pacemaker`, `aggregation`, `availability`, `witness` comartilhodos comartilhados sob `availability`.
   - הודעה מחליפה העוברת אד-הוק למעטפות Norito tipados e introduzir בדיקות נכסים de view-change (acompanhado no backlog de mensageria Sumeragi).
3. **נתיב/הוכחה Integracao**
   - Alinhar lane הוכחות com מחויבויות de DA e garanter que o gating de RBC seja uniforme.
   - בדיקת אינטגרציה מקצה לקצה `integration_tests/tests/extra_functional/seven_peer_consistency.rs` אימות או בדיקה עם RBC.### שלב D - חוזים חכמים ומארח pointer-ABI
1. **Auditoria da fronteira מארח**
   - Consolidar Verificacoes de pointer-type (`ivm::pointer_abi`) ו-Adaptadores de Host (`iroha_core::smartcontracts::ivm::host`).
   - כמו ציפיות של טבלת מצביעים e os bindings de host manifest sao cobertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que exercitam os mappings TLV golden.
2. **Sandbox de execucao de triggers**
   - Refatorar triggers para roder דרך um `TriggerExecutor` comum que impoe gas, validacao de pointers e journaling de eventos.
   - Adicionar tests de regressao para triggers de call/time cobrindo paths de falha (acompanhado via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento de CLI e client**
   - Garantir que operacoes do CLI (`audit`, `gov`, `sumeragi`, `ivm`) usem funcos compartilhadas do cliente `iroha` para drifte.
   - בדיקות של תמונות JSON ל-CLI ב-`tests/cli/json_snapshot.rs`; mantenha-os atualizados para que a saida dos comandos להמשיך אלינהד ל-JSON canonica.

### Fase E - Hardening do codec Norito
1. **רישום סכימות**
   - צור סכימת רישום Norito ב-`crates/norito/src/schema/` עבור קידודים קנוניים עבור ליבת טיפוס.
   - Adicionados doc tests verificando a codificacao de payloads de exemplo (`norito::schema::SamplePayload`).
2. **Refresh de golden fixtures**
   - Atualizar os fixtures golden em `crates/norito/tests/*` para coincidir com o new schema WSV quando or refactor pousar.
   - `scripts/norito_regen.sh` regenera os golden JSON Norito de forma deterministica באמצעות o helper `norito_regen_goldens`.
3. **Integracao IVM/Norito**
   - אימות המניפסטים ברצף Kotodama מקצה לקצה באמצעות Norito, מבטיח מצביע מטא נתונים של ABI עקבי.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantem paridade Norito קידוד/פענוח para Manifestes.

## 3. Preocupacoes transversais
- **Estrategia de testes**: מבחני יחידה לקידום שלב -> מבחני ארגז -> מבחני אינטגרציה. בדיקות que falham capturam regressoes atuais; בדיקות novos impedem que retornem.
- **Documentacao**: Apos cada fase, atualizar `status.md` elevar itens em aberto para `roadmap.md` כדי להסיר את התוצאות.
- **מדדי ביצועים**: ספסלי Manter existentes em `iroha_core`, `ivm` e `norito`; adicionar medicoes base pos-refactor para validar que nao ha regressoes.
- **דגלים מאפיינים**: Manter מחליף את ה-Nivel de crate apenas ל-backends que exigem toolchains externos (`cuda`, `zk-verify-batch`). נתיבים SIMD של מעבד וסימפר בנוי ובחירה בזמן ריצה; fornecer fallbacks escalares deterministas para hardware nao supportado.## 4. Acoes imediatas
- פיגומים da Fase A (תכונת תמונת מצב + חיווט טלמטריה) - ver tarefas acionavis nas atualizacoes do מפת דרכים.
- A Auditoria recente de defeitos para `sumeragi`, `state` e `ivm` revelou os seguintes destaques:
  - `sumeragi`: קצבאות להגנה על קוד מת או שידור של הצגת שינויי תצוגה, גרסה חוזרת של VRF ו-Exportacao de telemetria EMA. Eles permanecem gated ate que a simplificacao do fluxo de consenso da Fase C e os entregaveis de integracao lane/proof sejam entregues.
  - `state`: a limpeza de `Cell` e o roteamento de telemetria entram na trilha de telemetria WSV da Fase A, enquanto as notas de SoA/מקביל-apply entram no backlog de otimizacao Fase da pipeline.
  - `ivm`: exposicao לעשות החלפת CUDA, validacao de envelopes e cobertura Halo2/Metal mapeiam para o trabalho de host-boundary da Fase D mais o tema transversal de aceleracao GPU; גרעינים קבועים ללא צבר דידיקדו של GPU אכלו estarem prontos.
- הכן את רזומה חוצה צוות של ה-RFC עבור ההרשמה לפני ההרשמה הקודמת.

## 5. Questoes em aberto
- RBC deve permanecer optional alem de P1, ou e obrigatorio para lanes do book Nexus? בקש את החלטת בעלי העניין.
- Impomos grupos de composabilidade DS em P1 או os mantemos desativados אכלו que as lane proofs amadurecam?
- Qual e o local canonico parametros ML-DSA-87? מועמד: ארגז נובו `crates/fastpq_isi` (criacao pendente).

---

_Ultima atualizacao: 2025-09-12_