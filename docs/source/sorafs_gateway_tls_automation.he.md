---
lang: he
direction: rtl
source: docs/source/sorafs_gateway_tls_automation.md
status: complete
translation_last_reviewed: 2026-02-18
title: מדריך המפעיל ל-TLS ו-ECH של שער SoraFS
summary: הגדרות, טלמטריה, הפעלת תגובה לאירועים ודרישות ממשל לאוטומציית התעודות SF-5b.
---

# מדריך המפעיל ל-TLS ו-ECH של שער SoraFS

## היקף

מדריך זה מתעד כיצד להפעיל את שער SoraFS של Torii כאשר מערך אוטומציית התעודות SF-5b פעיל. הוא מחליף את מסמך התכנון הקודם ומתמקד במשימות היומיומיות: הכנת סודות, הגדרת ACME ו-ECH, קריאת הטלמטריה, עמידה בחובות הממשל והפעלת ספרי ההפעלה המאושרים לטיפול באירועים.

המסמך מניח שהשער כולל את בקר האוטומציה שב-`iroha_torii::sorafs::gateway` ושבשרת הבסטיון מותקנת הפקודה `cargo xtask sorafs-self-cert`.

## רשימת בדיקת הפעלה מהירה

1. יש לאחסן את אישורי ה-ACME ותוצרי ה-GAR בכספת `sorafs_gateway_tls/` (KMS או Vault) ולהעתיקם לשרת הבסטיון.
2. יש למלא את `torii.sorafs_gateway.acme` בהעדפות לצ'אלנג'ים DNS-01 ו-TLS-ALPN-01, בקביעת האם להפעיל ECH ובשמים סף לחידוש.
3. יש לפרוס או להפעיל את היחידה `sorafs-gateway-acme@<environment>.service` כדי שבקר האוטומציה יפעל באופן רציף.
4. יש להריץ `cargo xtask sorafs-self-cert --check-tls` כדי לקבל חבילת אישור בסיסית ולאמת כותרות וטלמטריה.
5. יש לפרסם לוחות מחוונים והתראות עבור המטריקות המפורטות ב**מדריך הטלמטריה** ולשייך אותן לצוות התורן.
6. יש להעלות את ספרי ההפעלה לכלי הניהול (PagerDuty, Notion) ולתזמן את התרגילים שפורטו ב**תדירות התרגילים ותיעוד ראיות**.

## מדריך הגדרה

### תנאים מקדימים

| דרישה | חשיבות |
|-------------|----------------|
| אישורי חשבון ACME המאוחסנים ב-`sorafs_gateway_tls/` (KMS/Vault) | נחוצים להזמנות ידניות, לביטול תעודות ולניהול אסימוני חידוש. |
| עותק לא מקוון של חבילת ההגדרות `torii.sorafs_gateway` | מאפשר לשנות `ech_enabled`, רשימות מארחים וזמני נסיונות מחדש ללא תלות בצינור ההגדרה המרכזי. |
| גישה למניפסטי ממשל (`manifest_signatures.json`, מעטפות GAR) | דרושה בעת פרסום טביעות אצבע חדשות או סבבי שמות קנוניים. |
| התקנת שרשרת הכלים `cargo xtask` על שרת הבסטיון | מאפשרת להריץ את `sorafs-gateway-attest` / `sorafs-self-cert` לצורך בדיקה. |
| שמירת תבנית הספרים לכלים כגון PagerDuty/Notion | מאפשרת להגיע לרשימות הבדיקה בחוברת זו בלחיצה אחת בזמן אירוע. |

### סודות וסידור קבצים

- יש לשמור את פרטי חשבון ה-ACME תחת `/etc/iroha/sorafs_gateway_tls/` בבעלות `iroha:iroha` והרשאות `0600`.
- יש להחזיק גיבוי מוצפן בכספת (KMS/Vault) לצורך התאוששות מהירה בעת רוטציה ידנית.
- בקר האוטומציה שומר את מצבו בקובץ `/var/lib/sorafs_gateway_tls/automation.json`; יש לכלול אותו בגיבויים כדי לשמר את חלונות הג'יטר והנסיונות מחדש.

