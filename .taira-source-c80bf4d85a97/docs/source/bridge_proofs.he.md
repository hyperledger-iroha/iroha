---
lang: he
direction: rtl
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

<div dir="rtl">

> דף זה הוא תקציר מתורגם ומקוצר, ולא תרגום מלא. עבור כללי הממשל, ממשקי ה־API,
> משמעות ההוכחות ודרישות ההפצה, [הדף הקנוני באנגלית](bridge_proofs.md) הוא
> המקור הנורמטיבי המדויק.

# הוכחות גשר SCCP V1 — תקציר מקוצר

## תחום הגרסה הראשונה

SCCP V1 הוא פרוטוקול סגור לגרסה הראשונה. המקורות החיצוניים הנתמכים היחידים הם
`ethereum-mainnet`,‏ `bsc-mainnet` ו־`tron-mainnet`, ויעד SORA היחיד הוא
`sora-taira`. ‏Solana,‏ TON, רשתות מותאמות אישית וכל יעד SORA אחר אינם נתמכים
ונדחים באופן בטוח.

בגרסה זו `SubmitBridgeProof` מקבל רק הוכחות מטיפוס `NativeProtocol` ו־
`SccpDestination`. שליחת `Ics` כללי או `TransparentZk` אינה זמינה ונדחית עד
שיהיה עבורם מאמת מוסמך על השרשרת.

## מרשם מטופס והגנה מפני replay

`SccpRegistryV1` הוא מרשם מטופס, קשור ל־lane ומאפשר הוספה בלבד (append-only).
כל lane שומר לכל היותר 64 גרסאות route ו־4,096 native trust anchors. רשומות
היסטוריות אינן מפונות במשתמע; עם ההגעה למגבלה, ההוספה הבאה נדחית אטומית ללא
שינוי מצב.

מרווחי anchor נמדדים בקואורדינטת התקדמות consensus מאומתת: Ethereum משתמשת
ב־finalized beacon slot, ואילו BSC ו־TRON משתמשות בגובה native block סופי.
anchor ישן נשאר תקף עד checkpoint היורש, כולל נקודת הגבול; ה־anchor הנוכחי
האחרון פתוח בקצהו. ה־finality cutoff של route סופי חייב להיות שווה בדיוק
ל־checkpoint היורש של ה־anchor ההיסטורי.

רשומת inbound עמידה שומרת בנפרד את גובה הסופיות של האירוע/המקור ואת
`anchor_interval_height` המאומת. אינדקס high-water עמיד, שמפתחו lane ו־hash של
anchor, מונע מהממשל לבחור checkpoint יורש הנמוך מקואורדינטה שכבר התקבלה.
טעינת snapshot מחשבת את האינדקס מחדש מתוך הרשומות העמידות ודורשת שוויון מדויק;
אינדקס חסר, מיושן, פגום או חסר גיבוי נדחה. גם מזהי הודעות שכבר נוצלו נשמרים
כדי למנוע replay.

נתיב המקור של TRON משתמש בדיוק ב־ABI
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. ביצוע מצליח מחייב
`expectedNonce == transferNonce`; לאחר מכן אותו ערך נכתב אל ה־canonical payload
לפני הגדלת ה־storage. קבלת native משחזרת את קריאת ה־ABI המלאה מתוך ה־recipient
שב־payload, הסכום לאחר scaling וה־nonce. לכן ה־selector הישן בעל שני הארגומנטים,
nonce ישן או עתידי ו־nonce מסוג `uint64` שמוצה נדחים כולם באופן fail-closed.

## אימות במעבר יחיד ומגבלות עבודה

הוכחות destination ו־native נבנות פעם אחת, נקשרות פעם אחת ושומרות מראש תקציב
עבודה דטרמיניסטי לפני הפעלת קריפטוגרפיה יקרה. מסלול destination מאמת פעם אחת
את ה־BN254 pairing-product ופעם אחת את סופיות ה־BLS המקומית. מסלולי native
דורשים את ה־canonical shortest-prefix: לכל היותר 1,004 headers ב־BSC ו־54
ב־TRON.

`[zk.sccp]` אוכף מגבלות חיוביות לכל transaction ולכל block על מספר ההוכחות
וגודלן, native headers/bytes, עדכוני light client של Ethereum, שחזורי
secp256k1, בדיקות BLS aggregate ותרומות מפתחות, ובדיקות BN254 pairing. מגבלות
קבלה אלה קשורות ל־consensus: כל המאמתים חייבים להשתמש באותם ערכים מקובץ
התצורה, ואין להן overrides באמצעות משתני סביבה.

מגבלות ברירת המחדל של הגרסה הראשונה הן:

| ממד עבודה | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

proof יחיד יכול להכיל לכל היותר 8 MiB של canonical bytes. עבודה שנשמרה עבור
transaction שננטשה או נדחתה אינה זולגת אל ה־block.

## התחייבות outbound, שמירה וגילוי

כל הודעת outbound שהצליחה מקבלת `commitment_index` צפוף לפי סדר הביצוע בבלוק
(`0..=511`). ב־V1 המגבלות הקבועות הן 512 הודעות לבלוק ו־4,096 בתים של payload
קנוני להודעה. `[zk.sccp]` מגביל יחד את מצב ה־payload הממתין באמצעות
`max_pending_outbound_messages` (ברירת מחדל `65536`) ו־
`max_pending_outbound_payload_bytes` (ברירת מחדל `268435456`).

לפני פרסום finality או פינוי גוף הבלוק, Kura שומר באופן immutable את ה־canonical
header המדויק ואת ארכיון SCCP המאומת בשורש. שחזור proof, bundle, proof request
והיסטוריה אחרונה אינו קורא גוף בלוק היסטורי או עותק payload משתנה מן ה־WSV.
כאשר destination proof מתקבל, ה־pending payload והחיוב שלו נמחקים atomically
ומוחלפים ב־fixed terminal descriptor תוך שמירת locator/index. המצב הממתין חסום;
רשומות terminal והיסטוריית Kura הבלתי־משתנה גדלות בכוונה להגנת replay קבועה.
`GET /v1/sccp/messages/recent` משתמש בסמן המורכב `{ from, after_index }`.
הראיות הבלתי־משתנות נספרות בשימוש הדיסק הכולל/של המפעיל, אך לא בתקציב גופי הבלוקים
הניתנים לפינוי.

## מגבלות Torii ו־HTTP

Torii אוכף מגבלת גוף JSON נפרדת לכל SCCP endpoint לפני קריאת הגוף, הקצאת זיכרון
או אימות קריפטוגרפי. `Content-Length` או גוף chunked שחורגים מן המגבלה נדחים
עם HTTP `413`. הלקוח גם קורא את תגובת ה־HTTP המפוענחת תחת מגבלה קבועה, ולכן
`Content-Length` חסר או כוזב אינו יכול לעקוף אותה.

כל קלטי JSON,‏ base64 ו־Norito חייבים להיות canonical. שדות לא מוכרים, מפתחות
כפולים, network/route/anchor שגויים, replay, חריגה ממכסת עבודה או כשל אימות
נדחים ללא שינוי חלקי של המצב.

</div>
