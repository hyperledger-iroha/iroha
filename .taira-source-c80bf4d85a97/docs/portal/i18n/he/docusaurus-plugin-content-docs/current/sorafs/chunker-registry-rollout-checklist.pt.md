---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-rollout-checklist
כותרת: רשימת רשימת פתיחה לרישום chunker da SoraFS
sidebar_label: רשימת בדיקה להפצה של chunker
תיאור: Plano de rollout passo a passo para atualizacoes do registro de chunker.
---

:::שים לב Fonte canonica
Reflete `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas as copias sincronizadas.
:::

# רשימת רשימת הפעלה לרישום SoraFS

Este checklist captura os passos necessarios para promotor um novo perfil de chunker
או צרור כניסה דה פרודור דה רוויסאו פאר producao depois que o charter
de governanca עבור ratificado.

> **Escopo:** אפליקציית יישום היום כפי מהדורות que modificam
> `sorafs_manifest::chunker_registry`, מעטפות כניסה לספק, חבילות או
> de fixtures canonicos (`fixtures/sorafs_chunker/*`).

## 1. Validacao לפני הטיסה

1. מכשירי Regenere ו-verifique determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. אשר את que os hashes de determinismo em
   `docs/source/sorafs/reports/sf1_determinism.md` (או רלטוריו של פרפיל
   רלוונטי) batem com os artefatos regenerados.
3. Garanta que `sorafs_manifest::chunker_registry` compila com
   `ensure_charter_compliance()` ביצוע:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. השג את התיק הבא:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atas do conselho em `docs/source/sorafs/council_minutes_*.md`
   - Relatorio de determinismo

## 2. Sign-off de governanca

1. נציג או יחס לעשות קבוצת עבודה של כלי עבודה e o digest da proposta ao
   פאנל תשתיות פרלמנט סורה.
2. Registre detalhes de aprovacao em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. פרסום המעטפה assinado pelo Parlamento junto com os fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. בדוק את המעטפה המתאימה דרך אחזור עוזר הממשל:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. הפעל אותם בימוי

עיין ב[playbook de manifest em staging](./staging-manifest-playbook) para um
passo a passo detalhado.

1. השתלת Torii com discovery `torii.sorafs` אכיפת קבלה
   ligado (`enforce_admission = true`).
2. מעטפות קבלה של ספק שירותי Envie OS aprovados para o diretorio de registry
   de staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. תעמולה של ספקי פרסומות של Verifique que באמצעות API de Discovery:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. תרגיל את נקודות הקצה של המניפסט/תוכנית com headers de governanca:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. אשר את לוחות המחוונים של טלמטריה (`torii_sorafs_*`) e regras de alerta
   reportam o novo perfil sem errors.

## 4. הפעל אותם

1. Repita os passos de staging nos nodes Torii de producao.
2. Anuncie a janela de ativacao (נתונים/הורה, תקופת חסד, פלנו דה החזרה) לא
   canais de operadores e SDK.
3. מיזוג או מחלוקת יחסי ציבור:
   - מתקנים e envelope atualizados
   - Mudancas na documentacao (referencias ao charter, relatorio de determinismo)
   - רענון מפת הדרכים/סטטוס
4. Tagueie a release e arquive os artefatos assinados para provenance.

## 5. אודיטוריה לאחר השקה1. לכידת מדדים פינאיים (ספירות גילוי, taxa de sucesso de fetch, היסטוגרמות
   דה שגיאה) 24 שעות לאחר השקה.
2. להטמיע את `status.md` באמצעות רזומה וקישור ליחסי דטרמיניזם.
3. Registre tarefas de acompanhamento (לדוגמה, orientacao adicional para authoring
   de perfis) em `roadmap.md`.