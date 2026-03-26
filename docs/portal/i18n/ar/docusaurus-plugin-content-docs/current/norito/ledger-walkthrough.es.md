---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Recorrido del libro mayor
الوصف: إعادة إنتاج تدفق محدد من التسجيل -> النعناع -> النقل باستخدام CLI `iroha` والتحقق من الحالة الناتجة عن دفتر الأستاذ.
سبيكة: /norito/ledger-walkthrough
---

يتم تسجيل هذا بشكل مكمل لـ [البدء السريع في Norito](./quickstart.md) كما يتم تغييره وفحص حالة دفتر الأستاذ باستخدام CLI `iroha`. قم بالتسجيل بتعريف جديد للنشاط، وجمع الوحدات في حساب المشغل المعيب، وتحويل جزء من الرصيد إلى حساب آخر، والتحقق من المعاملات والاستثمارات الناتجة. تعكس كل خطوة التدفقات الموجودة في Quickstarts لـ SDK لـ Rust/Python/JavaScript حتى تتمكن من تأكيد التكافؤ بين CLI وSDK.

## المتطلبات السابقة

- اضغط على [quickstart](./quickstart.md) لبدء تشغيل نظير واحد فقط عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- تأكد من أن `iroha` (el CLI) تم تجميعه أو تنزيله ويمكنه
  الكنزار النظير المستخدم `defaults/client.toml`.
- المساعدون الاختياريون: `jq` (تنسيق الاستجابة JSON) وقشرة POSIX لـ
  مقتطفات المتغيرات المستخدمة أدناه.

لفترة طويلة من الأداة، قم باستبدال `$ADMIN_ACCOUNT` و`$RECEIVER_ACCOUNT` معهما
معرفات الحساب التي تستخدمها. تتضمن الحزمة المعيبة عدتين من الحسابات
Derivadas de las claves العرض التوضيحي:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

تأكيد القيم المدرجة في الحسابات الأولى:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. فحص نشأة الحالة

قم باستكشاف دفتر الأستاذ الذي يمثل CLI:

```sh
# Domains registrados en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (reemplaza --limit por un numero mayor si hace falta)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ya existen
iroha --config defaults/client.toml asset definition list all --table
```

تعتمد هذه الأوامر على الردود التي تم الرد عليها لـ Norito، لأن التصفية والصفحة محددتان ومتزامنتان مع تلقي SDK.

## 2. قم بتسجيل تعريف النشاط

أنشئ نشاطًا جديدًا لا نهائيًا للاتصال `coffee` داخل نطاق السيطرة
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

يقوم CLI بطباعة تجزئة المعاملة المرسلة (على سبيل المثال، `0x5f...`). Guardalo para Consulting el estado mas tarde.

## 3. وحدة واحدة في حساب المشغل

يتم تشغيل عدد كبير من الأنشطة على قدم المساواة مع `(asset definition, account)`. أكونا
250 وحدة من `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` و`$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

من جديد، قم بالتقاط تجزئة المعاملة (`$MINT_HASH`) من إخراج CLI. الفقرة
التحقق من التوازن، التنفيذ:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

o، من أجل البدء منفردًا بالنشاط الجديد:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. تحويل جزء من الرصيد إلى حساب آخر

قم بتحريك 50 وحدة من حساب المشغل إلى `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

قم بحماية تجزئة المعاملة مثل `$TRANSFER_HASH`. استشر الممتلكات لدى السفراء
حسابات للتحقق من الأرصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من أدلة دفتر الأستاذ

يتم استخدام التجزئات المحمية لتأكيد تأكيد المعاملات التجارية:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```يمكنك أيضًا إرسال الكتل الحديثة حتى تتضمن الكتلة النقل:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

كل أمر سابق هو نفس الحمولات Norito التي هي SDK. النسخ المتماثلة سي
هذا التدفق المتوسط للكود (إصدار Quickstarts de SDK abajo)، والتجزئات والأرصدة
يتزامن دائمًا مع نفس اللون الأحمر والإعدادات الافتراضية.

## Enlaces de paridad مع SDK

- [Rust SDK Quickstart](../sdks/rust) - تعليمات التسجيل التالية،
  أرسل المعاملات واستشر حالة الصدأ.
- [Python SDK Quickstart](../sdks/python) - يستخدم نفس عمليات التسجيل/النعناع
  مع مساعدين JSON مستجيبين لـ Norito.
- [البدء السريع لـ JavaScript SDK](../sdks/javascript) - طلبات مكعبة Torii،
  مساعدو إدارة ومغلفات الاستعلامات ذات النوعية الجيدة.

قم بتنفيذ أول عملية تسجيل لـ CLI، ثم كرر السيناريو باستخدام SDK الخاص بك
يُفضل التأكد من وجود أسطح متراكمة في تجزئة المعاملات،
الأرصدة ونتائج الاستشارات.