### הגדרת Torii

יש להוסיף או לעדכן את ההגדרות הבאות בקובץ הקונפיגורציה (לדוגמה `configs/production.toml`):

```toml
[torii.sorafs_gateway.acme]
account_email = "tls-ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
dns_provider = "route53"
renewal_threshold_seconds = 2_592_000  # 30 days
retry_backoff_seconds = 900
retry_jitter_seconds = 120
use_tls_alpn_challenge = true
ech_enabled = true
```

- `dns_provider` ממופה למימוש הרשום ב-`acme::DnsProvider`. Route53 מסופק כברירת מחדל; ניתן להוסיף ספקים נוספים ללא הידור מחדש של השער.
- ערך `ech_enabled = true` גורם לזרימת האוטומציה ליצור תצורת ECH לצד התעודה. אם ה-CDN או הלקוחות אינם תומכים, יש להגדיר `false` (ראו את ספר ההפעלה להלן).
- `renewal_threshold_seconds` מגדיר מתי תתחיל הזמנה חדשה; ברירת המחדל היא 30 יום וניתן להקטין בסביבות רגישות.
- מנגנון הניתוח נמצא ב-`iroha_torii::sorafs::gateway::config`, ושגיאות יאומתו בלוגי ההפעלה של Torii.

### טבלת תצורה

| מפתח | ברירת מחדל | ציפייה בפרודקשן | דרישת ציות |
|-----|------------|------------------|------------|
| `torii.sorafs_gateway.acme.enabled` | `true` | להשאיר פעיל, למעט במהלך תרגילי אירוע או טיפול בתקלות. | כיבוי האוטומציה חייב להיות מתועד ביומן השינויים עם מזהה אירוע/כרטיס. |
| `torii.sorafs_gateway.acme.directory_url` | Let’s Encrypt v2 | להחליף רק בעת מעבר בין רשויות תעודה (CA). | הממשל דורש לפרסם את ה-CA הנבחר במניפסטי GAR. |
| `torii.sorafs_gateway.acme.dns_provider` | `route53` | לציין שם ספק הרשום בבקר האוטומציה. | מדיניות ה-IAM של הספק נסקרת אחת לרבעון. |
| `torii.sorafs_gateway.acme.renewal_threshold_seconds` | `2_592_000` (30 יום) | להתאים לרמת הסיכון (לא פחות מ-14 יום). | שינוי הסף מחייב אישור של בעל הסיכון. |
| `torii.sorafs_gateway.acme.retry_backoff_seconds` | `900` | לשמור על ≤ 900 כדי לעמוד ב-SLA של GAR. | ערך גבוה מ-900 דורש ויתור מתועד ביומן הממשל. |
| `torii.sorafs_gateway.acme.ech_enabled` | `true` | להפעיל/לכבות רק בהתאם לספר ההפעלה. | כל שינוי חייב להיות מתועד עם סיבה וראיות במאגר הציות. |
| `torii.sorafs_gateway.acme.telemetry_topic` | `sorafs-tls` | להשאיר ברירת מחדל אלא אם נדרש ניתוב לוגים ייעודי. | שינוי הערך מחייב רישום הערוץ החדש אצל צוות התצפית לצורכי שמירת נתונים. |

בעת יישום שינוי ידני יש לעדכן את `iroha_config::actual::sorafs_gateway` (או את מערכת ניהול הקונפיגורציה שלכם), לצרף דיפ לכרטיס השינוי ולהעלות חבילת self-cert חדשה לאחר הפריסה.

### שירות האוטומציה

- יש להפעיל את יחידת ה-systemd המצורפת לחבילת האוטומציה:

  ```bash
  systemctl enable --now sorafs-gateway-acme@production.service
  ```

- יומני הריצה נרשמים ליעד `sorafs_gateway_tls_automation`. מומלץ להזרים אותם למערכת הלוגים המרכזית ולתייג לפי סביבה כדי להקל על ניתוח אירועים.
- סטטוס מימוש: `iroha_torii::sorafs::gateway::acme::AcmeAutomation` מספק טיפול דטרמיניסטי בצ'אלנג'ים DNS-01/TLS-ALPN-01, ג'יטר/באק-אוף ניתנים להגדרה ושימור מצב חידוש.

