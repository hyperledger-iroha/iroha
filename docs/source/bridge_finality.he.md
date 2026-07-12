---
lang: he
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
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

לפני פרסום finality או פינוי גוף הבלוק, Kura כותב את ה־canonical header המדויק ואת
ארכיון SCCP המאומת בשורש אל immutable retained-block record, ולאחר מכן שומר את exact
V2 artifact ברשומת finality בלתי־משתנה נפרדת. שתי הכתיבות idempotent ודוחות כל
קונפליקט באותו גובה. `build_finality_proof` קורא רק retained header ורשומת finality
מאומתת; הוא לעולם אינו קורא historical block body או מחליף PoP ב־world state משתנה.
בעת restart מאומת מחדש הקשר header/archive/artifact/hash. פינוי הגוף אינו מעלים proof
תקין; רשומה חסרה, פגומה, סותרת או בלתי ניתנת לאימות גורמת לכשל סגור.

המאמת חסר-המצב מתאים במדויק גרסה, שרשרת, גובה, hash של header,
ה-predecessor הקנוני ו-view, הקשר, subject ו-CommitQC,
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
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` מחזיר `BridgeFinalityProof`;
- `GET /v1/bridge/finality/bundle/{height}` מחזיר `BridgeFinalityBundle`.

שני הנתיבים נכשלים באופן סגור אם ה־canonical header השמור או ה־v2 artifact המדויק
חסרים או אינם תקפים. פינוי גוף בלוק היסטורי אינו הופך proof תקין לבלתי זמין. יש לדחות
שדות, גרסאות וצורות הוכחה שיצאו משימוש ואינם מוכרים.

</div>
