<!-- Hebrew translation of docs/source/examples/iroha_js_ci.md -->

---
lang: he
direction: rtl
source: docs/source/examples/iroha_js_ci.md
status: complete
translator: manual
---

<div dir="rtl">

<!--
  SPDX-License-Identifier: Apache-2.0
  המסמך מסביר כיצד להריץ את @iroha/iroha-js במערכות CI.
-->

# מדריך CI עבור Iroha JS

חבילת `@iroha/iroha-js` כוללת כריכות ילידיות דרך `iroha_js_host`. כדי להריץ בדיקות או לבנות את החבילה ב-CI יש לספק גם סביבת Node.js וגם את כלי Rust, כך שהמודול הילידי ייקומפל לפני הרצת הבדיקות.

## שלבים מומלצים

1. השתמשו בגרסת LTS של Node (18 או 20) באמצעות `actions/setup-node` או מקבילה בכלי ה-CI שלכם.
2. התקינו את כלי ה-Rust המופיע ב-`rust-toolchain.toml`. ב-GitHub Actions מומלץ להשתמש ב-`dtolnay/rust-toolchain@v1`.
3. הטמיעו Cache עבור ה-Registry וה-Git של cargo וכן עבור תיקיית `target/` כדי למנוע קומפילציה מחדש בכל ריצה.
4. הריצו `npm install`, אחריו `npm run build:native`, ולבסוף `npm test`.
5. כאשר אין צורך בצינור ה-`npm` המלא (למשל בדיקות עשן מהירות לפני merge), ניתן להריץ `node --test`.
6. ניתן להוסיף בדיקות לינט (`npx eslint`) או בדיקות פורמט בפרויקט הצרכן לפי הצורך—ה-SDK עצמו אינו כופה סטייל.

## תבנית GitHub Actions

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run build:native
      - run: npm test
```

## משימת Smoke מהירה (אופציונלי)

עבור Pull Request שמעדכנים רק תיעוד או קבצי TypeScript, ניתן להריץ משימה קומפקטית שמנצלת Cache קיים, מקמפלת מחדש את המודול הילידי ומפעילה את ראנר הבדיקות של Node:

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

המשימה קצרה אך מאמתת שהמודול הילידי מתקמפל ושראנר הבדיקות של Node עובר בהצלחה.

</div>
