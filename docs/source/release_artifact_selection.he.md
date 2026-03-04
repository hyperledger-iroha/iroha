---
lang: he
direction: rtl
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/release_artifact_selection.md -->

# בחירת ארטיפקטי ריליס של Iroha

מסמך זה מבהיר אילו ארטיפקטים (bundles ותמונות קונטיינר) על מפעילים לפרוס עבור כל פרופיל ריליס.

## פרופילים

- **iroha2 (Self-hosted networks)** — תצורת lane יחיד התואמת ל-`defaults/genesis.json` ו-`defaults/client.toml`.
- **iroha3 (SORA Nexus)** — תצורת Nexus מרובת lanes המשתמשת בתבניות `defaults/nexus/*`.

## Bundles (בינארים)

ה-bundles נוצרים באמצעות `scripts/build_release_bundle.sh` עם `--profile` שמוגדר ל-`iroha2` או `iroha3`.

כל tarball כולל:

- `bin/` — `irohad`, `iroha`, ו-`kagami` שנבנו עם פרופיל הפריסה.
- `config/` — תצורת genesis/client לפי הפרופיל (single מול nexus). bundles של Nexus כוללים `config.toml` עם פרמטרי lanes ו-DA.
- `PROFILE.toml` — מטא-דאטה המתאר פרופיל, קונפיג, גרסה, commit, OS/arch ומערך ה-features המופעל.
- ארטיפקטי מטא-דאטה שנכתבים לצד ה-tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` ו-`.pub` (כאשר מסופק `--signing-key`)
  - `<profile>-<version>-manifest.json` המתעד את נתיב ה-tarball, hash ופרטי החתימה

## תמונות קונטיינר

תמונות קונטיינר נוצרות באמצעות `scripts/build_release_image.sh` עם אותם ארגומנטים של פרופיל/קונפיג.

פלט:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- חתימה/מפתח ציבורי אופציונלי (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` המתעד tag, מזהה תמונה, hash ומטא-דאטה של חתימה

## בחירת הארטיפקט הנכון

1. קבעו את משטח הפריסה:
   - **SORA Nexus / multi-lane** -> השתמשו ב-bundle וב-image של `iroha3`.
   - **Self-hosted single-lane** -> השתמשו בארטיפקטים של `iroha2`.
   - אם יש ספק, הריצו `scripts/select_release_profile.py --network <alias>` או `--chain-id <id>`; ה-helper ממפה רשתות לפרופיל הנכון לפי `release/network_profiles.toml`.
2. הורידו את ה-tarball הרצוי ואת קבצי ה-manifest הנלווים. אמתו את ה-hash של SHA256 ואת החתימה לפני חילוץ:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. חלצו את ה-bundle (`tar --use-compress-program=zstd -xf <tar>`) והציבו את `bin/` ב-PATH של הפריסה. החילו overrides של קונפיג מקומי לפי הצורך.
4. טענו את תמונת הקונטיינר עם `docker load -i <profile>-<version>-<os>-image.tar` אם אתם משתמשים בפריסות קונטיינריות. אמתו את ה-hash/חתימה כפי שמפורט לעיל לפני הטעינה.

## רשימת בדיקה לתצורת Nexus

- `config/config.toml` חייב לכלול את הסעיפים `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, ו-`[nexus.da]`.
- ודאו שכללי ניתוב lanes תואמים את ציפיות ה-governance (`nexus.routing_policy`).
- אמתו שספי DA (`nexus.da`) ופרמטרי fusion (`nexus.fusion`) מתואמים להגדרות שאושרו על ידי המועצה.

## רשימת בדיקה לתצורת single-lane

- `config/config.d` (אם קיים) צריך להכיל רק overrides של single-lane, ללא סעיפי `[nexus]`.
- ודאו ש-`config/client.toml` מפנה ל-endpoint של Torii ולרשימת ה-peers הרצויה.
- ה-genesis צריך לשמור על domains/assets הקנוניים עבור רשת self-hosted.

## רפרנס קצר לכלים

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — זרימת onboarding מקצה לקצה למפעילי data-space של Sora Nexus לאחר בחירת הארטיפקטים.

</div>
