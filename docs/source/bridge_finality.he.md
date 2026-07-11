---
lang: he
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93505cbda553c6d73c4850776545a87723b03a0d922610e6e7786a3f379b8fae
source_last_modified: "2026-07-11T23:16:35+00:00"
translation_last_reviewed: 2026-07-11
---

<div dir="rtl">

<!--
SPDX-License-Identifier: Apache-2.0
-->

# הוכחות סופיות של הגשר

מסמך זה מגדיר את פורמט המהדורה הראשונה לסופיות הגשר. ההוכחה נושאת את ראיית
הסופיות המדויקת והמתמידה שמפיק Sumeragi v2. גרסת הסכמה של מעטפת ההוכחה היא `1`,
ואילו גרסת פרוטוקול הקונצנזוס שבתוכה היא `2`. אין projection של תעודת Sumeragi v1,
אין decoder ואין מסלול fallback.

## פורמט ההוכחה המדויק

`BridgeFinalityProof` בקידוד Norito או Norito JSON מכיל בדיוק שלושה שדות:

```text
{ version, block_header, finality_artifact }
```

- `version` חייב להיות `1`;
- `block_header` הוא ה-`BlockHeader` הקנוני של הגובה המבוקש;
- `finality_artifact` הוא ה-`V2FinalityArtifact` המדויק שנשמר עבור הבלוק. הוא מטמיע
  באופן מתמיד PoP מסוג BLS-normal לכל validator, לפי סדר ה-roster שב-height context
  (`validator_set_pops`).

ה-artifact כולל `HeightContext` מלא ובלתי משתנה, `BlockSubject` מדויק, hash של הבלוק,
CommitQC ו-PoP התואמים ל-roster. ה-height context מקפיא את השרשרת, ה-epoch, ה-roster,
ה-`DualQuorum`, פריסת DA, ה-leader seed ושאר נתוני הקונצנזוס. הקשר של בלוק האב
שמסיים epoch כולל גם `next_epoch_snapshot` אופציונלי; מאחר שהשדה הוא חלק מה-context
id, ה-CommitQC של האב מאמת אותו לפני שיוכל לאשר את roster הבן.
ה-snapshot הסופי מאמת גם את `epoch_end_height`, את ה-`validator_set_pops` המיושרים
ל-roster הבא ואת פרמטרי ה-epoch הבא.

## התמדה ואימות

מסלול ה-apply של Sumeragi v2 מאמת את ה-artifact ושומר אותו כ-Kura sidecar בלתי משתנה.
בונה ההוכחה קורא את הבלוק הקנוני ואת ה-sidecar שלו, ואינו בונה מחדש PoP או תעודות
היסטוריות מתוך ה-world state הנוכחי והמשתנה. sidecar חסר, פגום, סותר או בלתי ניתן
לאימות גורם לכשל סגור; הזמינות אינה מוגבלת לחלון היסטוריה עדכני בזיכרון.

המאמת חסר-המצב מתאים במדויק גרסה, שרשרת, גובה, hash של header, הקשר, subject ו-CommitQC,
ומאמת את כל ה-PoP המוטמעים ב-artifact. אינדקסי החותמים חייבים להיות עולים בהחלט ובטווח;
ה-CommitQC חייב לעמוד בשני ספי quorum — מספר validators וכוח הצבעה — וחתימת ה-BLS
המצטברת על ה-Sumeragi v2 vote preimage המדויק חייבת להיות תקפה.

## עוגן אמון ואימות יורשים

הוכחה יחידה מוכיחה רק עקביות פנימית תחת ה-roster שהיא נושאת. לכן
`BridgeFinalityVerifier` דורש `HeightContextId` מהימן במפורש לפני קבלת ההוכחה הראשונה.
לאחר מכן הוא מקבל רק את הגובה הבא המיידי ומאמת את ה-parent CommitQC שבהקשר הבן באמצעות
ה-roster וה-PoP הקודמים שהוקפאו. בתוך epoch ה-artifact הבן מעתיק את ה-PoP של ה-artifact
הקודם; בגבול epoch, ה-epoch, ה-roster, ה-quorum, ה-seed וה-PoP חייבים להתאים ל-
`next_epoch_snapshot` שבהקשר האב, כולל `epoch_end_height` המאומת, והכול מאומת ב-CommitQC
של האב. גבהים ישנים או מדולגים ויורשים
שאינם מקושרים נדחים.

SCCP משתמש באותו `BridgeFinalityProof`. אין להסתפק בחתימה תחת roster שסיפקה ההודעה;
יש לאמת כל יורש מיידי מה-context/artifact של checkpoint שעוגן בממשל ועד artifact ההודעה.

## Bundle ו-API

`BridgeFinalityBundle` מכיל בדיוק `{ commitment, finality_proof }`. ה-commitment הוא
`{ chain_id, height_context_id, block_height, block_hash, mmr_root?,
mmr_leaf_index?, mmr_peaks? }`. שדות MMR אופציונליים הם commitments בלבד; הם אינם
סופיות ואינם הוכחת הכללה.

- `GET /v1/bridge/finality/{height}` מחזיר `BridgeFinalityProof`;
- `GET /v1/bridge/finality/bundle/{height}` מחזיר `BridgeFinalityBundle`.

שני הנתיבים נכשלים באופן סגור אם הבלוק או ה-artifact המדויק והמתמיד של v2 חסרים או
אינם תקפים. יש לדחות שדות, גרסאות וצורות הוכחה שיצאו משימוש ואינם מוכרים.

</div>
