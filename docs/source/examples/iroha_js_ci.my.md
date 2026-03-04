---
lang: my
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

#Iroha JS CI ကိုးကား

`@iroha/iroha-js` ပက်ကေ့ဂျ်သည် `iroha_js_host` မှတစ်ဆင့် မူရင်းချည်နှောင်မှုများကို စုစည်းထားသည်။ တစ်ခုခု
စမ်းသပ်မှုများ သို့မဟုတ် တည်ဆောက်မှုများကို လုပ်ဆောင်သည့် CI ပိုက်လိုင်းသည် Node.js runtime နှစ်ခုလုံးကို ပေးဆောင်ရမည်ဖြစ်သည်။
နှင့် Rust toolchain တို့သည် စမ်းသပ်မှုများမပြုလုပ်မီ မူလအစုအဝေးကို စုစည်းနိုင်စေရန်
ပြေး

## အကြံပြုထားသော အဆင့်များ

1. `actions/setup-node` သို့မဟုတ် သင်၏ CI မှတဆင့် Node LTS (18 သို့မဟုတ် 20) ကို အသုံးပြုပါ။
   ညီမျှသည်။
2. `rust-toolchain.toml` တွင်ဖော်ပြထားသော Rust toolchain ကို ထည့်သွင်းပါ။ အကြံပြုလိုပါတယ်။
   GitHub လုပ်ဆောင်ချက်များတွင် `dtolnay/rust-toolchain@v1`။
3. ရှောင်ရှားရန် ကုန်တင်စာရင်းသွင်းခြင်း/ git အညွှန်းများနှင့် `target/` လမ်းညွှန်ကို သိမ်းဆည်းပါ
   အလုပ်တိုင်းတွင် မူလ addon ကို ပြန်လည်တည်ဆောက်ခြင်း။
4. `npm install`၊ ထို့နောက် `npm run lint:test` ကိုဖွင့်ပါ။ ပေါင်းစပ်ဇာတ်ညွှန်းကို ပြဋ္ဌာန်းထားသည်။
   သုညသတိပေးချက်များနှင့်အတူ ESLint၊ မူရင်း addon ကိုတည်ဆောက်ပြီး Node စမ်းသပ်မှုကို လုပ်ဆောင်သည်။
   suite ဖြစ်သောကြောင့် CI သည် release gating workflow နှင့် ကိုက်ညီပါသည်။
5. `node --test` ကို အမြန်မီးခိုးအဆင့်အဖြစ် `npm run build:native` တစ်ကြိမ်တွင် ရွေးချယ်နိုင်သည်
   addon ကို ထုတ်လုပ်ခဲ့သည် (ဥပမာ၊ ပြန်လည်အသုံးပြုသည့် အမြန်စစ်ဆေးသည့်လမ်းများကို ကြိုတင်တင်ပြပါ။
   သိမ်းဆည်းထားသော ရှေးဟောင်းပစ္စည်းများ)။
6. သင့်စားသုံးသူထံမှ နောက်ထပ် linting သို့မဟုတ် formatting စစ်ဆေးမှုများကို အလွှာပြုလုပ်ပါ။
   တင်းကျပ်သောမူဝါဒများ လိုအပ်သည့်အခါ `npm run lint:test` ၏ထိပ်တွင် ပရောဂျက်။
7. ဝန်ဆောင်မှုများတစ်လျှောက် ဖွဲ့စည်းမှုပုံစံကို မျှဝေသည့်အခါ၊ `iroha_config` ကို တင်ပြီး ဖြတ်သန်းပါ။
   စာရွက်စာတမ်းကို `resolveToriiClientConfig({ config })` သို့ ခွဲခြမ်းစိတ်ဖြာပြီး Node clients များ
   တူညီသော အချိန်ကုန်/ကြိုးစားမှု/တိုကင်မူဝါဒကို အခြားအသုံးပြုမှုအဖြစ် ပြန်လည်အသုံးပြုပါ (ကြည့်ပါ။
   နမူနာအပြည့်အစုံအတွက် `docs/source/sdk/js/quickstart.md`)။

## GitHub လုပ်ဆောင်ချက် နမူနာပုံစံ

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

## Fast Smoke Job (ရွေးချယ်ခွင့်)

စာရွက်စာတမ်းများ သို့မဟုတ် TypeScript အဓိပ္ပါယ်ဖွင့်ဆိုချက်များကိုသာ ထိသောဆွဲယူတောင်းဆိုမှုများအတွက်၊ a
အနည်းဆုံးအလုပ်သည် ကက်ရှ်လုပ်ထားသော ရှေးဟောင်းပစ္စည်းများကို ပြန်သုံးနိုင်ပြီး မူလ module ကို ပြန်လည်တည်ဆောက်နိုင်ပြီး ၎င်းကို လုပ်ဆောင်နိုင်သည်။
Node test runner ကို တိုက်ရိုက်-

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

မူရင်း addon သည် compile လုပ်ကြောင်း အတည်ပြုနေချိန်တွင် ဤအလုပ်သည် လျင်မြန်စွာပြီးမြောက်ပါသည်။
Node test suite ပြီးသွားပါပြီ။

> **အကိုးအကား အကောင်အထည်ဖော်မှု-** repository တွင် ပါဝင်သည်။
> `.github/workflows/javascript-sdk.yml`၊ အထက်အဆင့်များကို ဝါယာကြိုးတစ်ခုအဖြစ်သို့ ပြောင်းပါ။
> ကုန်တင်သိမ်းဆည်းခြင်းနှင့်အတူ Node 18/20 matrix