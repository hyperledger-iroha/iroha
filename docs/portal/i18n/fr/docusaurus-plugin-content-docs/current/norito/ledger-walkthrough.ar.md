---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : جولة في السجل
description: اعادة انتاج تدفق حتمي register -> mint -> transfer باستخدام CLI `iroha` والتحقق من حالة السجل الناتجة.
slug : /norito/ledger-walkthrough
---

تكمل هذه الجولة [Norito quickstart](./quickstart.md) en utilisant la CLI `iroha`. ستسجل تعريف اصل جديدا، وتسك وحدات في حساب المشغل الافتراضي، وتنقل جزءا من الرصيد الى حساب اخر، وتتحقق من المعاملات والممتلكات الناتجة. Vous trouverez plus d'informations sur les démarrages rapides de Rust/Python/JavaScript à l'aide du SDK CLI.

## المتطلبات المسبقة

- اتبع [quickstart](./quickstart.md) لتشغيل شبكة بعقدة واحدة عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Utilisez le `iroha` (CLI) et connectez-vous au peer `defaults/client.toml`.
- Fichiers de fichiers : `jq` (fichiers JSON) et POSIX pour les lecteurs de fichiers.

طوال الدليل، استبدل `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` بمعرفات الحساب التي تخطط لاستخدامها. يتضمن الـ bundle الافتراضي بالفعل حسابين مشتقين من مفاتيح العرض:

```sh
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

اكد القيم عبر سرد اولى الحسابات:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة genèse

Utilisez la CLI :

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

تعتمد هذه الاوامر على ردود مدعومة by Norito, لذا يكون الترشيح والتقسيم حتميين ومتطابقين مع ما Utilisez les SDK.

## 2. تسجيل تعريف اصلانشئ اصلا جديدا قابلا للسك بلا حدود باسم `coffee` داخل نطاق `wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Utilisez le hachage CLI pour la connexion (avec `0x5f…`). احفظه كي تستعلم عن الحالة لاحقا.

## 3. سك وحدات في حساب المشغل

Il s'agit du `(asset definition, account)`. Il y a 250 millions de dollars pour `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` pour `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Utilisez la fonction de hachage (`$MINT_HASH`) dans la CLI. للتحقق من الرصيد نفذ:

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

Il y a 50 millions de dollars dans le dossier `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Utilisez le hachage pour `$TRANSFER_HASH`. استعلم عن الممتلكات في الحسابين للتحقق من الارصدة الجديدة:

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

Vous pouvez utiliser les charges utiles pour les SDK Norito. اذا كررت هذا التدفق عبر الكود (انظر quickstarts for SDK ادناه)، فستطابق الهاشات والارصدة ما دمت تستهدف الشبكة نفسها والافتراضات نفسها.

## روابط تكافؤ SDK- [Démarrage rapide du SDK Rust](../sdks/rust) — Il s'agit d'un outil de démarrage rapide pour Rust.
- [Démarrage rapide du SDK Python](../sdks/python) — Vous pouvez utiliser Register/Mint pour utiliser JSON vers Norito.
- [Démarrage rapide du SDK JavaScript](../sdks/javascript) — Il s'agit du code Torii et des touches typées.

Vous pouvez utiliser la CLI pour créer un SDK qui vous permet de créer des liens avec les utilisateurs. والارصدة ومخرجات الاستعلام.