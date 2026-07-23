---
lang: he
direction: rtl
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus_fee_model.md -->

# עדכוני מודל העמלות של Nexus

נתב הסליקה המאוחד מתעד כעת קבלות דטרמיניסטיות לפי lane, כך שמפעילים יכולים
ליישב חיובי gas מול מודל העמלות של Nexus.

- לארכיטקטורת הנתב המלאה, מדיניות הבאפרים, מטריצת הטלמטריה ורצף ה-rollout ראו
  `docs/settlement-router.md`. המדריך מסביר כיצד הפרמטרים המתועדים כאן קשורים למסירת NX-3
  וכיצד על SREs לעקוב אחרי הנתב בפרודקשן.
- הגדרת asset הגז (`pipeline.gas.units_per_gas`) כוללת ערך עשרוני `twap_local_per_xor`,
  `liquidity_profile` (`tier1`, `tier2`, או `tier3`), ו-`volatility_class` (`stable`,
  `elevated`, `dislocated`). דגלים אלה מוזנים ל-settlement router כדי שהצעת מחיר XOR
  תתאים ל-TWAP הקנוני ול-tier ה-haircut של ה-lane.
- כל עסקה חייבת לשאת את השדה המובנה והקשור לחתימה `fee_payment`
  (`FeePaymentIntent`). השדה בוחר את הרשות או תוכנית ספונסר מדויקת ואת
  הגרסה הבלתי משתנה שלה, וכולל תקרות חיוב חתומות וגבול gas חיובי כשנדרש.
  מפתחות המטא-נתונים הישנים `fee_sponsor`, `gas_limit` ו-`gas_asset_id`
  נדחים.
- יש לקבל הצעת מחיר לפני החתימה: בונים את ה-payload הלא חתום המדויק, נותנים
  לרשות שלו לאמת את `POST /v1/fees/quote`, בודקים את ה-intent המומלץ,
  מחליפים רק את `payload.fee_payment`, ואז חותמים ושולחים בדיוק את אותו
  payload. הצעת המחיר היא תצפית ולא הזמנה; admission בודק שוב את המצב העדכני.
- settlement ישיר תומך ברשות או בתוכנית ספונסר מדויקת. settlement מבוסס
  receipts (`lane_relay_burn`) מיועד רק לספונסר מדויק: עמלות Nexus במימון
  הרשות נדחות עם `relay_capacity_unavailable`, משום שיתרת הרשות אינה
  source lock מאומת של receipt.
- כל טרנזקציה שמשלמת gas רושמת `LaneSettlementReceipt`. כל קבלה שומרת מזהה מקור שסופק על ידי
  הקורא, micro-amount מקומי, XOR לתשלום מיידי, XOR צפוי לאחר haircut, מרווח בטיחות ממומש
  (`xor_variance`), וחותמת זמן בלוק במילישניות.
- ביצוע הבלוק מאגד קבלות לפי lane/dataspace ומפרסם אותן דרך `lane_settlement_commitments`
  ב-`/v1/sumeragi/status`. הסיכומים חושפים `total_local_amount`, `total_xor_due`,
  ו-`total_xor_after_haircut` מסוכמים עבור הבלוק לצורך ייצוא התאמות ליליות.
- מונה חדש `total_xor_variance` עוקב אחרי כמה מרווח בטיחות נצרך (הבדל בין ה-XOR לתשלום
  לבין הציפיה אחרי haircut), ו-`swap_metadata` מתעד את פרמטרי ההמרה הדטרמיניסטיים
  (TWAP, epsilon, liquidity profile, ו-volatility_class) כדי שמבקרים יוכלו לאמת את
  קלטי הצעת המחיר ללא תלות בהגדרות runtime.

צרכנים יכולים לעקוב אחרי `lane_settlement_commitments` לצד snapshots קיימים של commitments עבור
lane ו-dataspace כדי לוודא שבאפרי העמלות, tiers של haircut וביצוע swap תואמים את מודל העמלות של
Nexus שהוגדר.

</div>