### אימות ואטסטציה

1. לאחר החלת ההגדרות יש לטעון מחדש את Torii:

   ```bash
   systemctl reload iroha-torii.service
   ```

2. יש להריץ את חבילת ה-self-cert כדי לוודא כותרות, טלמטריה ואינטגרציית GAR:

   ```bash
   cargo xtask sorafs-self-cert --check-tls \
     --gateway https://gateway.example.com \
     --out artifacts/sorafs_gateway_self_cert
   ```

3. יש לשמור את הדו"ח יחד עם מעטפת ה-GAR לצורך עקיבות.
4. סטטוס מימוש: `iroha_torii::sorafs::gateway::telemetry` חושף את `SORA_TLS_STATE_HEADER`, את `Metrics::set_sorafs_tls_state` ואת `Metrics::record_sorafs_tls_renewal` שבהם משתמשת החבילה.

## מדריך הטלמטריה

| ממשק | שם | תיאור | התראה / פעולה |
|---------|------|-------------|----------------|
| מטריקה | `sorafs_gateway_tls_cert_expiry_seconds` | מספר השניות עד תוקף התעודה הפעילה. | התראה כאשר `< 1_209_600` (14 יום). |
| מטריקה | `sorafs_gateway_tls_renewal_total{result}` | מונה ניסיונות חידוש עם תוויות `success`/`error`. | חקירה אם שיעור הכשל עולה על 5% בשעה. |
| מטריקה | `sorafs_gateway_tls_ech_enabled` | מד 0/1 המשקף את מצב ה-ECH. | התראה כאשר הופך ל-`0` ללא תכנון. |
| מטריקה | `torii_sorafs_gar_violations_total{reason,detail}` | מונה הפרות GAR. | הסלמה מיידית לממשל והוספת הלוגים. |
| כותרת | `X-Sora-TLS-State` | מצב שמצורף לתשובות (לדוגמה `ech-enabled;expiry=2025-06-12T12:00:00Z`). | ניטור סינתטי; במצב `ech-disabled`/`degraded` להפעיל את ספרי ההפעלה. |
| לוג | `journalctl -u sorafs-gateway-acme@*.service` | רשומות חידוש, כשלים בצ'אלנג'ים והת干לות ידניות. | לשמור כנלוות לכרטיסי אירוע ולתרגילים. |

יש לחשוף את המטריקות ב-Prometheus/OpenTelemetry, לבנות לוחות מחוונים למגמות חידוש/תוקף וליצור בדיקות סינתטיות שבודקות את `X-Sora-TLS-State` מדי שעה.

### חיבור התרעות

- **מרווח תוקף:** יש להגדיר התראת אזהרה כאשר `sorafs_gateway_tls_cert_expiry_seconds` יורד מתחת ל-14 יום והתראת קריטית מתחת ל-7 ימים. ההתרעה צריכה לנתב לספר ההפעלה של **רוטציית תעודה חירומית**.
- **כשלי חידוש:** יש לפתוח אירוע כאשר `sorafs_gateway_tls_renewal_total{result="error"}` גדל ביותר משלוש פעמים בפרק זמן של שש שעות, ולצרף לוגים של בקר האוטומציה.
- **הפרות GAR:** יש להעביר התראות מתוך `torii_sorafs_gar_violations_total` ישירות לערוץ הממשל, כדי שניתן יהיה לאשר חריגות או להסיט תעבורה.
- **סטיית מצב ECH:** יש להתריע לצוות חוויית המפתחים כאשר `sorafs_gateway_tls_ech_enabled` עובר מ-`1` ל-`0` ללא תיעוד מתוכנן, כדי לעדכן את צוותי ה-SDK והאופרטורים.

## חיבורי מדיניות GAR

