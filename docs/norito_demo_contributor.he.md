<!-- Hebrew translation of docs/norito_demo_contributor.md -->

---
lang: he
direction: rtl
source: docs/norito_demo_contributor.md
status: complete
translator: manual
---

<div dir="rtl">

# מדריך תורמים ל-Norito SwiftUI Demo

המסמך מתאר את השלבים הידניים הנדרשים להפעלת הדמו של SwiftUI מול צומת Torii מקומי ולדג'ר מדומה. הוא משלים את `docs/norito_bridge_release.md` ומתמקד במשימות פיתוח יומיומיות, ובמידה וצריך מדריך שילוב מלא ב-Xcode (כולל NoritoBridge ו-Connect) ניתן לעיין גם ב-`docs/connect_swift_integration.md`.

## הכנת סביבת העבודה

1. התקינו את כלי ה-Rust המוגדרים ב-`rust-toolchain.toml`.
2. התקינו Swift 5.7 ומעלה ואת כלי הפקודה של Xcode ב-macOS.
3. (אופציונלי) התקינו את [SwiftLint](https://github.com/realm/SwiftLint) לצורך linting.
4. הריצו `cargo build -p irohad` כדי לוודא שהצומת מתקמפל.
5. העתיקו את `examples/ios/NoritoDemoXcode/Configs/demo.env.example` ל-`.env` ועדכנו את הערכים לסביבה שלכם. האפליקציה קוראת את המשתנים הבאים בעת ההפעלה:
   - `TORII_NODE_URL` — כתובת הבסיס של REST (ה-WebSocket נגזר ממנה).
   - `CONNECT_SESSION_ID` — מזהה סשן בגודל 32 בתים (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — האסימונים שמוחזרים מ-`/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — מזהה השרשרת שנשלח במסגרת בקרת Connect.
   - `CONNECT_ROLE` — התפקיד שמופיע כברירת מחדל בממשק (`app` או `wallet`).
   - עזרים אופציונליים לבדיקות: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`.
   - `NORITO_ACCEL_CONFIG_PATH` — (אופציונלי) מסלול לקובץ `iroha_config` בפורמט JSON/TOML
     שמכיל הגדרות Metal/NEON. אם אינו מוגדר, נעשה שימוש בקבצים המוטמעים או בברירות המחדל.

## Bootstrap ל-Torii + לדג'ר מדומה

המאגר כולל סקריפט שמפעיל צומת Torii עם לדג'ר בזיכרון:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

הסקריפט מפיק:

- לוגים של Torii ל-`artifacts/torii.log`.
- מדדי לדג'ר (Prometheus) ל-`artifacts/metrics.prom`.
- אסימוני גישה לקליינט ב-`artifacts/torii.jwt`.

הסקריפט נשאר פעיל עד לחיצה על `Ctrl+C`. הוא יוצר מצב מוכנות ב-`artifacts/ios_demo_state.json`, שומר את לוג stdout, בודק את `/metrics` עד שניתן למשוך את הנתונים ומייצר JWT לפי הקונפיג. פרמטרים זמינים:

- `--artifacts` – שינוי ספריית הפלט.
- `--telemetry-profile` – התאמה לפרופיל Telemetry.
- `--exit-after-ready` – סיום אוטומטי לצורכי CI.

מבנה `SampleAccounts.json`:

- `name` (אופציונלי) – נשמר כחשבון Metadata `alias`.
- `public_key` (חובה) – משמש כחותם החשבון.
- `private_key` (אופציונלי) – נכלל ב-`torii.jwt` לטובת לקוחות.
- `domain` (אופציונלי) – ברירת מחדל לדומיין הנכס.
- `asset_id` (חובה) – נכס למינטה לחשבון.
- `initial_balance` (חובה) – כמות שמוזרקת לחשבון.

## הרצת הדמו ב-SwiftUI

1. בנו את ה-XCFramework לפי `docs/norito_bridge_release.md` והוסיפו ל-root של הפרויקט (`NoritoBridge.xcframework`).
2. פתחו את `NoritoDemoXcode` ב-Xcode.
3. בחרו סכימה `NoritoDemo` והפעילו סימולטור או מכשיר.
4. ודאו שהקובץ `.env` נטען דרך הגדרות הסביבה בסכמה, ומלאו את ערכי `CONNECT_*` שנוצרו על ידי `/v1/connect/session` כדי שהטפסים ימולאו אוטומטית בעת ההפעלה.
5. בדקו את הגדרות ההאצה: `App.swift` קורא `DemoAccelerationConfig.load().apply()`
   כך שהדמו יחיל אוטומטית את הקובץ שמוגדר ב-`NORITO_ACCEL_CONFIG_PATH` או את הקבצים
   `acceleration.{json,toml}`/`client.{json,toml}` שנמצאים בחבילה. ניתן להסיר/להחליף קבצים
   אלו כדי לכפות ריצת CPU לפני ההפעלה.
6. קומפלו והפעילו את האפליקציה. המסך הראשי יבקש פרטי Torii אם לא הוגדרו.
7. פתחו סשן "Connect" למנויים/אישורים.
8. בצעו העברת IRH ועקבו אחר הלוגים והלוגים של Torii.

### מתגי האצה (Metal / NEON)

`DemoAccelerationConfig` משקף את קובץ `iroha_config` של הצומת כדי למנוע שכפול הגדרות.
בעת ההפעלה הוא מחפש לפי הסדר:

1. `NORITO_ACCEL_CONFIG_PATH` — נתיב מוחלט/`~` לקובץ JSON/TOML עם הסעיף `[accel]`.
2. קבצים מוטמעים בשם `acceleration.{json,toml}` או `client.{json,toml}` בתוך החבילה.
3. אם לא נמצא דבר, נותרות ברירות המחדל (`AccelerationSettings()`).

דוגמת `acceleration.toml`:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

אם הקובץ לא מכיל `[accel]` או שמסלול ההאצה אינו זמין (למשל בסימולטור ללא Metal), המנוע נשאר במסלול ה-CPU באופן דטרמיניסטי.

## בדיקות אינטגרציה

- הבדיקות יישבו תחת `Tests/NoritoDemoTests` (יתווספו כאשר CI ל-macOS יהיה זמין).
- הן מפעילות צומת Torii באמצעות הסקריפטים ובודקות מנויים ב-WebSocket, יתרות ואינטראקציות.
- לוגים נשמרים ב-`artifacts/tests/<timestamp>/` לצד מדדים ודאמפים של לדג'ר.

## בדיקות פרי-PR ו-CI

- לפני שליחת PR שנוגע לדמו או לפיקסצ'רים, הריצו `make swift-ci`. היעד מבצע בדיקות פריטי, ולידציית JSON של הדשבורד ורנדר CLI.
- ב-CI העבודה נשענת על מטא-דאטה של Buildkite (`ci/xcframework-smoke:<lane>:device_tag`) כדי לזהות באיזה ליין (למשל `iphone-sim`, ‏`strongbox`) נאספו התוצאות; אם שיניתם את הפייפליין או תגי הסוכנים, ודאו שהמטא-דאטה מופיע.
- במקרה של כישלון, פעלו לפי `docs/source/swift_parity_triage.md` ועיינו בפלט של `mobile_ci` כדי לזהות איזה ליין דורש ריג'נרציה או טיפול באינסידנט.

## פתרון תקלות

- אם הדמו לא מתחבר ל-Torii, בדקו כתובת ו-TLS.
- ודאו שאסימון JWT תקין ולא פג תוקף.
- עיינו ב-`artifacts/torii.log` לשגיאות בצד השרת.
- לבעיות WebSocket, בדקו את חלון הלוג באפליקציה או את מסוף Xcode.

</div>
