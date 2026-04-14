---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: جولة في السجل
الوصف: إعادة تدفق إنتاج حتمي سجل -> نعناع -> نقل باستخدام CLI `iroha` والتحقق من حالة السجل الموجودة.
سبيكة: /norito/ledger-walkthrough
---

تكمل هذه الجولة [Norito Quickstart](./quickstart.md) عبر التوضيح كيفية تعديل حالة السجل وفحصها باستخدام CLI `iroha`. ستسجل تعريف اصل جديدا، وتسك وحدات في الحساب الافتراضي، والتنقل جزء من الرصيد الى الحساب اخر، والتحقق من المعاملات والممتلكات المملوكة. كل خطوة احترامات الناشطة المغطاة في Quickstarts الخاصة ب Rust/Python/JavaScript لتتمكن من التحقق من التطابق بين CLI وسلوك SDK.

##المتطلبات المسبقة

- اتبع [البدء السريع](./quickstart.md) لتشغيل شبكة بعقدة واحدة عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- متأكد من ان `iroha` (الـ CLI) مبنية على ربط الوصول وانك فعال الى الـ Peer باستخدام `defaults/client.toml`.
- ادوات اختيارية: `jq` (تنسيق ردود JSON) وصدفة POSIX لمقاطع متغيرات البيئة في الاسفل.

IP الدليل، استبدل `$ADMIN_ACCOUNT` و `$RECEIVER_ACCOUNT` بمعرفات الحساب التي تخطط لاستخدامها. تتضمن ـ الحزمة الافتراضية بالفعل حسابين مشتقين من مفاتيح العرض:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

قيم القيم عبر سرد اولى آت:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

##1.حالة فحص التكوين

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

تعتمد هذه الاوامر على ردود مدعومة ب Norito، لذلك يكون الترشيحات والتقسيم حتميين ومتطابقين مع ما تتلقاها SDKs.

## 2. تسجيل تعريف اصلانشئ اصلا جديدا قابلا للسك بلا حدود باسم I18NI00 داخل نطاق00029X `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

يطبع CLI hash مميزة (مثلا `0x5f…`). احفظه كي يتعلم عن الحالة لاحقا.

## 3. وحدات سك في حسابات التشغيل

هناك المزيد من الاصول تحت الزوج `(asset definition, account)`. اسك 250 وحدة من `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` في `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

مرة اخرى احفظ التجزئة (`$MINT_HASH`) من التخلص من CLI. ينفذ من الرصيد:

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

انقل 50 وحدة من الحسابات لتشغيل `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

احفظ التجزئة حسب `$TRANSFER_HASH`. استعلم عن وجود فيالكين من الارصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من السجل المعادل

استخدم الهاشات المحفوظة لتاكيد ان العاملتين وقد وعدهما:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

يمكنك أيضًا بث الكتل الحديثة لتعلم أي كتلة بديلة:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

تستخدم كل الاوامر اعلاه نفس الحمولات الخاصة بـ Norito التي تستخدمها SDKs. إذا كررت هذا التعاون عبر الكود (انظر Quickstarts لـ SDK ادناه)، فستتطابق الهاشات والارصدة ما دمت الشبكة نفسها والافتراضات نفسها.

## روابط تكافؤ SDK- [Rust SDK Quickstart](../sdks/rust) — يوضح تسجيل التعليمات، إرسال المعاملات، ومعلومات الحالة من Rust.
- [Python SDK Quickstart](../sdks/python) — نفس المنتجات المصنعة Register/mint مع مساعدات JSON مدعومة ب Norito.
- [JavaScript SDK Quickstart](../sdks/javascript) — يغطي الطلبات Torii، ومساعدات ال تور، وتسجيلات غير المكتوبة.

نفذ جولة CLI أولا ثم كرر السيناريو باستخدام SDK المفضل لديك للتأكد من تطابق التوافقين في هاشات المعاملات والرصدة ومخرجات.