- כאשר השער מסרב בקשה, המונה `torii_sorafs_gar_violations_total{policy_reason,policy_detail}` גדל ו-Alertmanager יכול להדליק נהלי ממשל אוטומטיים.
- Torii משדר אירוע `DataEvent::Sorafs(GarViolation)` עם מזהי ספק, נתוני רשימת חסימה והקשר רייט-לימיט; מומלץ להתחבר ל-SSE או Webhook של הצינור הממשלי.
- הלוגים מכילים שדות מבניים `policy_reason`, `policy_detail`, `provider_id_hex`, כך שקל יותר לבצע חקירה פורנזית ולספק חומרי ביקורת.

## תאימות וממשל

על המפעילים לעמוד בדרישות הבאות כדי לשמור על תאימות לרשת Nexus:

- **יישור GAR:** לאחר כל חידוש יש לפרסם טביעת אצבע חדשה במניפסט GAR ולהגיש למועצת הממשל לוגים, חבילת self-cert וטביעת אצבע של האטסטציה.
- **שמירת לוגים:** יש לשמור לוגי הפרות GAR למשך 180 יום לפחות ולכלול אותם בדוחות הרבעוניים.
- **שמירת אטסטציות:** יש לארכיון כל יצוא של `cargo xtask sorafs-self-cert` תחת `artifacts/sorafs_gateway_tls/<YYYYMMDD>/` ולאפשר גישה לקריאת מבקרים.
- **ניהול שינויי קונפיגורציה:** יש לתעד כל שינוי ב-`torii.sorafs_gateway`, כולל הסיבה להפעלת/כיבוי `ech_enabled` או שינוי ספי החידוש.
- **ביצוע תרגילים:** יש לבצע את התרגילים המתוארים ולהתעדכן בתוך שלושה ימי עסקים.

### רשימת תיעוד לציות

| חובה | מה לאסוף | תקופת שמירה | בעל תפקיד |
|------|-----------|-------------|-----------|
| יישור GAR | מניפסט GAR מעודכן, חבילת טביעת אצבע חתומה, קישור לכרטיס האירוע/השינוי. | 3 שנים | רכז הממשל |
| רישום מדיניות | יצואי `torii_sorafs_gar_violations_total`, לוגים מבניים, התראות Alertmanager. | 180 יום | צוות התצפית |
| שמירת אטסטציות | דוח `cargo xtask sorafs-self-cert`, צילום כותרת TLS, טביעת אצבע OpenSSL. | 3 שנים | צוות תפעול השער |
| ניהול שינויים | דיפ `torii.sorafs_gateway`, רישום אישור, חותמת זמן פריסה. | שנתיים | מנהל השינויים |
| תיעוד תרגילים | רשומת מעקב, רשימת משתתפים, משימות המשך. | שנתיים | רכז הכאוס |
| אירועי ECH | יומן שינוי, הודעה חתומה לצוותי SDK/אופס, צילום טלמטריה לפני/אחרי. | שנתיים | צוות חוויית המפתחים |

## ספרי הפעלה

### רוטציית תעודה חירומית

**תנאי הפעלה**
- `sorafs_gateway_tls_cert_expiry_seconds` < 1,209,600 (14 יום).
- `sorafs_gateway_tls_renewal_total{result="failure"}` הופיע פעמיים במהלך חלון החידוש.
- השדה `X-Sora-TLS-State` כולל `last-error=` או שהלקוחות מדווחים על כשל בהנד-שייק/אי התאמת תעודה.

**ייצוב**
1. יש להזעיק את צוות התורן דרך PagerDuty ולפתוח אירוע בערוץ `#sorafs-incident`.
2. יש להשהות הפצות קונפיגורציה ולציין את מזהה הקומיט הנוכחי בכרטיס האירוע.
3. יש להגדיר `torii.sorafs_gateway.acme.enabled = false`, להחיל את השינוי ולהפעיל מחדש את פריסת השער כדי לעצור את האוטומציה.

**הנפקה ופריסה של חבילה חדשה**
1. יש לאסוף מצב נוכחי לצורכי ביקורת:
   ```bash
   curl -sD - https://gateway.example/status \
     | grep -i '^x-sora-tls-state'
   openssl s_client -connect gateway.example:443 -servername gateway.example \
     < /dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256
   ```
