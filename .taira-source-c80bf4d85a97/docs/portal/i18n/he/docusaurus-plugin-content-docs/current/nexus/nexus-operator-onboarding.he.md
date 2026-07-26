---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9224b69afbd10e72f9776ebd4955f694a5ead05af70500dc5f209e98a8fcaac5
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-operator-onboarding
title: קליטת מפעילי data-space של Sora Nexus
description: מראה של `docs/source/sora_nexus_operator_onboarding.md`, העוקב אחר צ'קליסט release מקצה לקצה למפעילי Nexus.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sora_nexus_operator_onboarding.md`. שמרו על יישור שתי הגרסאות עד שהמהדורות המקומיות יגיעו לפורטל.
:::

# קליטת מפעילי Data-Space של Sora Nexus

מדריך זה מתעד את הזרימה מקצה לקצה שמפעילי data-space של Sora Nexus חייבים לבצע לאחר הכרזת release. הוא משלים את runbook הדו-מסלולי (`docs/source/release_dual_track_runbook.md`) ואת הערת בחירת הארטיפקטים (`docs/source/release_artifact_selection.md`) על ידי תיאור איך ליישר bundles/images שהורדו, manifests ותבניות תצורה עם ציפיות ה-lane הגלובליות לפני העלאת צומת לאוויר.

## קהל יעד ודרישות מקדימות
- אושרת על ידי תוכנית Nexus וקיבלת את השיוך ל-data-space (אינדקס lane, data-space ID/alias ודרישות מדיניות ניתוב).
- יש לך גישה לארטיפקטים חתומים של ה-release שפורסמו על ידי Release Engineering (tarballs, images, manifests, signatures, public keys).
- יצרת או קיבלת חומר מפתחות פרודקשן לתפקיד validator/observer (זהות צומת Ed25519; מפתח קונצנזוס BLS + PoP ל-validators; וכן כל toggle של תכונות סודיות).
- יש לך גישה ל-peers קיימים של Sora Nexus שיבצעו bootstrap לצומת שלך.

## שלב 1 - אימות פרופיל ה-release
1. זהה את ה-alias של הרשת או את chain ID שקיבלת.
2. הרץ `scripts/select_release_profile.py --network <alias>` (או `--chain-id <id>`) על checkout של הריפו הזה. ה-helper קורא את `release/network_profiles.toml` ומדפיס את הפרופיל לפריסה. עבור Sora Nexus התשובה חייבת להיות `iroha3`. עבור כל ערך אחר, עצור ופנה ל-Release Engineering.
3. רשום את tag הגרסה מהכרזת ה-release (למשל `iroha3-v3.2.0`); תשתמש בו כדי להביא artifacts ו-manifests.

## שלב 2 - שליפה ואימות ארטיפקטים
1. הורד את חבילת `iroha3` (`<profile>-<version>-<os>.tar.zst`) ואת הקבצים הנלווים (`.sha256`, אופציונלי `.sig/.pub`, `<profile>-<version>-manifest.json`, ו-`<profile>-<version>-image.json` אם אתה מפרוס קונטיינרים).
2. אמת את השלמות לפני פריסה:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   החלף את `openssl` במאמת המאושר בארגון אם אתה משתמש ב-KMS עם חומרה.
3. בדוק את `PROFILE.toml` בתוך ה-tarball ואת manifests ה-JSON כדי לוודא:
   - `profile = "iroha3"`
   - השדות `version`, `commit`, ו-`built_at` תואמים להכרזת ה-release.
   - ה-OS/ארכיטקטורה תואמים ליעד הפריסה.
4. אם אתה משתמש בתמונת קונטיינר, חזור על בדיקת hash/signature עבור `<profile>-<version>-<os>-image.tar` ואשר את image ID הרשום ב-`<profile>-<version>-image.json`.

## שלב 3 - הכנת תצורה מתוך תבניות
1. חלץ את החבילה והעתק את `config/` למיקום שבו הצומת יקרא את התצורה.
2. התייחס לקבצים תחת `config/` כתבניות:
   - החלף `public_key`/`private_key` במפתחות Ed25519 של פרודקשן. הסר מפתחות פרטיים מהדיסק אם הצומת משתמש ב-HSM; עדכן את התצורה כך שתצביע על מחבר ה-HSM.
   - התאם את `trusted_peers`, `network.address`, ו-`torii.address` כך שישקפו את הממשקים הנגישים ואת peers ה-bootstrap שהוקצו לך.
   - עדכן את `client.toml` עם endpoint של Torii למפעילים (כולל תצורת TLS אם רלוונטי) ועם האישורים שהקצית לכלי התפעול.
3. שמור על chain ID שסופק בחבילה אלא אם Governance הורה אחרת במפורש - ה-lane הגלובלי מצפה למזהה שרשרת קנוני יחיד.
4. תכנן להפעיל את הצומת עם דגל פרופיל Sora: `irohad --sora --config <path>`. טוען התצורה ידחה הגדרות SoraFS או multi-lane כאשר הדגל חסר.

