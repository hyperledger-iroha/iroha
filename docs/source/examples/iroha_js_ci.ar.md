---
lang: ar
direction: rtl
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2026-01-03T18:08:00.440949+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# مرجع Iroha JS CI

تقوم الحزمة `@iroha/iroha-js` بتجميع الارتباطات الأصلية عبر `iroha_js_host`. أي
يجب أن يوفر خط أنابيب CI الذي ينفذ الاختبارات أو البناءات وقت تشغيل Node.js
وسلسلة أدوات Rust بحيث يمكن تجميع الحزمة الأصلية قبل الاختبارات
تشغيل.

## الخطوات الموصى بها

1. استخدم إصدار Node LTS (18 أو 20) عبر `actions/setup-node` أو CI الخاص بك
   ما يعادلها.
2. قم بتثبيت سلسلة أدوات Rust المدرجة في `rust-toolchain.toml`. نحن نوصي
   `dtolnay/rust-toolchain@v1` في إجراءات جيثب.
3. قم بتخزين فهارس تسجيل/بوابة البضائع ودليل `target/` لتجنب
   إعادة بناء الملحق الأصلي في كل وظيفة.
4. قم بتشغيل `npm install`، ثم `npm run lint:test`. يفرض النص المدمج
   يقوم ESLint بدون أي تحذيرات بإنشاء الملحق الأصلي وتشغيل اختبار العقدة
   جناح بحيث يتطابق CI مع سير عمل بوابة الإصدار.
5. اختياريًا، قم بتشغيل `node --test` كخطوة دخان سريعة مرة واحدة `npm run build:native`
   قام بإنتاج الملحق (على سبيل المثال، الإرسال المسبق لممرات الفحص السريع التي يمكن إعادة استخدامها
   القطع الأثرية المخزنة مؤقتا).
6. قم بطبقة أي عمليات فحص أو تنسيق إضافية من المستهلك الخاص بك
   المشروع أعلى `npm run lint:test` عندما تكون هناك حاجة إلى سياسات أكثر صرامة.
7. عند مشاركة التكوين عبر الخدمات، قم بتحميل `iroha_config` وتمرير الملف
   تم تحليل المستند إلى `resolveToriiClientConfig({ config })` لذلك عملاء Node
   إعادة استخدام نفس سياسة المهلة/إعادة المحاولة/الرمز المميز مثل بقية عملية النشر (انظر
   `docs/source/sdk/js/quickstart.md` للحصول على مثال كامل).

## قالب إجراءات جيثب

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

## وظيفة الدخان السريع (اختياري)

بالنسبة لطلبات السحب التي تتطرق فقط إلى الوثائق أو تعريفات TypeScript، أ
يمكن لمهمة الحد الأدنى إعادة استخدام العناصر المخزنة مؤقتًا، وإعادة بناء الوحدة الأصلية، وتشغيل
عداء اختبار العقدة مباشرة:

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

تكتمل هذه المهمة بسرعة مع الاستمرار في التحقق من تجميع الملحق الأصلي
وأن مجموعة اختبار العقدة قد نجحت.

> **التنفيذ المرجعي:** يتضمن المستودع
> `.github/workflows/javascript-sdk.yml`، الذي يربط الخطوات المذكورة أعلاه في ملف
> مصفوفة العقدة 18/20 مع التخزين المؤقت للبضائع.