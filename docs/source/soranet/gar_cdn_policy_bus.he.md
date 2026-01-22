---
lang: he
direction: rtl
source: docs/source/soranet/gar_cdn_policy_bus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b230f98f025e9d55d6eefa2b7b44348ceef3f20505e723fc703151497aeef777
source_last_modified: "2026-01-03T18:08:02.008828+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/gar_cdn_policy_bus.md -->

# אפיק מדיניות GAR CDN

משטח האכיפה של SNNet-15G מאפשר כעת למפעילים לפרסם מטעני מדיניות GAR CDN (עקיפות TTL, תגיות purge, מזהי moderation, תקרות קצב, כללי geofence והחזקות משפטיות) דרך bus מגובה-קבצים כך ש-PoPs יקבלו ארטיפקטים ניתנים לשחזור לצד חבילות הקבלות שלהם.

## פרסום
- הריצו `cargo xtask soranet-gar-bus --policy <path> [--pop <label>] [--out-dir <dir>]` כדי לקרוא מטען JSON של `GarCdnPolicyV1` ולהפיק `gar_cdn_policy_event.{json,md}` תחת תיקיית היעד (ברירת מחדל `artifacts/soranet/gateway/<pop>/gar_bus/`).
- חבילת ה-JSON מתעדת את נתיב המקור, חותמת זמן הפרסום, תווית PoP אופציונלית ואת מדיניות ה-CDN המלאה, בהתאמה לחבילות הראיות של PoP.

## צריכת השערים
- שערים טוענים את `sorafs.gateway.cdn_policy_path` ומיישמים את אותו חוזה אכיפה (עקיפת TTL, תגיות purge, מזהי moderation, תקרות קצב, רשימות geofence/deny והחזקה משפטית) שמופיע בהפרות GAR של `GatewayPolicy` ובווריאציות פעולת הקבלה של ה-CLI (`ttl_override`, `moderation`).
- עדכונים לאירועי הפרת GAR נושאים את תוויות המדיניות החדשות, ה-TTL שנצפה, האזור ורמזי תקרת הקצב לצורכי דשבורדים/התראות.

</div>
