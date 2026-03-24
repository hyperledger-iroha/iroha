---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: جولة في السجل
descripción: اعادة انتاج تدفق حتمي registro -> mint -> transferencia باستخدام CLI `iroha` والتحقق من حالة السجل الناتجة.
babosa: /norito/ledger-walkthrough
---

تكمل هذه الجولة [Norito inicio rápido](./quickstart.md) عبر توضيح كيفية تعديل حالة السجل وفحصها باستخدام CLI `iroha`. ستسجل تعريف اصل جديدا، وتسك وحدات في حساب المشغل الافتراضي، وتنقل جزءا من الرصيد الى حساب اخر، وتحقق من المعاملات والممتلكات الناتجة. Utilice los inicios rápidos de Rust/Python/JavaScript para acceder a la CLI y al SDK.

## المتطلبات المسبقة

- اتبع [inicio rápido](./quickstart.md) لتشغيل شبكة بعقدة واحدة عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Utilice `iroha` (CLI) para conectar y conectar el par con `defaults/client.toml`.
- Nombre del usuario: `jq` (texto JSON) y código POSIX para el usuario.

Los dispositivos `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` están conectados a los terminales correspondientes. Este paquete incluye:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

اكد القيم عبر سرد اولى الحسابات:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة génesis

Haga clic en la CLI:

```sh
# Domains المسجلة في genesis
iroha --config defaults/client.toml domain list all --table

# Accounts داخل wonderland (استبدل --limit بعدد اكبر عند الحاجة)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions الموجودة مسبقا
iroha --config defaults/client.toml asset definition list all --table
```

تعتمد هذه الاوامر على ردود مدعومة ب Norito, لذا يكون الترشيح والتقسيم حتميين ومتطابقين مع ما Utilice los SDK.

## 2. تسجيل تعريف اصلانشئ اصلا جديدا قابلا للسك بلا حدود باسم `coffee` داخل نطاق `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Utilice el hash CLI para el archivo (modelo `0x5f…`). احفظه كي تستعلم عن الحالة لاحقا.

## 3. سك وحدات في حساب المشغل

توجد كميات الاصول تحت الزوج `(asset definition, account)`. Hay 250 puntos entre `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` y `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Utilice el hash (`$MINT_HASH`) de la CLI. للتحقق من الرصيد نفذ:

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

50 puntos de la marca `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

El hash del archivo `$TRANSFER_HASH`. استعلم عن الممتلكات في الحسابين للتحقق من الارصدة الجديدة:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من ادلة السجل

Las siguientes opciones son:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

يمكنك ايضا بث الكتل الحديثة لمعرفة اي كتلة تضمنت التحويل:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Establece cargas útiles en los SDK Norito. اذا كررت هذا التدفق عبر الكود (انظر inicios rápidos para el SDK ادناه), فستتطابق الهاشات yالارصدة ما دمت تستهدف الشبكة نفسها والافتراضات نفسها.

## روابط تكافؤ SDK- [Inicio rápido de Rust SDK] (../sdks/rust) — يوضح تسجيل التعليمات، ارسال المعاملات، واستطلاع الحالة من Rust.
- [Inicio rápido del SDK de Python] (../sdks/python) — يعرض نفس عمليات registro/mint مع مساعدات JSON مدعومة ب Norito.
- [Inicio rápido del SDK de JavaScript] (../sdks/javascript) — يغطي طلبات Torii, ومساعدات الحوكمة، واغلفة الاستعلامات المات المات.

Utilice CLI y haga clic en el SDK para acceder a los archivos adjuntos. والارصدة ومخرجات الاستعلام.