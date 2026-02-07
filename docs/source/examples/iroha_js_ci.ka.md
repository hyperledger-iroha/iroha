---
lang: ka
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2025-12-29T18:16:35.953373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha JS CI მითითება

`@iroha/iroha-js` პაკეტი აერთიანებს ბუნებრივ კავშირებს `iroha_js_host`-ის მეშვეობით. ნებისმიერი
CI მილსადენი, რომელიც ახორციელებს ტესტებს ან აშენებებს, უნდა უზრუნველყოს ორივე Node.js გაშვების დრო
და Rust-ის ხელსაწყოების ჯაჭვი, რათა მშობლიური პაკეტის შედგენა შესაძლებელია ტესტებამდე
გაშვება.

## რეკომენდებული ნაბიჯები

1. გამოიყენეთ Node LTS გამოშვება (18 ან 20) `actions/setup-node` ან თქვენი CI მეშვეობით
   ექვივალენტი.
2. დააინსტალირეთ Rust ინსტრუმენტთა ჯაჭვი, რომელიც ჩამოთვლილია `rust-toolchain.toml`-ში. ჩვენ გირჩევთ
   `dtolnay/rust-toolchain@v1` GitHub Actions-ში.
3. ქეშით ტვირთის რეესტრი/git ინდექსები და `target/` დირექტორია, რათა თავიდან აიცილოთ
   მშობლიური დანამატის აღდგენა ყველა სამუშაოში.
4. გაუშვით `npm install`, შემდეგ `npm run lint:test`. კომბინირებული სკრიპტი ძალაშია
   ESLint ნულოვანი გაფრთხილებით, აშენებს მშობლიურ დამატებას და აწარმოებს Node ტესტს
   კომპლექტი ისე, რომ CI ემთხვევა გამოშვების კარიბჭის სამუშაო პროცესს.
5. სურვილისამებრ გაუშვით `node --test` როგორც სწრაფი კვამლის ნაბიჯი ერთხელ `npm run build:native`
   შექმნა დანამატი (მაგალითად, წინასწარ გაგზავნეთ სწრაფი შემოწმების ზოლები, რომლებიც ხელახლა გამოიყენება
   ქეშირებული არტეფაქტები).
6. მოათავსეთ ნებისმიერი დამატებითი ლინტინგის ან ფორმატირების შემოწმება თქვენი მომხმარებლისგან
   პროექტი `npm run lint:test`-ის თავზე, როცა უფრო მკაცრი პოლიტიკაა საჭირო.
7. სერვისებში კონფიგურაციის გაზიარებისას ჩატვირთეთ `iroha_config` და გაიარეთ
   გაანალიზებულია დოკუმენტი `resolveToriiClientConfig({ config })`-ში, რათა Node კლიენტებს
   ხელახლა გამოიყენეთ იგივე დროის ამოწურვის/ხელახალი ცდის/ჟეტონის პოლიტიკა, როგორც დანარჩენი განლაგება (იხ
   `docs/source/sdk/js/quickstart.md` სრული მაგალითისთვის).

## GitHub მოქმედებების შაბლონი

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
      - run: npm run lint:test
```

## სწრაფი მოწევის სამუშაო (სურვილისამებრ)

pull მოთხოვნებისთვის, რომლებიც ეხება მხოლოდ დოკუმენტაციას ან TypeScript განმარტებებს, a
მინიმალურ სამუშაოს შეუძლია ხელახლა გამოიყენოს ქეშირებული არტეფაქტები, აღადგინოს მშობლიური მოდული და გაუშვას
კვანძის ტესტირება პირდაპირ:

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

ეს სამუშაო სწრაფად სრულდება, ხოლო ჯერ კიდევ ამოწმებს, რომ მშობლიური დანამატი შედგენილია
და რომ Node ტესტის კომპლექტი გადის.

> **მიმართვის განხორციელება:** საცავი მოიცავს
> `.github/workflows/javascript-sdk.yml`, რომელიც აკავშირებს ზემოთ მოცემულ ნაბიჯებს ა
> კვანძის 18/20 მატრიცა ტვირთის ქეშირებით.