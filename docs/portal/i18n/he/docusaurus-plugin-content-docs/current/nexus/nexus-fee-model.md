---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-fee-model
title: עדכוני מודל העמלות של Nexus
description: מראה של `docs/source/nexus_fee_model.md`, המתעד קבלות סליקה של lanes ומשטחי פיוס.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/nexus_fee_model.md`. שמרו על יישור שתי הגרסאות בזמן שהתרגומים ליפנית, עברית, ספרדית, פורטוגזית, צרפתית, רוסית, ערבית ואורדו עוברים.
:::

# עדכוני מודל העמלות של Nexus

נתב הסליקה המאוחד כעת קולט קבלות דטרמיניסטיות לכל lane כדי שמפעילים יוכלו להתאים חיובי גז מול מודל העמלות של Nexus.

- לארכיטקטורת הנתב המלאה, מדיניות הבופר, מטריצת הטלמטריה ורצף ההשקה ראו `docs/settlement-router.md`. המדריך מסביר כיצד הפרמטרים המתועדים כאן נקשרים לאבן הדרך NX-3 וכיצד צוותי SRE צריכים לנטר את הנתב בייצור.
- תצורת נכס הגז (`pipeline.gas.units_per_gas`) כוללת ערך עשרוני `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2`, או `tier3`) ו-`volatility_class` (`stable`, `elevated`, `dislocated`). דגלים אלו מזינים את נתב הסליקה כך שהציטוט של XOR יתאים ל-TWAP הקנוני ולשכבת ה-haircut של ה-lane.
- כל עסקה שמשלמת גז רושמת `LaneSettlementReceipt`. כל קבלה שומרת מזהה מקור שסופק על ידי הקורא, את המיקרו-סכום המקומי, את ה-XOR המיידי לתשלום, את ה-XOR הצפוי לאחר ה-haircut, את השונות שהושגה (`xor_variance_micro`), ואת חותמת הזמן של הבלוק במילישניות.
- ביצוע הבלוק מאגד קבלות לכל lane/dataspace ומפרסם אותן דרך `lane_settlement_commitments` ב-`/v2/sumeragi/status`. הסיכומים מציגים `total_local_micro`, `total_xor_due_micro`, ו-`total_xor_after_haircut_micro` מסוכמים על פני הבלוק ליצוא פיוס לילה.
- מונה חדש `total_xor_variance_micro` עוקב אחרי כמה מרווח בטיחות נצרך (ההפרש בין ה-XOR החייב לבין הציפייה לאחר haircut), ו-`swap_metadata` מתעד פרמטרי המרה דטרמיניסטיים (TWAP, epsilon, liquidity profile, ו-volatility_class) כדי שמבקרים יוכלו לאמת את קלטי הציטוט ללא תלות בהגדרות זמן ריצה.

צרכנים יכולים לעקוב אחרי `lane_settlement_commitments` לצד ה-snapshots הקיימים של commitments ל-lane ול-dataspace כדי לוודא שמאגרי העמלות, שכבות ה-haircut וביצוע ה-swap תואמים את מודל העמלות של Nexus שהוגדר.
