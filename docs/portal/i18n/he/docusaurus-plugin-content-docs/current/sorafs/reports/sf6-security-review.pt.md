---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Revisao de seguranca SF-6
תקציר: Achados e itens de acompanhamento da avaliacao independente de signless keyless, proof streaming e pipelines de envio de manifests.
---

# Revisao de seguranca SF-6

**Janela de avaliacao:** 2026-02-10 -> 2026-02-18  
**Lideres da revisao:** גילדת הנדסת אבטחה (`@sec-eng`), קבוצת עבודה של כלי עבודה (`@tooling-wg`)  
**Escopo:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), ממשקי API להזרמת הוכחה, טיפול במניפסט Torii, אינטגראקאו Sigstore/OIDC, ווי שחרור CI.  
** חפצים:**  
- בדיקות Fonte do CLI e (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii מטפלי מניפסט/הוכחה (`crates/iroha_torii/src/sorafs/api.rs`)  
- אוטומציה של שחרור (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- רתמת זוגיות דטרמיניסטית (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS דו"ח זוגיות של תזמורת GA](./orchestrator-ga-parity.md))

## מתודולוגיה

1. **סדנאות דוגמנות איומים** mapearam capacidades de ataque עבור תחנות עבודה של מפתחים, מערכות CI e nodes Torii.  
2. **סקירת קוד** מתמקדת במשטחי אישורים (OIDC החלפת אסימונים, חתימה ללא מפתח), validacao de Norito מראה את הזרמת הוכחה בלחץ אחורי.  
3. **בדיקות דינמיות** מתקן ביצוע מחדש מציג מצבי כישלון סימולריים (שידור חוזר של אסימון, שיבוש מניפסט, זרימת הוכחה truncados) usando parity harness e fuzz drives sob medida.  
4. **בדיקת תצורה** תקפות ברירות המחדל `iroha_config`, דגל CLI טיפול בסקריפטים של שחרור e para garantir runs deterministicas e auditavis.  
5. **ראיון תהליכים** מאשר את זרימת התיקון, נתיבי הסלמה וקבלת ראיות ביקורת com os שחרור הבעלים עושים Tooling WG.

## סיכום ממצאים| תעודת זהות | חומרה | אזור | מוצא | רזולוציה |
|----|--------|------|--------|------|
| SF6-SR-01 | גבוה | חתימה ללא מפתח | ברירת המחדל של קהל האסימון OIDC מבוססת על תבניות CI, com risco de replay cross-tenant. | הוראות אכיפה ברורות של `--identity-token-audience` עם ווי שחרור ותבניות CI ([תהליך שחרור](../developer-releases.md), `docs/examples/sorafs_ci.md`). O CI agora falha quando a audience e omitida. |
| SF6-SR-02 | בינוני | הזרמת הוכחה | נתיבי לחץ אחורי מסייעים לחצני מנויים מוגבלים, ומאפשרים מיצוי זיכרון. | `sorafs_cli proof stream` גדלי ערוץ אפליקציית מוגבלות com truncation deterministico, registra Norito סיכומים eborta o stream; o Torii מראה foi atualizado עבור נתחי תגובה מוגבלים (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | בינוני | הגשת מניפסט | O CLI aceitava גילויים אשר מוודאים תוכניות נתחים משובצות quando `--plan` estava ausente. | `sorafs_cli manifest submit` מאזן מחדש והשוואה למכונית מעכלת ומנוסה que `--expect-plan-digest` ראה פורנקידו, חזר על אי התאמה ורמזים לתיקון. בדיקות cobrem casos de sucesso/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | נמוך | מסלול ביקורת | O שחרור רשימת התיוג נאו tinha um חתום יומן אישורים עבור רוויזאו דה סגורנקה. | Foi adicionada uma secao em [תהליך שחרור](../developer-releases.md) exigindo anexar hashes do review memo e URL do ticket de sign-off antes de GA. |

Todos os achados high/medium foram corrigidos durante a janela de revisao e validados pelo parity harness existente. ננהום נושא ביקורת קביעות סמויה.

## אימות בקרה

- **היקף אישורים:** תבניות CI agora exigem audience e explicitos המנפיק; o CLI e o שחרור עוזר falham rapido a menos que `--identity-token-audience` acompanhe `--identity-token-provider`.  
- **שידור חוזר דטרמיניסטי:** בודקים את הגשת המניפסטים/שליליות של קוברם זרמים חיוביים/שליליים של הגשת מניפסטים, ודאי מתארים תקצירים לא תואמים ממשיכים.  
- **הוכחה זרימה חוזרת-לחץ:** Torii agora faz stream de itens PoR/PoTR em canais limitados, e o CLI reten apenas חביון דגימות truncados + cinco כשל דוגמאות, prevenindo crescimento sem limite e mantendo summaries deistic.  
- **ניתן לצפייה:** הוכחה מונים זרימה (`torii_sorafs_proof_stream_*`) e CLI סיכומים תפסו סיבות לביטול, רציף ביקורת לחם aos מפעילים.  
- **תיעוד:** מדריכים למפתחים ([אינדקס מפתחים](../developer-index.md), [הפניה CLI](../developer-cli.md)) destacam flags sensiveis a securanca e escalation workflows.

## תוספות רשימת שחרורים

מנהלי שחרורים **מפתחים** מציינים ראיות שונות או מקדם מועמד ל-GA:1. Hash לעשות סקירת אבטחה תזכיר חדש (este documento).  
2. Link para o remediation ticket rastreado (לדוגמה: `governance/tickets/SF6-SR-2026.md`).  
3. פלט של `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` מפורש לקהל/מנפיק.  
4. Logs capturados עושים רתמה זוגית (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmacao de que כמו הערות שחרור לעשות Torii כולל הוכחה מוגבלת מונים טלמטריה.

Nao coletar os artefactos acima bloqueia o sign-off de GA.

**הפניות ל-hashs של חפצי אמנות (יציאה 2026-02-20):**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## מעקבים יוצאי דופן

- **רענון דגם האיום:** Repita esta revisao trimestralmente או antes de grandes adicoes de CLI דגלי.  
- **כיסוי מטושטש:** הוכחת קידודי הובלה של זרימת זרימה sao fuzzed באמצעות `fuzz/proof_stream_transport`, זהות מטענים של cobrindo, gzip, deflate e zstd.  
- **חזרה על תקרית:** סדר היום על תרגילי פעולות סימולנדו פשרה והחזרת מניפסט לאחור, הבטחה לביצוע פעולות דוקומנטריות.

## אישור

- נציג גילדת הנדסת אבטחה: @sec-eng (2026-02-20)  
- נציג קבוצת העבודה Tooling: @tooling-wg (2026-02-20)

Guarde מאשר את assinados junto ao לשחרר חבילת חפצים.