---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: تجول في دفتر الأستاذ
الوصف: إعادة إنتاج التدفق الحتمي للتسجيل -> النعناع -> النقل عبر CLI `iroha` والتحقق من حالة دفتر الأستاذ الناتج.
سبيكة: /norito/ledger-walkthrough
---

تكمل هذه الإرشادات التفصيلية [Norito Quickstart](./quickstart.md) لعرضها كتغيير وفحص حالة دفتر الأستاذ مع CLI `iroha`. قم بتسجيل اسم جديد محدد للنشاط، وإنشاء وحدات في حساب المشغل الرئيسي، ونقل جزء من المبلغ إلى حساب آخر والتحقق من المعاملات والممتلكات الناتجة. كل خطوة تخص التدفقات الشاملة لنا Quickstarts لـ SDK Rust/Python/JavaScript لتأكيد التكافؤ بين CLI وSDK.

## المتطلبات المسبقة

- اضغط على [quickstart](./quickstart.md) لبدء تشغيل نظير واحد عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- ضمان أن `iroha` (o CLI) تم تجميعه أو حذفه وأنك ستوصي بذلك
  الوصول أو النظير باستخدام `defaults/client.toml`.
- خيارات المساعدة: `jq` (تنسيق الاستجابة JSON) وقشرة POSIX للفقرة
  هذه المقتطفات المتنوعة من البيئة المستخدمة مؤخرًا.

منذ فترة طويلة إلى الدليل، استبدل `$ADMIN_ACCOUNT` و`$RECEIVER_ACCOUNT` بمعرفات الشعر
conta que voceplaneja usar. تحتوي الحزمة على حسابين مشتقين من ذلك
شافيز دي التجريبي:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

قم بتأكيد القيم المدرجة كحسابات أولية:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة التكوينتعال لاستكشاف دفتر الأستاذ الذي يعجبك CLI:

```sh
# Domains registrados no genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (substitua --limit por um numero maior se necessario)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ja existem
iroha --config defaults/client.toml asset definition list all --table
```

تعتمد هذه الأوامر على الإجابات التي تم الرد عليها لـ Norito، بما في ذلك التصفية والصفحة وتحديد ما إذا كانت مجموعات SDK ستستقبل.

## 2. سجل تعريفًا للموضوع

اصرخ بجديد لا نهائي من السحر `coffee` في دومينيو
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

يقوم CLI بطباعة تجزئة التحويل المرسل (على سبيل المثال، `0x5f...`). Guarde-o الفقرة
مستشار أو حالة متأخرة.

## 3. احفظ الوحدات بحساب المشغل

كما تحيا كميات كبيرة من الأشياء على قدم المساواة `(asset definition, account)`. النعناع 250
وحدات `coffee#wonderland` في `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

من جديد، قم بالتقاط تجزئة التحويل (`$MINT_HASH`) من خلال CLI. الفقرة
أكد يا سالدو، ركب:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

أو، لرؤية المزيد من الأحداث:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. تحويل جزء من المبلغ إلى حساب آخر

Mova 50 وحدة حساب المشغل لـ `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

قم بحماية تجزئة التحويل مثل `$TRANSFER_HASH`. قم بمراجعة المقتنيات لدى السفراء
كمعلومات للتحقق من الصلوات الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. تحقق كما تفعل الأدلة في دفتر الأستاذ

استخدم طلقات التجزئات لتأكيد رسالتك كعمليات تحويل ملتزم بها:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

قد يؤدي هذا أيضًا إلى إنشاء تيار من الكتل الأخيرة لرؤية أي كتلة تتضمن ذلك
ترانسفيرنسيا:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```كل أمر يستخدم حمولات كل من Norito وSDKs. إنه يكرر الصوت
يتم تدفق هذا التدفق عبر الكوديجو (الحصول على عمليات التشغيل السريع لـ SDK من أعلى)، والتجزئات والأوراق المالية
vao alinhar fromde que voce mire a mesma rede e os mesmos defaults.

## روابط حزمة SDK

- [البدء السريع لـ Rust SDK] (../sdks/rust) - عرض تعليمات المسجل،
  تحويلات مقياس فرعي وحالة استشارية من الصدأ.
- [Python SDK Quickstart](../sdks/python) - يتم تسجيل/نعناع عمليات التشغيل mesmas
  com helpers JSON respaldados por Norito.
- [البدء السريع لـ JavaScript SDK](../sdks/javascript) - طلبات الكوبري Torii،
  مساعدو الإدارة ومغلفات الاستعلام ذات أنواع مختلفة.

اتبع الإرشادات التفصيلية لـ CLI الأولى، بعد تكرار السيناريو باستخدام SDK الخاص بها
تفضيل لضمان توافق سطحين مع التجزئات
المعاملات والمبيعات ومخرجات الاستعلام.