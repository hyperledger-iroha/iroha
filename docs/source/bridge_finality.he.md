---
lang: he
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7236dfe86175ff89f660be4cb4dd2c90df20a05f9606d2707407465f639b1a1c
source_last_modified: "2025-12-05T06:21:36.529838+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/bridge_finality.md -->

<!--
SPDX-License-Identifier: Apache-2.0
-->

# הוכחות סופיות Bridge

מסמך זה מתאר את משטח הוכחת הסופיות הראשוני של bridge עבור Iroha.
המטרה היא לאפשר לשרשראות חיצוניות או light clients לאמת שבלוק Iroha סופי
ללא חישוב off-chain או ממסרים מהימנים.

## פורמט הוכחה

`BridgeFinalityProof` (Norito/JSON) מכיל:

- `height`: גובה הבלוק.
- `chain_id`: מזהה השרשרת של Iroha למניעת replay בין שרשראות.
- `block_header`: `BlockHeader` קנוני.
- `block_hash`: hash של ה-header (הלקוחות מחשבים מחדש כדי לאמת).
- `commit_certificate`: סט מאמתים + חתימות שסיימו את הבלוק.

ההוכחה עצמאית; אין צורך ב-manifests חיצוניים או בבלובים אטומים.
Retention: Torii מגיש הוכחות סופיות עבור חלון ה-commit-certificate האחרון
(מוגבל לפי history cap מוגדר; ברירת מחדל 512 רשומות דרך
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). לקוחות
צריכים לקמט או לעגן הוכחות אם הם צריכים אופקים ארוכים יותר.
הטופל הקנוני הוא `(block_header, block_hash, commit_certificate)`: hash ה-header חייב
להתאים ל-hash בתוך ה-commit certificate, וה-chain id קושר את ההוכחה ללדג'ר יחיד.
השרתים דוחים ומתעדים `CommitCertificateHashMismatch` כאשר ה-certificate מצביע
ל-hash בלוק שונה.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) מרחיב את ההוכחה הבסיסית עם commitment ו-justification
מפורשים:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: חתימות של ה-authority set על payload ה-commitment
  (שימוש חוזר בחתימות ה-commit certificate).
- `block_header`, `commit_certificate`: כמו בהוכחה הבסיסית.

Placeholder נוכחי: `mmr_root`/`mmr_peaks` נגזרים מחישוב מחדש של MMR על hash הבלוקים בזיכרון;
אין החזרת inclusion proofs עדיין. הלקוחות עדיין יכולים לאמת את אותו hash באמצעות
payload ה-commitment כיום.

MMR peaks are ordered left to right. Recompute `mmr_root` by bagging peaks
from right to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

האימות דומה להוכחה הבסיסית: לחשב מחדש `block_hash` מה-header, לאמת חתימות של
commit certificate, ולוודא ששדות ה-commitment תואמים ל-certificate ול-hash של הבלוק.
ה-bundle מוסיף מעטפת commitment/justification עבור פרוטוקולים של bridge שמעדיפים הפרדה.

## שלבי אימות

1. חשב מחדש `block_hash` מ-`block_header`; דחה על אי התאמה.
2. בדוק ש-`commit_certificate.block_hash` תואם ל-`block_hash` המחושב מחדש;
   דחה זוגות header/commit certificate לא תואמים.
3. בדוק ש-`chain_id` תואם לשרשרת Iroha הצפויה.
4. חשב מחדש `validator_set_hash` מ-`commit_certificate.validator_set` ובדוק שהוא תואם
   ל-hash/גרסה הרשומים.
5. אמת חתימות ב-commit certificate מול hash ה-header באמצעות מפתחות ציבוריים ואינדקסים
   של המאמתים המצוינים; אכוף quorum (`2f+1` כאשר `n>3`, אחרת `n`) ודחה אינדקסים כפולים/מחוץ לטווח.
6. אופציונלית: קושר ל-checkpoint מהימן על ידי השוואת hash ה-validator set לערך מעוגן
   (weak-subjectivity anchor).
7. אופציונלית: קושר ל-epoch צפוי כך שהוכחות מאפוקים ישנים/חדשים יידחו עד שה-anchor יוחלף במכוון.

`BridgeFinalityVerifier` (ב-`iroha_data_model::bridge`) מיישם את הבדיקות הללו, ודוחה
chain-id/height drift, אי התאמה של hash/גרסה ב-validator set, חתומים כפולים/מחוץ לטווח,
חתימות לא תקינות, ו-epochs לא צפויים לפני ספירת quorum, כך ש-light clients יכולים
להשתמש במאמת יחיד.

## מאמת רפרנס

`BridgeFinalityVerifier` מקבל `chain_id` צפוי ועוד anchors אופציונליים של validator set ו-epoch.
הוא אוכף את טופל header/block-hash/commit-certificate, מאמת hash/גרסה של validator set,
בודק חתימות/quorum מול roster המאמתים המפורסם, ועוקב אחרי הגובה האחרון כדי לדחות הוכחות
stale/מדולגות. כאשר מסופקים anchors הוא דוחה replays בין epochs/rosters עם שגיאות
`UnexpectedEpoch`/`UnexpectedValidatorSet`; ללא anchors הוא מאמץ את hash ה-validator set
וה-epoch מההוכחה הראשונה לפני שהוא ממשיך לאכוף שגיאות דטרמיניסטיות עבור חתימות
כפולות/מחוץ לטווח/לא מספיקות.

## משטח API

- `GET /v1/bridge/finality/{height}` - מחזיר `BridgeFinalityProof` עבור גובה הבלוק המבוקש.
  Content negotiation דרך `Accept` תומך Norito או JSON.
- `GET /v1/bridge/finality/bundle/{height}` - מחזיר `BridgeFinalityBundle`
  (commitment + justification + header/certificate) עבור הגובה המבוקש.

## הערות ומעקבים

- הוכחות מופקות כרגע מ-commit certificates שמורים. ההיסטוריה המוגבלת עוקבת אחרי חלון השימור
  של commit certificates; לקוחות צריכים לקמט הוכחות עוגן אם הם צריכים אופקים ארוכים יותר.
  בקשות מחוץ לחלון מחזירות `CommitCertificateNotFound(height)`; הציגו את השגיאה וחזרו ל-checkpoint
  מעוגן.
- הוכחה משוחזרת או מזויפת עם `block_hash` לא תואם (header לעומת certificate) נדחית עם
  `CommitCertificateHashMismatch`; לקוחות צריכים לבצע את אותה בדיקת טופל לפני אימות חתימות
  ולדחות payloads לא תואמים.
- עבודה עתידית יכולה להוסיף שרשראות commitment של MMR/authority-set כדי לצמצם את גודל ההוכחות
  להיסטוריות ארוכות מאוד. הפורמט נשאר תואם לאחור על ידי עטיפת ה-commit certificate בתוך
  מעטפות commitment עשירות יותר.

</div>
