---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Посаговый разбой еестра
الوصف: قم بتسجيل نقطة التحديد -> النعناع -> النقل باستخدام CLI `iroha` وتحقق من هذا السجل.
سبيكة: /norito/ledger-walkthrough
---

تتضمن هذه الإرشادات التفصيلية [Norito Quickstart](./quickstart.md)، توضح كيفية مراقبة السجل الموجود والتحقق منه باستخدام واجهة سطر الأوامر (CLI) `iroha`. قم بتسجيل تعريف جديد للنشاط، وقم بإيداع وحدات في حساب المشغل الافتراضي، وحقق جزءًا من التوازن في قم بالحساب الآخر والتحقق من هذه المعاملات والودائع. في كل مرة يتم فيها حذف النقاط التي تم إنشاؤها في Quickstart SDK Rust/Python/JavaScript، يمكنك إعادة بناء التكافؤ بين CLI وتحسين SDK.

## تريبوفانيا

- اتبع [quickstart](./quickstart.md)، لإلغاء تعيين آخر مرة أخرى
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- تأكد من أن `iroha` (CLI) تم اختباره أو حذفه، وما يمكنك تقديمه إلى
  النظير عبر `defaults/client.toml`.
- التعزيزات الاختيارية: `jq` (تنسيق تنسيق JSON) وPOSIX Shell من أجل
  مقتطفات من التخفيضات المؤقتة.

اتبع جميع التعليمات لمتطلباتك `$ADMIN_ACCOUNT` و`$RECEIVER_ACCOUNT`
حسابات الهوية. يوجد في الحزمة الافتراضية حسابان مستفيدان من المفاتيح التجريبية:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

قم بالإجابة على الأسئلة السابقة بالحسابات الأولى:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. نشأة الطبيعة

نصائح لتعلم العداد من CLI الأصلي:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```يتم تشغيل هذه الأوامر على Norito، التصفية والصفحات
المحددون والمتوافقون مع الموضوع الذي يمكنهم من خلاله الحصول على SDK.

## 2. قم بتسجيل التعريف النشط

قم بإنشاء نشط جديد تمامًا لـ Mintable `coffee` في المجال `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

يعرض CLI المعاملات الصحيحة الأخرى (على سبيل المثال، `0x5f…`). إنه صاحب الذات
حاول التحقق من الحالة.

## 3. قم بإضافة وحدات حساب المشغل

كل الكائنات الحية النشطة تحت `(asset definition, account)`. خذ 250
الوحدة `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` إلى `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

جميع المعاملات الجديدة المخزنة (`$MINT_HASH`) من خلال CLI. للتحقق من التوازن،
الاختيار:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

أو للحصول على نشاط جديد تمامًا:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. تحقق من جزء من رصيد الحساب الآخر

قم بالاطلاع على 50 يومًا من حساب مشغل الهاتف `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

هذه المعاملات المضمنة مثل `$TRANSFER_HASH`. حماية ممتلكاتهم من الحسابات,
للتحقق من الأرصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من سجل الإحالة

استخدم الأشياء المخبأة لتأكيد التزامك بالمعاملة:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

يمكنك أيضًا تقليل الكتل التالية لمعرفة كيفية تضمين الكتلة في النقل:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```جميع الأوامر السابقة تستخدم حمولات Norito، وهي SDK. إذا قمت بالإجابة
هذه السرعة في الكود (sm. Quickstarts SDK جيدة)، والتوازن والتوازن مع الاستخدام،
ما الذي يجب عليك تعيينه هو الإعدادات الافتراضية.

## خيارات SDK على التوازي

- [Rust SDK Quickstart](../sdks/rust) — عرض تعليمات التسجيل،
  إجراء المعاملات وحالة الاقتراع من الصدأ.
- [Python SDK Quickstart](../sdks/python) - يعرض عمليات التسجيل/النعناع
  مع مساعدي JSON المدعومين من Norito.
- [JavaScript SDK Quickstart](../sdks/javascript) — قم بإنهاء Torii مرة أخرى،
  مساعدو الحوكمة وأغلفة الاستعلام المكتوبة.

قم بالبدء في الإرشادات التفصيلية في CLI، ثم قم باستعراض السيناريو باستخدام SDK المقترح،
من أجل التأكد من أن الصلابة ترتكز على المعاملات الخاصة والتوازن وال
النتيجة النهائية.