2. יש להריץ את עטיפת המאגר כדי להזמין חבילה חדשה לדטרמיניזם:
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   הפקודה מפיקה את `fullchain.pem`, `privkey.pem` ו-`ech.json` בתיקיית `pending`.
3. יש להעתיק את הקבצים למאגר הסודות ולהטעין מחדש את Torii:
   ```bash
   install -m 600 /var/lib/sorafs_gateway_tls/pending/fullchain.pem /etc/iroha/sorafs_gateway_tls/fullchain.pem
   install -m 600 /var/lib/sorafs_gateway_tls/pending/privkey.pem  /etc/iroha/sorafs_gateway_tls/privkey.pem
   install -m 640 /var/lib/sorafs_gateway_tls/pending/ech.json     /etc/iroha/sorafs_gateway_tls/ech.json
   systemctl reload iroha-torii.service
   ```

**אימות וחזרה לאוטומציה**
1. יש להריץ את חבילת ה-self-cert:
   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --gateway https://gateway.example \
     --cert /etc/iroha/sorafs_gateway_tls/fullchain.pem \
     --ech-config /etc/iroha/sorafs_gateway_tls/ech.json \
     --out artifacts/sorafs_gateway_self_cert
   ```
2. יש לאשר שהטלמטריה התאוששה:
   - `sorafs_gateway_tls_cert_expiry_seconds` > 2,592,000 (30 יום).
   - `sorafs_gateway_tls_renewal_total{result="success"}` עלה באחד.
   - `X-Sora-TLS-State` מציג `ech-enabled;expiry=…;renewed-at=…` ללא `last-error`.
3. יש לעדכן את תוצרי הממשל (מניפסט GAR וחבילת האטסטציה) ולצרף אותם לכרטיס האירוע.
4. יש להחזיר את `torii.sorafs_gateway.acme.enabled = true`, לטעון מחדש את Torii, להמשיך פריסות ולהגיש דוח סיכום בתוך שלושה ימי עסקים.

### מעבר למצב ECH מדורג / מופחת

ספר הפעלה זה מיועד כאשר CDN או לקוחות אינם מצליחים להשלים ECH.

1. יש לזהות את המצב דרך `sorafs_gateway_tls_ech_enabled == 0`, דרך אירועי לקוחות (לדוגמה `GREASE_ECH_MISMATCH`) או דרך הנחיית הממשל.
2. יש לבטל את ECH:
   ```toml
   [torii.sorafs_gateway.acme]
   ech_enabled = false
   ```
   יש להחיל את הקונפיגורציה (או להשתמש ב-`iroha_cli config apply`) ולהפעיל מחדש את Torii. יש לוודא ש-`X-Sora-TLS-State` מציג `ech-disabled`.
3. יש לעדכן מפעילי אחסון, צוותי SDK והממשל, כולל חלון הזמן הצפוי לבחינה.
4. יש לעקוב אחרי `sorafs_gateway_tls_renewal_total{result="failure"}` במהלך הדגרציה ולהבטיח שה- TLS נשאר יציב.
5. לאחר ההתאוששות, יש להריץ את העטיפה החדשה כדי ליצור חבילה דטרמיניסטית:
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   הפקודה מפיקה את קבצי ה-PEM בתיקיית `pending`; לאחר מכן יש להריץ שוב את חבילת ה-self-cert, להחזיר `ech_enabled = true` ולשתף את קטעי הטלמטריה המעודכנים.

### ביטול בעקבות חשיפת מפתח פרטי

ספר הפעלה זה משמש אם קיים חשד לחשיפת מפתח או הודעת CA על הנפקה שגויה.

1. יש להשאיר את האוטומציה כבויה ולבצע רוטציה של מפתח חתימת אסימוני הזרם בהתאם לחוברת ההפעלה של הפעילות, כדי למנוע שימוש חוזר.
2. יש לבטל את החבילה הנוכחית באמצעות העטיפה החדשה (הקבצים מועברים לתיקיית `revoked/`).
   ```bash
   scripts/sorafs-gateway tls revoke \
     --out /var/lib/sorafs_gateway_tls \
     --reason keyCompromise
   ```
   הפקודה יוצרת ארכיון מתוארך וקובץ ביקורת JSON; יש לשמור אותם יחד עם דוח האירוע.
3. יש להנפיק חבילה חדשה באמצעות `scripts/sorafs-gateway tls renew --out /var/lib/sorafs_gateway_tls/pending --force` ולבצע את תהליך הבדיקה של ספר ההפעלה הקודם.
4. יש לפרסם מעטפות GAR חדשות ואת `manifest_signatures.json` כדי שספקי המשנה יאמצו את טביעת האצבע החדשה.
5. יש להודיע למועצת הממשל ולצוותי ה-SDK על מזהה האירוע, זמן הביטול והפעולות שננקטו.

## תדירות התרגילים ותיעוד ראיות

| תרגיל | תדירות | תרחיש | מדדי הצלחה |
|-------|-----------|----------|------------------|
| `tls-renewal` | רבעוני | הרצת ספר ההפעלה בסביבת סטייג'ינג (כולל כיבוי/הדלקה של האוטומציה). | מסתיים בפחות מ-15 דקות, הטלמטריה מעודכנת והחומרים נשמרים. |
| `ech-fallback` | פעמיים בשנה | ECH כבוי לשעה והחזרתו. | הודעות נשלחו, `X-Sora-TLS-State` מציג את שני המצבים ואין התראות פתוחות. |
| `tls-revocation` | פעם בשנה | הדמיית חשיפת מפתח ופסילת התעודה בסטייג'ינג. | ביטול מאושר, חבילה חדשה נפרסה ומניפסט GAR עודכן. |

לאחר כל תרגיל:
- יש לארכיון יומני אוטומציה, צילומי Prometheus ואת פלט ה-self-cert בתוך `artifacts/sorafs_gateway_tls/<YYYYMMDD>/`.
- יש לעדכן את רישום התרגילים ב-`docs/source/sorafs_chaos_plan.md` עם המשתתפים, משך הזמן, תובנות ומשימות המשך.
- יש לפתוח משימות בהמשך הדרך (SF-5 או SF-7) עבור תקלות אוטומציה ולצרף ראיות.

## פתרון תקלות

- **כשלי DNS-01 חוזרים בלוג:** לבדוק הרשאות IAM של `dns_provider` ולוודא תפוצה של TXT באמצעות `dig _acme-challenge.gateway.example.com txt`.
- **`X-Sora-TLS-State` חסר או שגוי:** לבדוק ש-Torii הוטען מחדש לאחר עדכון הקבצים ולחפש בלוגים אזהרות ממימוש `Metrics::set_sorafs_tls_state`.
- **מונה הפרות GAR עולה:** לבחון את הלוגים המבניים של `torii_sorafs_gar_violations_total`, לתקן את הספק או המניפסט הפגומים ולהודיע לממשל לפני השבת תעבורה.
- **כשלים בלקוחות גם לאחר ביטול ECH:** לנקות מטמון ב-CDN (Cloudflare/CloudFront) ולספק ללקוחות שמות מארח חלופיים.

## הערות מימוש

- **ספריית לקוח ACME:** נעשה שימוש ב-`letsencrypt-rs` שתומכת בצ'אלנג'ים DNS-01 ו-TLS-ALPN-01 ומשתלבת עם המפעיל הא-סינכרוני של השער תחת רישיון Apache-2.0.
- **הפשטת ספק DNS:** Route53 נתמך כברירת מחדל. ניתן להוסיף ספקים כמו Cloudflare או Google Cloud DNS באמצעות הממשק `DnsProvider` בלי לשנות את הבקר. יש להגדיר עם `acme.dns_provider = "<provider>"`.
- **יישור שמות טלמטריה:** השמות בטבלה אושרו על ידי צוות התצפיתיות, וזמני המדדים נמדדים בשניות. לאחר שדרוג יש לוודא שהלוחות מביאים את הסכמה המעודכנת.
- **שילוב self-cert:** כל חידוש מפעיל את ערכת ה-self-cert. הקובץ `sorafs_gateway_tls_automation.yml` מריץ את `cargo xtask sorafs-self-cert --check-tls` לפני שליחת הודעות כדי לוודא את תקפות TLS/ECH.
