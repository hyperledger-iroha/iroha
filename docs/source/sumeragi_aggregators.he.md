<!-- Hebrew translation of docs/source/sumeragi_aggregators.md -->

---
lang: he
direction: rtl
source: docs/source/sumeragi_aggregators.md
status: complete
translator: manual
---

<div dir="rtl">

# ניתוב אגרגטורים ב-Sumeragi

## סקירה כללית

מסמך זה מתאר את אסטרטגיית הניתוב הדטרמיניסטית של האוספים (“Aggregators”) שבה משתמשת Sumeragi לאחר עדכון הפיירנס של שלב 3. כל מאמת מחשב את אותו סדר אוספים עבור זוג גובה-בלוק/תצוגה, מה שמבטל תלות באקראיות אד-הוק ומבטיח שאם האוספים הייעודיים אינם מגיבים, החזרות מסלימות למניפה (“gossip”) חסומה.

## בחירה דטרמיניסטית

- המודול החדש `sumeragi::collectors` מספק את `deterministic_collectors(topology, mode, k, seed, height, view)` ומחזיר `Vec<PeerId>` שחוזר על עצמו עבור זוג `(height, view)`.
- במצב Permissioned, קבוצת האוספים בסוף הרשימה מסתובבת לפי `height + view`, כך שכל אוסף הופך להיות ראשי בסבב עגול. הדבר שומר על התנהגות ה-proxy tail ההיסטורית ומחלק עומס שווה גם על צופים.
- במצב NPoS, ה-PRF לכל אפוק נותר כפי שהיה, אך פונקציית העזר מרכזת את החישוב כך שכל קורא יקבל את אותו סדר. ה-seed נגזר מהאקראיות של האפוק המסופקת ע״י `EpochManager`.
- `CollectorPlan` עוקב אחרי ניצול סדר היעדים ומסמן אם fallback לגוסיפ הופעל. מדדי טלמטריה (`collect_aggregator_ms`, ‏`sumeragi_redundant_sends_*`, ‏`sumeragi_gossip_fallback_total`) מראים באיזו תדירות ההסלמה מתרחשת וכמה זמן נמשך הפאן-אאוט.

## מטרות הפיירנס

1. **שחזוריות:** אותה טופולוגיית מאמתים, אותו מצב קונצנזוס ואותו זוג `(height, view)` חייבים להניב ראשיים/משניים זהים בכל peer. פונקציית העזר מסתירה מאפיינים טופולוגיים (proxy tail, צופים) כך שהסדר נייד בין רכיבים ובדיקות.
2. **סבביות:** בפריסות permissioned האוסף הראשי מתחלף בכל בלוק (וגם לאחר bump לתצוגה), כדי שאף צופה לא ישלוט תמיד באיסוף. בחירת NPoS המבוססת PRF כבר מספקת אקראיות ולכן איננה מושפעת.
3. **תצפיות:** הטלמטריה ממשיכה לדווח על שיוכי אוספים והמסלול החלופי מפיק אזהרה כשהגוסיפ נכנס לפעולה, כך שמפעילים יכולים לזהות אוספים בעייתיים.

## חזרות ו-backoff של גוסיפ

- מאמתים מחזיקים `CollectorPlan` לצד `VotingBlock` שבטיפול. התוכנית רושמת את מספר האוספים שנוצר קשר איתם ואם בוצעה הסלמה לגוסיפ.
- שליחה עודפת (`r`) מיושמת דטרמיניסטית בהתקדמות לאורך התוכנית. כאשר לא נשארים אוספים נוספים – או שכל הניסיונות מוצו ללא אישור – התוכנית מסמנת שהופעל fallback. המדד `collect_aggregator_gossip_total` ב-`/v1/sumeragi/phases` משקף את מדד Prometheus כדי שמפעילים יוכלו להתראה על מיצוי חוזר.
- fallback שולח את הבלוק וה-prevote החתומים לכל peer אחר בטופולוגיה (למעט העצמי). זה מבטיח חיות גם אם כל האוספים המתוכננים כשלו, ומחקה את פייל-סייפ הישן של “שידור לכולם” תוך שמירה על מיקוד בזמן עבודה רגיל.
- ה-fallback מתרחש פעם אחת בלבד לכל בלוק כדי למנוע הצפת רשת. כל השלכה של הצעה בשל שער commit certificate נעול מעלה את `block_created_dropped_by_lock_total`; מסלולי כשלי ולידציית כותרת מעלים את `block_created_hint_mismatch_total` ואת `block_created_proposal_mismatch_total`, ומאפשרים למפעילים לקשר fallback חוזר לבעיות במנהיג. תמונת `/v1/sumeragi/status` מציגה את ה-Highest/Locked commit certificate העדכניים כך שניתן לחבר פסגות drop להאש־בלוקים מסוימים.

## סיכום יישום

- מודול ציבורי חדש `sumeragi::collectors` מאחסן את `CollectorPlan` ו-`deterministic_collectors`, כך שבדיקות רמה-קרייט ובדיקות אינטגרציה יכולות לאמת פיירנס ללא הפעלת שחקן הקונצנזוס המלא.
- `VotingBlock` שומר עתה `CollectorPlan` ומספק פונקציות עזר לזיהוי מיצוי התוכנית ולהפעלת גוסיפ בדיוק פעם אחת.
- `Sumeragi` משתמשת ב-`collector_targets_for_round()` כדי לבנות את התוכנית וב-`gossip_vote_to_all_collectors()` כדי לממש את השידור החלופי.
- בדיקות יחידה ואינטגרציה מאמתות סבביות במצב permissioned, דטרמיניזם של PRF ומעברי מצבי backoff.

## חתימות ביקורת

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG

</div>
