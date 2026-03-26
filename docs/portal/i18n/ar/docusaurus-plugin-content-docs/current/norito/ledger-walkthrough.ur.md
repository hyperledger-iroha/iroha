---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: ليجر واک تھرو
الوصف: `iroha` CLI سجل الحتمية -> النعناع -> نقل البيانات الجديدة ونتائج البحث.
سبيكة: /norito/ledger-walkthrough
---

هذه الإرشادات التفصيلية [Norito Quickstart](./quickstart.md) تكمل القائمة وتوضح `iroha` CLI وهي متوافقة مع هذه القائمة تشيك كري. يوجد هنا سجل جديد لتعريف الأصول، وهو عبارة عن وحدات نقدية فريدة من نوعها تعمل على سك القروض، وهو ما يعد شيئًا آخر يساعد في تحويل الأموال وتسويقها. يتم إجراء هذه المعاملات والممتلكات التي يتم تداولها. توفر ميزة التشغيل السريع لـ Rust/Python/JavaScript SDK ميزة جديدة تتمثل في استخدام CLI وSDK لتكافؤ تصوّر البرامج.

## پیشگی تتطلبے

- [البدء السريع](./quickstart.md) لا يوجد عمل جديد لبطاقة العمل
  `docker compose -f defaults/docker-compose.single.yml up --build` تم إصداره مرة أخرى.
- تم إنشاء أو تنزيل الإصدار `iroha` (CLI) وإصدار `defaults/client.toml` من نظير إلى آخر.
- اختر العناصر: `jq` (تنسيق استجابات JSON) وPOSIX Shell لا يسمح باستخدام مقتطفات متغيرة البيئة.

هذا هو `$ADMIN_ACCOUNT` و`$RECEIVER_ACCOUNT` الذي يمكن استخدام معرفات الحساب فيه. حزمة الإعدادات الافتراضية تحتوي على مفاتيح تجريبية وتأخذ في الاعتبار حساباتك التي تتضمن العناصر التالية:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

بعض الحسابات القليلة لتصفح مقاطع الفيديو:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. التكوين هو معاكسCLI هو برنامج لألعاب الفيديو وهو اكسبلورس:

```sh
# genesis میں رجسٹرڈ domains
iroha --config defaults/client.toml domain list all --table

# wonderland کے اندر accounts (ضرورت ہو تو --limit بڑھائیں)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# وہ asset definitions جو پہلے سے موجود ہیں
iroha --config defaults/client.toml asset definition list all --table
```

لقد تم تمكين الاستجابات المدعومة من Norito للمقاطعة، من أجل تحقيق هذه التصفية وتحديد الصفحات وإمكانية الحصول على SDKs.

## 2. تعريف الأصول مسجل

`wonderland` ڈومین کے اندر `coffee` نام کا ایك نيا، لا حدود للأصول القابلة للتعدين:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

قدمت CLI تجزئة المعاملة (مثلاً `0x5f…`) هذا أمر مهم بعد الاستعلام عن حالة أي شخص في الجامعة.

## 3. وحدات الورق بالنعناع

تم تحديد كميات الأصول `(asset definition, account)`. `$ADMIN_ACCOUNT` يحتوي على 250 وحدة بالنعناع `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

إخراج CLI هو تجزئة المعاملة (`$MINT_HASH`) محفوظات کریں۔ بيلنس دوبلينز يكرر شيئًا جديدًا:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

أو قم بتوزيع أصول جديدة على إجمالي الأصول:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. الفواتير هي شيء جديد لتنقل الأموال

تنتقل الكاميرا إلى `$RECEIVER_ACCOUNT` مع 50 وحدة:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

تجزئة المعاملة `$TRANSFER_HASH` في حالة الحفظ. قم بتقديم معلومات عن الاستعلام عن المقتنيات لمعرفة الأرصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. سجل البيانات

يتم استخدام التجزئات المحفوظة في تنفيذ المعاملات التي يتم تنفيذها:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

تشمل الكتل التي يتم دفقها أيضًا على شريط فيديو آخر نقل كتلة الكتلة ما يلي:

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```تم استخدام الحمولات النافعة السابقة وNorito واستخدام بطاقات SDK. إذا كنت قد أحدثت بعض التغيير (المزيد من عمليات التشغيل السريع لـ SDK)، فستتمكن من محاذاة التجزئات والأرصدة في الوقت المناسب مع هذه المهام الجديدة والإعدادات الافتراضية.

## تكافؤ SDK لنکس

- [Rust SDK Quickstart](../sdks/rust) — تعليمات Rust سے رجسٹر کرنا، إرسال المعاملات کرنا، واستطلاع الحالة کرنا دکھاتا ہے۔
- [Python SDK Quickstart](../sdks/python) — مساعدات JSON المدعومة من Norito تعمل على تسجيل/عمليات النعناع.
- [JavaScript SDK Quickstart](../sdks/javascript) — طلبات Torii، ومساعدي الإدارة، وأغلفة الاستعلام المكتوبة کو کور کرتا ہے۔

في كل مرة يتم فيها تجول CLI، يتم إنشاء SDK الذي تم تفقّده ومخطط المشهد دون تجزئات المعاملات والأرصدة ومخرجات الاستعلام بشكل كامل.