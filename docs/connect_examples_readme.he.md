<!-- Hebrew translation of docs/connect_examples_readme.md -->

---
lang: he
direction: rtl
source: docs/connect_examples_readme.md
status: complete
translator: manual
---

<div dir="rtl">

## דוגמאות Iroha Connect (אפליקציית Rust / ארנק)

להלן זרימה מלאה של שני הקבצים לדוגמה יחד עם צומת Torii.

דרישות מקדימות
- צומת Torii עם `connect` מופעל ב-`http://127.0.0.1:8080`.
- סביבת Rust (stable).
- Python‎ 3.9 ומעלה עם החבילה ‎`iroha-python`‎ (עבור כלי ה-CLI בהמשך).

דוגמאות
- דוגמת אפליקציה: `crates/iroha_torii_shared/examples/connect_app.rs`
- דוגמת ארנק: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- כלי CLI ב-Python: `python -m iroha_python.examples.connect_flow`

סדר הפעלה
1) טרמינל א' — אפליקציה (מדפיסה sid + אסימונים, מתחברת ל-WS, שולחת SignRequestTx):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   פלט לדוגמה:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) טרמינל ב' — ארנק (מתחבר עם token_wallet, משיב ב-SignResultOk):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   פלט לדוגמה:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) טרמינל האפליקציה מציג את התוצאה:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  בעת פענוח המטען השתמשו ב-`connect_norito_decode_envelope_sign_result_alg` (ובתיבות העטיפה ל-Swift/Kotlin בתיקייה זו) כדי לשחזר את שם האלגוריתם.

הערות:
- הדוגמאות מפיקות מפתחות ארעיים מ-`sid`, כך שהאפליקציה והארנק מסתנכרנים אוטומטית. אין להשתמש בזרימה זו בפרודקשן.
- ה-SDK כופה קשירת AEAD AAD ושימוש ב-`seq` כ-nonce; פריימי שליטה אחרי אישור צריכים להישלח כשהם מוצפנים.
- לקוחות Swift יכולים להיעזר ב-`docs/connect_swift_integration.md` / `docs/connect_swift_ios.md`. לאחר שינוי יש להריץ `make swift-ci` כדי לוודא שפריות הפיקסצ'רים ופידי הדשבורד נשארים מיושרים, ושהמטא-דאטה `ci/xcframework-smoke:<lane>:device_tag` ממשיך להופיע ב-Buildkite.
- דוגמת CLI ב-Python:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  הכלי מציג את פרטי הסשן ואת מצב ה-Connect בפורמט קריא, ומפיק פריים `ConnectControlOpen` בקידוד Norito. ניתן להוסיף `--send-open` לשליחה אל Torii, לבחור `--frame-output-format binary` לשמירה כבתים גולמיים, להשתמש ב-`--frame-json-output` לייצוא JSON ידידותי ל-REST, ו-`--status-json-output` לכתיבת תמונת מצב טיפוסית להמשך אוטומציה. אפשר גם לטעון מטא-דאטה של האפליקציה מקובץ JSON בעזרת `--app-metadata-file metadata.json` שבו השדות `name`, `url`, `icon_hash` (ראו `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). ניתן ליצור תבנית חדשה עם `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`. לצפייה במצב בלבד אפשר להריץ `--status-only` (ולבחירה גם `--status-json-output status.json`) וכך לדלג על יצירת סשן.
</div>
