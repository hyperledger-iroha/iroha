---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5af9275262d327866a96e8dc77c52b478d676494cedaff4fb9c50167473b4aa3
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: developer-releases
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


# תהליך שחרור

הבינארים של SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) וה-crates של ה-SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) יוצאים יחד. צינור השחרור
שומר על יישור בין ה-CLI והספריות, מבטיח כיסוי lint/test ולוכד ארטיפקטים עבור
צרכנים downstream. הריצו את הצ'קליסט למטה עבור כל תג מועמד.

## 0. אישור חתימת ביקורת אבטחה

לפני הרצת שער השחרור הטכני, לכדו את ארטיפקטי ביקורת האבטחה העדכניים:

- הורידו את מזכר ביקורת האבטחה האחרון של SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  ורשמו את ה-hash SHA256 שלו בכרטיס השחרור.
- צרפו את קישור כרטיס הרמדייישן (למשל `governance/tickets/SF6-SR-2026.md`) וציינו
  את המאשרים מ-Security Engineering ומ-Tooling Working Group.
- ודאו שהצ'קליסט במזכר סגור; פריטים לא פתורים חוסמים את השחרור.
- היערכו להעלאת לוגים של harness parity (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  יחד עם bundle ה-manifest.
- ודאו שפקודת החתימה שאתם מתכננים להריץ כוללת גם `--identity-token-provider` וגם
  `--identity-token-audience=<aud>` מפורש כדי ללכוד את תחום Fulcio בעדויות השחרור.

כללו את הארטיפקטים הללו בעת הודעה לגאברננס ופרסום השחרור.

## 1. הרצת שער השחרור/בדיקות

העזר `ci/check_sorafs_cli_release.sh` מריץ עיצוב, Clippy ובדיקות על
ה-crates של CLI ו-SDK עם ספריית target מקומית ל-workspace (`.target`)
כדי למנוע התנגשויות הרשאות בעת הרצה בקונטיינרי CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

הסקריפט מבצע את הבדיקות הבאות:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` עבור `sorafs_car` (עם feature `cli`),
  `sorafs_manifest` ו-`sorafs_chunker`
- `cargo test --locked --all-targets` עבור אותם crates

אם שלב כלשהו נכשל, תקנו את הרגרסיה לפני tagging. Builds של שחרור חייבים
להיות רציפים עם main; אל תבצעו cherry-pick לתיקונים בענפי release. השער גם
בודק שדגלי חתימה ללא מפתח (`--identity-token-issuer`, `--identity-token-audience`)
מסופקים היכן שנדרש; ארגומנטים חסרים מפילים את ההרצה.

## 2. החלת מדיניות גרסאות

כל ה-crates של CLI/SDK ב-SoraFS משתמשים ב-SemVer:

- `MAJOR`: נכנס לשימוש בשחרור 1.0 הראשון. לפני 1.0 העלאת minor `0.y`
  **מציינת שינויים שוברים** במשטח ה-CLI או בסכמות Norito.
- `MINOR`: עבודה תואמת לאחור (פקודות/דגלים חדשים, שדות Norito חדשים מאחורי
  מדיניות אופציונלית, תוספות טלמטריה).
- `PATCH`: תיקוני באגים, שחרורים של תיעוד בלבד ועדכוני תלות שלא משנים התנהגות
  נצפית.

שמרו תמיד על `sorafs_car`, `sorafs_manifest` ו-`sorafs_chunker` באותה גרסה כדי
שצרכני SDK downstream יוכלו להסתמך על מחרוזת גרסה אחת. בעת העלאת גרסאות:

1. עדכנו את שדות `version =` בכל `Cargo.toml`.
2. בצעו רגנרציה ל-`Cargo.lock` דרך `cargo update -p <crate>@<new-version>` (ה-workspace
   אוכף גרסאות מפורשות).
3. הריצו שוב את שער השחרור כדי לוודא שאין ארטיפקטים ישנים.

## 3. הכנת הערות שחרור

כל שחרור חייב לפרסם changelog ב-markdown שמדגיש שינויים שמשפיעים על CLI, SDK
וגאברננס. השתמשו בתבנית שב-`docs/examples/sorafs_release_notes.md` (העתיקו אותה
לתיקיית ארטיפקטי השחרור ומלאו את הסעיפים בפרטים קונקרטיים).

תוכן מינימלי:

- **Highlights**: כותרות פיצ'רים עבור צרכני CLI ו-SDK.
- **תאימות**: שינויים שוברים, שדרוגי מדיניות, דרישות מינימום ל-gateway/node.
- **צעדי שדרוג**: פקודות TL;DR לעדכון תלות cargo והרצת fixtures דטרמיניסטיים מחדש.
- **אימות**: hashes של פלט פקודות או envelopes והגרסה המדויקת של
  `ci/check_sorafs_cli_release.sh` שהורצה.

צרפו את הערות השחרור המלאות לתג (למשל גוף שחרור GitHub) ואחסנו אותן לצד
ארטיפקטים שנוצרו דטרמיניסטית.

## 4. הרצת hooks לשחרור

הריצו `scripts/release_sorafs_cli.sh` כדי לייצר bundle חתימה ותקציר אימות
שמצורפים לכל שחרור. העטיפה בונה את ה-CLI בעת הצורך, קוראת ל-
`sorafs_cli manifest sign`, ומיד מריצה `manifest verify-signature` כדי לחשוף
כשלונות לפני tagging. דוגמה:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

טיפים:

- עקבו אחרי קלטי השחרור (payload, plans, summaries, hash טוקן צפוי) בריפו או
  בקונפיג פריסה כדי שהסקריפט יישאר שחזורי. ה-bundle של fixtures תחת
  `fixtures/sorafs_manifest/ci_sample/` מציג את ה-layout הקנוני.
- בנו אוטומציה של CI על `.github/workflows/sorafs-cli-release.yml`; הוא מריץ
  את שער השחרור, מפעיל את הסקריפט ומארכב bundles/חתימות כארטיפקטי workflow.
  שיקפו את אותו סדר פקודות (שער שחרור → חתימה → אימות) במערכות CI אחרות כך
  שלוגי הביקורת יתיישרו עם ה-hashes שנוצרו.
- שמרו את `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  ו-`manifest.verify.summary.json` יחד — הם מהווים את החבילה המוזכרת בהודעת
  הממשל.
- כאשר השחרור מעדכן fixtures קנוניים, העתיקו את ה-manifest המעודכן, את chunk plan
  ואת ה-summaries אל `fixtures/sorafs_manifest/ci_sample/` (ועדכנו
  `docs/examples/sorafs_ci_sample/manifest.template.json`) לפני tagging. מפעילים
  downstream תלויים ב-fixtures המחויבים כדי לשחזר את ה-bundle.
- לכדו את לוג הריצה של אימות bounded-channels עבור `sorafs_cli proof stream`
  וצרפו אותו לחבילת השחרור כדי להוכיח שההגנות של proof streaming פעילות.
- רשמו את `--identity-token-audience` המדויק ששימש בחתימה בהערות השחרור;
  הממשל מצליב את ה-audience מול מדיניות Fulcio לפני אישור הפרסום.

השתמשו ב-`scripts/sorafs_gateway_self_cert.sh` כאשר השחרור כולל גם rollout של
gateway. הצביעו על אותו bundle של manifest כדי להוכיח שה-attestation תואם
לארטיפקט המועמד:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tagging ופרסום

לאחר שהבדיקות עברו וה-hooks הושלמו:

1. הריצו `sorafs_cli --version` ו-`sorafs_fetch --version` כדי לאשר שהבינארים
   מדווחים על הגרסה החדשה.
2. הכינו את קונפיג השחרור ב-`sorafs_release.toml` שמנוהל בריפו (מועדף) או בקובץ
   קונפיג אחר שמנוהל בריפו הפריסה. הימנעו מהסתמכות על משתני סביבה אד-הוק;
   העבירו נתיבים ל-CLI עם `--config` (או שקול) כדי שהקלטים יהיו מפורשים ושחזוריים.
3. צרו tag חתום (מועדף) או tag annotated:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. העלו ארטיפקטים (bundles של CAR, manifests, סיכומי proofs, הערות שחרור,
   פלטי attestation) לרג'יסטרי הפרויקט לפי צ'קליסט הממשל במדריך
   [הפריסה](./developer-deployment.md). אם השחרור יצר fixtures חדשים, דחפו אותם
   לריפו fixtures משותף או ל-object store כדי שאוטומציית ביקורת תוכל להשוות את
   ה-bundle שפורסם מול בקרת המקור.
5. הודיעו לערוץ הממשל עם קישורים לתג החתום, הערות השחרור, hashes של bundle/חתימות
   ה-manifest, סיכומי `manifest.sign/verify` המאורכבים וכל מעטפת attestation. כללו
   את כתובת ה-job ב-CI (או ארכיון לוגים) שהריץ `ci/check_sorafs_cli_release.sh` ו-
   `scripts/release_sorafs_cli.sh`. עדכנו את כרטיס הממשל כדי שהמבקרים יוכלו לעקוב
   אחרי האישור עד לארטיפקטים; כאשר `.github/workflows/sorafs-cli-release.yml`
   מפרסם הודעות, קישרו את ה-hashes המתועדים במקום להדביק סיכומים אד-הוק.

## 6. מעקב לאחר שחרור

- ודאו שהמסמכים שמצביעים על הגרסה החדשה (quickstarts, תבניות CI) עודכנו
  או אשרו שלא נדרשים שינויים.
- הוסיפו פריטי roadmap אם נדרש המשך עבודה (למשל דגלי מיגרציה, הוצאת manifests
- ארכבו את לוגי פלט שער השחרור למבקרים — אחסנו אותם לצד הארטיפקטים החתומים.

מעקב אחר צינור זה שומר את ה-CLI, ה-crates של ה-SDK וחומרי הממשל מיושרים בכל
מחזור שחרור.
