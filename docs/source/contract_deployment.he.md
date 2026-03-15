<!-- Hebrew translation of docs/source/contract_deployment.md -->

---
lang: he
direction: rtl
source: docs/source/contract_deployment.md
status: complete
translator: manual
---

<div dir="rtl">

# פריסת חוזים (.to) — API ותהליך עבודה

סטטוס: מיושם ונבדק ב-Torii, ב-CLI ובבדיקות קבלה (נובמבר 2025).

## סקירה

- ניתן לפרוס בייטקוד IVM (.to) דרך Torii או בעזרת ההוראות `RegisterSmartContractCode` ו-`RegisterSmartContractBytes` ישירות.
- הנוד מחשב מחדש את `code_hash` ואת `abi_hash` הקנוני; אי התאמה → דחייה דטרמיניסטית.
- מניפסטים וקטעי קוד נשמרים ב-`contract_manifests` וב-`contract_code`. המניפסט מצביע על hash בלבד והקוד נשמר תחת `code_hash`.
- ניימספייס מוגנים יכולים לדרוש הצעת ממשל מאושרת לפני קבלה. קו הקבלה מצליב `(namespace, contract_id, code_hash, abi_hash)` מול ההצעה.

## ארטיפקטים ואחסון

- `RegisterSmartContractCode` מוסיף/מחליף את המניפסט עבור `code_hash`.
- `RegisterSmartContractBytes` שומר את הקוד תחת `contract_code[code_hash]`; פער בין גרסאות יוצר שגיאת invariant.
- גודל מקסימלי מוגדר בפרמטר `max_contract_code_bytes` (ברירת מחדל 16MiB). לשינוי יש לשלוח `SetParameter(Custom)`.
- אין מחיקה אוטומטית; הכל נשמר עד למהלך ממשל עתידי.

## קבלת טרנזקציות

- נבדק שה-header של IVM עומד ב-`version_major == 1` ו-`abi_version == 1`.
- אם מניפסט כבר קיים, חייב להיות התאמה בין hashים מאוחסנים למחשוב מחדש.
- ניימספייס מוגן מחייב מטא-דאטה `gov_namespace` ו-`gov_contract_id`; בהיעדר הצעת Deploy תואמת → `NotPermitted`.

## Torii (feature `app_api`)

- **POST `/v2/contracts/deploy`** — גוף: `DeployContractDto` (`authority`, `private_key`, `code_b64`). Torii מחשב hash-ים, בונה מניפסט ושולח `RegisterSmartContractCode` + `RegisterSmartContractBytes` בטרנזקציה אחת. תשובה: `{ ok, code_hash_hex, abi_hash_hex }`.
- **POST `/v2/contracts/code`** — גוף: `RegisterContractCodeDto` (`authority`, `private_key`, `manifest`). שולח רק את המניפסט.
- **POST `/v2/contracts/instance`** — גוף: `DeployAndActivateInstanceDto` (`authority`, `private_key`, `namespace`, `contract_id`, `code_b64`, מניפסט אופציונלי). מבצע פריסה והפעלה אטומית. תשובה: `{ ok, namespace, contract_id, code_hash_hex, abi_hash_hex }`.
- **POST `/v2/contracts/instance/activate`** — גוף: `ActivateInstanceDto` (`authority`, `private_key`, `namespace`, `contract_id`, `code_hash`). מפעיל חוזה קיים. תשובה: `{ ok: true }`.
- **GET `/v2/contracts/code/{code_hash}`** — מחזיר `{ manifest: { code_hash, abi_hash } }`.
- **GET `/v2/contracts/code-bytes/{code_hash}`** — מחזיר `{ code_b64 }` עם ה-bytecode.

כל נקודות הקצה של מחזור החיים חולקות rate limiter משותף (`torii.deploy_rate_per_origin_per_sec`, `torii.deploy_burst_per_origin`). ברירת המחדל: 4 בקשות לשנייה ו-burst של 8 לכל מקור (`X-API-Token`, כתובת IP, ושם נקודת הקצה). חריגה → HTTP 429 והגדלת `torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}`; שגיאת מעבד → `torii_contract_errors_total{endpoint=…}`.

## אינטגרציית ממשל וניימספייס מוגנים

- `gov_protected_namespaces` (פרמטר מותאם) מפעיל שער קבלה. Torii מספק `/v2/gov/protected-namespaces`; CLI: `iroha_cli app gov protected set/get`.
- הצעות `ProposeDeployContract` מאחסנות `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- לאחר `EnactReferendum` מתקבלות פריסות עם נתונים תואמים ומטא-דאטה `gov_namespace`/`gov_contract_id`.

## CLI

- `iroha_cli app contracts deploy` – שולח בקשת פריסה.
- `iroha_cli app contracts manifest get` – שולף מניפסט.
- `iroha_cli app contracts code get` – מוריד `.to`.
- `iroha_cli app contracts instances` – מציג מופעים פעילים.
- פקודות ממשל (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`, `iroha_cli app gov protected set/get`) משלימות את ה-flow.

## בדיקות

- `contract_code_bytes.rs` – כיסוי אחסון.
- `gov_enact_deploy.rs`, `gov_protected_gate.rs` – תרחישי ממשל.
- Torii ו-CLI כוללים בדיקות יחידה/אינטגרציה.

פרטים נוספים: `docs/source/governance_api.md`.

</div>
