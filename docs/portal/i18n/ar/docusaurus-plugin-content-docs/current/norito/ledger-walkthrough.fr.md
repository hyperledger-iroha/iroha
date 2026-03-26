---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Parcours du registre
الوصف: إعادة إنتاج سجل محدد التدفق -> النعناع -> النقل باستخدام CLI `iroha` والتحقق من حالة دفتر الأستاذ الناتج.
سبيكة: /norito/ledger-walkthrough
---

تكتمل هذه الخطوات بـ [quickstart Norito](./quickstart.md) ويتم تعديل التعليق والتحقق من حالة دفتر الأستاذ مع CLI `iroha`. Vous enregistrez une nouvelle Definition of Actif, Minterez des Units sur le compteoperator par default, Transferer une Partie du Solde to an autre compte and verifierez les Transactions and avoirs result. كل مرة تعكس التدفق الذي يغطي Quickstarts SDK Rust/Python/JavaScript لتأكيد التكافؤ بين CLI وسلوك SDK.

## المتطلبات الأساسية

- قم بمتابعة [بدء التشغيل السريع](./quickstart.md) لبدء إعادة الاتصال الأحادي عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- تأكد من أن `iroha` (le CLI) تم إنشاؤه أو تنزيله وأنه يمكنك
  انضم إلى النظير مع `defaults/client.toml`.
- خيارات الأدوات: `jq` (تنسيق ردود JSON) وقشرة POSIX للملفات
  مقتطفات من متغيرات البيئة ci-dessous.

كل هذا دليل طويل، استبدل `$ADMIN_ACCOUNT` و`$RECEIVER_ACCOUNT` بالاسم
معرفات الحساب التي تحسب استخدامها. تتضمن الحزمة الافتراضية حسابين
إصدار العناصر التجريبية :

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

قم بتأكيد القيم في قائمة الحسابات الأولى:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. مفتش نشأة الدولة

ابدأ باستخدام مستكشف دفتر الأستاذ cible بواسطة CLI:

```sh
# Domains enregistres en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dans wonderland (remplacez --limit par un nombre plus eleve si besoin)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions qui existent deja
iroha --config defaults/client.toml asset definition list all --table
```

يتم إرسال الأوامر إلى الردود Norito، دون التصفية والصفحة
 المحددات والمراسلات التي تتلقاها SDK.

## 2. قم بتسجيل تعريف النشاط

قم بإنشاء استدعاء نشط جديد لا نهائي لـ `coffee` في المجال
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

يعرض CLI تجزئة المعاملة الواردة (على سبيل المثال، `0x5f...`). كونسيرفيز لو
صب المستشار لو ستاتوت بلس تارد.

## 3.Minter des Units sur le compteoperator

الكميات النشطة تعيش داخل الزوج `(asset definition, account)`. مينتز 250
يوحد `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` في `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

مرة أخرى، يمكنك استرداد تجزئة المعاملة (`$MINT_HASH`) من خلال واجهة CLI. صب
التحقق من اللحام، تنفيذ :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler seulement le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. نقل جزء من اللحام إلى حساب آخر

قم بإزاحة 50 وحدة من مشغل الحساب إلى `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

قم بحفظ تجزئة المعاملة مثل `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
من أجل التحقق من المبيعات الجديدة :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من إجراءات دفتر الأستاذ

استخدم التجزئات الاحتياطية لتأكيد أن المعاملات الثنائية موجودة أمامك:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```يمكنك أيضًا بث الكتل الأخيرة لرؤية أي كتلة تتضمن النقل:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

جميع الأوامر ci-dessus تستخدم حمولات الميمات Norito من SDK. نعم
إعادة إنتاج التدفق عبر كود du (البحث عن Quickstarts SDK ci-dessous)، والتجزئات وغيرها
ستعمل Soldes على محاذاة كما تريد شبكة الميمات والميمات الافتراضية.

## امتيازات التكافؤ SDK

- [Rust SDK Quickstart](../sdks/rust) - قم بتسجيل التعليمات،
  تسليم المعاملات واستطلاع الوضع من الصدأ.
- [Python SDK Quickstart](../sdks/python) - تسجيل عمليات تسجيل الميمات/النعناع
  avec des helpers JSON يعلن عن Norito.
- [JavaScript SDK Quickstart](../sdks/javascript) - تغطية الطلبات Torii،
  مساعدو الإدارة ومغلفات الطلبات من النوع.

قم بتنفيذ الإرشادات التفصيلية لـ CLI، ثم قم بتكرار السيناريو مع SDK الخاص بك
تفضل للتأكد من أن الأسطح الثنائية تتوافق مع تجزئات
المعاملات والمبيعات ونتائج الطلبات.