## שלב 4 - יישור מטאדטה של data-space ומדיניות ניתוב
1. ערוך את `config/config.toml` כך שהקטע `[nexus]` יתאים לקטלוג ה-data-space שסיפקה Nexus Council:
   - `lane_count` חייב להיות שווה למספר ה-lanes הפעילות בתקופה הנוכחית.
   - כל רשומה ב-`[[nexus.lane_catalog]]` וב-`[[nexus.dataspace_catalog]]` חייבת לכלול `index`/`id` ייחודי וה-aliases שסוכמו. אל תמחק את הרשומות הגלובליות הקיימות; הוסף את ה-aliases שהוקצו לך אם המועצה הוסיפה data-spaces נוספים.
   - ודא שכל רשומת dataspace כוללת `fault_tolerance (f)`; ועדות lane-relay מוגדרות כ-`3f+1`.
2. עדכן את `[[nexus.routing_policy.rules]]` כדי לשקף את המדיניות שניתנה לך. התבנית ברירת המחדל מנתבת הוראות ממשל ל-lane `1` ופריסות חוזים ל-lane `2`; הוסף או עדכן כללים כך שהתנועה ל-data-space שלך תנותב ל-lane ול-alias הנכונים. תאם עם Release Engineering לפני שינוי סדר הכללים.
3. בדוק את הספים של `[nexus.da]`, `[nexus.da.audit]`, ו-`[nexus.da.recovery]`. מצופה מהמפעילים לשמור על הערכים שאושרו על ידי המועצה; עדכן רק אם מדיניות חדשה אושרה.
4. רשום את התצורה הסופית במעקב התפעולי שלך. ה-runbook הדו-מסלולי דורש לצרף את `config.toml` הסופי (עם סודות מודחקים) לכרטיס הקליטה.

## שלב 5 - בדיקות טרום הפעלה
1. הרץ את מאמת התצורה המובנה לפני הצטרפות לרשת:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   פעולה זו מדפיסה את התצורה המיושרת ונכשלת מוקדם אם רשומות הקטלוג/ניתוב אינן עקביות או אם genesis וה-config אינם תואמים.
2. אם אתה מפרוס קונטיינרים, הרץ את אותה פקודה בתוך התמונה אחרי טעינה עם `docker load -i <profile>-<version>-<os>-image.tar` (זכור לכלול `--sora`).
3. בדוק לוגים לאזהרות על מזהי lane/data-space placeholder. אם מופיעות אזהרות, חזור לשלב 4 - פריסות פרודקשן אינן אמורות להסתמך על מזהי placeholder שמגיעים עם התבניות.
4. הרץ את תהליך ה-smoke המקומי שלך (לדוגמה, שלח שאילתת `FindNetworkStatus` באמצעות `iroha_cli`, ודא ש-endpoints של הטלמטריה חושפים `nexus_lane_state_total`, ואמת שמפתחות ה-streaming הוחלפו או יובאו לפי הצורך).

## שלב 6 - Cutover ו-hand-off
1. שמור את `manifest.json` המאומת ואת ארטיפקטי החתימה בכרטיס ה-release כדי שמבקרים יוכלו לשחזר את הבדיקות שלך.
2. הודע ל-Nexus Operations שהצומת מוכן להכנסה; כלול:
   - זהות הצומת (peer ID, hostnames, endpoint של Torii).
   - ערכי קטלוג lane/data-space ומדיניות הניתוב בפועל.
   - Hashes של הבינארים/תמונות שאימתת.
3. תאם את הקליטה הסופית של peers (gossip seeds והקצאת lane) עם `@nexus-core`. אל תצטרף לרשת עד שתתקבל אישור; Sora Nexus אוכף תפוסת lanes דטרמיניסטית ודורש manifest קבלה מעודכן.
4. לאחר שהצומת פעיל, עדכן את ה-runbooks שלך בכל overrides שהוספת וציין את tag ה-release כדי שהאיטרציה הבאה תתחיל מה-baseline הזה.

## Checklist רפרנס
- [ ] פרופיל release אומת כ-`iroha3`.
- [ ] Hashes וחתימות של ה-bundle/image אומתו.
- [ ] מפתחות, כתובות peers ו-endpoints של Torii עודכנו לערכי פרודקשן.
- [ ] קטלוג lanes/dataspace ומדיניות ניתוב Nexus תואמים להקצאת המועצה.
- [ ] מאמת תצורה (`irohad --sora --config ... --trace-config`) עובר ללא אזהרות.
- [ ] manifests/חתימות נשמרו בכרטיס הקליטה ו-Ops קיבל הודעה.

להקשר רחב יותר על שלבי המיגרציה של Nexus וציפיות הטלמטריה, עיין ב-[Nexus transition notes](./nexus-transition-notes).
