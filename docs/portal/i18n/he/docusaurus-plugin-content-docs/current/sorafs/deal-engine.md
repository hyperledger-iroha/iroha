---
id: deal-engine
lang: he
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/deal_engine.md`. שמרו על שני המיקומים מיושרים כל עוד התיעוד הישן פעיל.
:::

# מנוע העסקאות של SoraFS

מסלול הרודמאפ SF-8 מציג את מנוע העסקאות של SoraFS, ומספק
חשבונאות דטרמיניסטית להסכמי אחסון ושליפה בין
לקוחות לספקים. ההסכמים מתוארים באמצעות payloads של Norito
המוגדרים ב-`crates/sorafs_manifest/src/deal.rs`, המכסים תנאי עסקה,
נעילת bonds, מיקרו-תשלומים הסתברותיים ורשומות הסדר.

ה-worker המובנה של SoraFS (`sorafs_node::NodeHandle`) יוצר כעת
מופע `DealEngine` לכל תהליך נוד. המנוע:

- מאמת ורושם עסקאות באמצעות `DealTermsV1`;
- צובר חיובים נקובים ב-XOR כאשר מדווח שימוש ברפליקציה;
- מעריך חלונות מיקרו-תשלום הסתברותיים באמצעות דגימה דטרמיניסטית
  המבוססת על BLAKE3; ו
- מפיק snapshots של ledger ו-payloads של הסדר המתאימים לפרסום ממשל.

בדיקות יחידה מכסות ולידציה, בחירת מיקרו-תשלומים וזרימות הסדר כדי
שהמפעילים יוכלו לתרגל את ה-API בביטחון. ההסדרים כעת פולטים payloads של ממשל
`DealSettlementV1`, משתלבים ישירות בצינור הפרסום SF-12, ומעדכנים את סדרת OpenTelemetry
`sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) עבור דשבורדים של Torii והחלת SLOs.
פריטי ההמשך מתמקדים באוטומציית slashing ביוזמת מבקרים ובהתאמת סמנטיקת הביטול למדיניות הממשל.

טלמטריית השימוש מזינה גם את קבוצת המדדים `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, ומוני הכרטיסים
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). הסכומים האלה חושפים את זרימת ההגרלה
ההסתברותית, כדי שמפעילים יוכלו לקשר בין זכיות מיקרו-תשלום והעברת קרדיט
לבין תוצאות ההסדר.

## אינטגרציית Torii

Torii חושפת endpoints ייעודיים כדי שספקים יוכלו לדווח על שימוש ולהניע את
מחזור החיים של העסקה ללא wiring מותאם:

- `POST /v1/sorafs/deal/usage` מקבל טלמטריית `DealUsageReport` ומחזיר
  תוצאות חשבונאיות דטרמיניסטיות (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` מסיים את החלון הנוכחי, ומזרים את
  `DealSettlementRecord` שנוצר לצד `DealSettlementV1` בקידוד base64
  מוכן לפרסום ב-DAG של ממשל.
- פיד `/v1/events/sse` של Torii משדר כעת רשומות `SorafsGatewayEvent::DealUsage`
  המסכמות כל שליחת שימוש (epoch, GiB-hours נמדדים, מוני כרטיסים,
  חיובים דטרמיניסטיים), רשומות `SorafsGatewayEvent::DealSettlement`
  הכוללות את snapshot הלדג'ר הקנוני של ההסדר יחד עם digest/size/base64 של BLAKE3
  עבור ארטיפקט הממשל על הדיסק, והתראות `SorafsGatewayEvent::ProofHealth` בכל פעם
  שספי PDP/PoTR נחצים (ספק, חלון, מצב strike/cooldown, סכום קנס). צרכנים יכולים
  לסנן לפי ספק כדי להגיב לטלמטריה חדשה, הסדרים או התראות בריאות proofs ללא polling.

שני ה-endpoints משתתפים במסגרת המכסות של SoraFS דרך החלון החדש
`torii.sorafs.quota.deal_telemetry`, ומאפשרים למפעילים לכוון את קצב ההגשה
המותר לכל פריסה.
