---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a02b5241a292129b1c5ccfd25b0ae82be62076d21d493f94021b0fa6783d9f83
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: جولة في السجل
description: اعادة انتاج تدفق حتمي register -> mint -> transfer باستخدام CLI `iroha` والتحقق من حالة السجل الناتجة.
slug: /norito/ledger-walkthrough
---

تكمل هذه الجولة [Norito quickstart](./quickstart.md) عبر توضيح كيفية تعديل حالة السجل وفحصها باستخدام CLI `iroha`. ستسجل تعريف اصل جديدا، وتسك وحدات في حساب المشغل الافتراضي، وتنقل جزءا من الرصيد الى حساب اخر، وتتحقق من المعاملات والممتلكات الناتجة. كل خطوة تعكس التدفقات المغطاة في quickstarts الخاصة ب Rust/Python/JavaScript لتتمكن من التحقق من التطابق بين CLI وسلوك SDK.

## المتطلبات المسبقة

- اتبع [quickstart](./quickstart.md) لتشغيل شبكة بعقدة واحدة عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- تاكد من ان `iroha` (الـ CLI) مبني او محمل وانك تستطيع الوصول الى الـ peer باستخدام `defaults/client.toml`.
- ادوات اختيارية: `jq` (تنسيق ردود JSON) وصدفة POSIX لمقاطع متغيرات البيئة في الاسفل.

طوال الدليل، استبدل `$ADMIN_ACCOUNT` و `$RECEIVER_ACCOUNT` بمعرفات الحساب التي تخطط لاستخدامها. يتضمن الـ bundle الافتراضي بالفعل حسابين مشتقين من مفاتيح العرض:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

اكد القيم عبر سرد اولى الحسابات:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة genesis

ابدأ باستكشاف السجل الذي يستهدفه CLI:

```sh
# Domains المسجلة في genesis
iroha --config defaults/client.toml domain list all --table

# Accounts داخل wonderland (استبدل --limit بعدد اكبر عند الحاجة)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions الموجودة مسبقا
iroha --config defaults/client.toml asset definition list all --table
```

تعتمد هذه الاوامر على ردود مدعومة ب Norito، لذا يكون الترشيح والتقسيم حتميين ومتطابقين مع ما تتلقاه SDKs.

## 2. تسجيل تعريف اصل

انشئ اصلا جديدا قابلا للسك بلا حدود باسم `coffee` داخل نطاق `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

يطبع CLI hash المعاملة المقدمة (مثلا `0x5f…`). احفظه كي تستعلم عن الحالة لاحقا.

## 3. سك وحدات في حساب المشغل

توجد كميات الاصول تحت الزوج `(asset definition, account)`. اسك 250 وحدة من `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` في `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

مرة اخرى احفظ hash المعاملة (`$MINT_HASH`) من خرج CLI. للتحقق من الرصيد نفذ:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

او لاستهداف الاصل الجديد فقط:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. نقل جزء من الرصيد الى حساب اخر

انقل 50 وحدة من حساب المشغل الى `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

احفظ hash المعاملة باسم `$TRANSFER_HASH`. استعلم عن الممتلكات في الحسابين للتحقق من الارصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من ادلة السجل

استخدم الهاشات المحفوظة لتاكيد ان المعاملتين تم التزامهما:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

يمكنك ايضا بث الكتل الحديثة لمعرفة اي كتلة تضمنت التحويل:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

تستخدم كل الاوامر اعلاه نفس payloads الخاصة ب Norito التي تستخدمها SDKs. اذا كررت هذا التدفق عبر الكود (انظر quickstarts للـ SDK ادناه)، فستتطابق الهاشات والارصدة ما دمت تستهدف الشبكة نفسها والافتراضات نفسها.

## روابط تكافؤ SDK

- [Rust SDK quickstart](../sdks/rust) — يوضح تسجيل التعليمات، ارسال المعاملات، واستطلاع الحالة من Rust.
- [Python SDK quickstart](../sdks/python) — يعرض نفس عمليات register/mint مع مساعدات JSON مدعومة ب Norito.
- [JavaScript SDK quickstart](../sdks/javascript) — يغطي طلبات Torii، ومساعدات الحوكمة، واغلفة الاستعلامات المtyped.

نفذ جولة CLI اولا ثم كرر السيناريو باستخدام SDK المفضل لديك للتأكد من تطابق السطحين في هاشات المعاملات والارصدة ومخرجات الاستعلام.
