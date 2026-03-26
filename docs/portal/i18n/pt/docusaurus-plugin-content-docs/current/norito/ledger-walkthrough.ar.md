---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: جولة في السجل
description: اعادة انتاج تدفق حتمي registrar -> mint -> transferir باستخدام CLI `iroha` والتحقق من حالة السجل الناتجة.
slug: /norito/ledger-passo a passo
---

تكمل هذه الجولة [Norito quickstart](./quickstart.md) عبر توضيح كيفية تعديل حالة السجل وفحصها Use CLI `iroha`. Faça o download do cartão de crédito e do cartão de crédito no final do mês. Faça isso com cuidado e segurança. Você pode usar os guias de início rápido para Rust/Python/JavaScript usando o CLI e SDK.

## المتطلبات المسبقة

- اتبع [quickstart](./quickstart.md) لتشغيل شبكة بعقدة واحدة عبر
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Você pode usar `iroha` (CLI) e usar o peer `defaults/client.toml`.
- A configuração é: `jq` (JSON) e POSIX para enviar mensagens de texto no site.

Para obter mais informações, verifique `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT`. يتضمن الـ bundle الافتراضي بالفعل حسابين مشتقين من مفاتيح العرض:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

اكد القيم عبر سرد اولى الحسابات:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة gênese

Clique em CLI:

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

A solução de problemas de hardware é Norito, mas não há nenhum problema com isso. Você pode usar SDKs.

## 2. تسجيل تعريف اصل

Para obter mais informações sobre o `coffee`, use o `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Use o hash CLI do arquivo (como `0x5f…`). Você pode fazer isso sem parar.

## 3. سك وحدات في حساب المشغل

Verifique o valor do arquivo `(asset definition, account)`. 250 unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Crie um hash de hash (`$MINT_HASH`) na CLI. Para obter mais informações:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

E o que fazer:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. نقل جزء من الرصيد الى حساب اخر

Mais de 50 itens do site `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Use hash como `$TRANSFER_HASH`. استعلم عن الممتلكات no site do site:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. التحقق من ادلة السجل

Para obter mais informações sobre o assunto, consulte:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

يمكنك ايضا بث الكتل الحديثة لمعرفة اي كتلة تضمنت التحويل:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Você pode usar payloads em Norito para SDKs. Para obter mais informações sobre os guias de início rápido do SDK, consulte o guia de início rápido do SDK. Limpe o local e o local de trabalho.

## SDK do SDK

- [Guia de início rápido do Rust SDK](../sdks/rust) — Você pode instalar o Rust SDK no Rust.
- [Início rápido do SDK do Python](../sdks/python) — Você pode usar o registro/mint como um arquivo JSON para Norito.
- [JavaScript SDK quickstart](../sdks/javascript) — يغطي طلبات Torii, ومساعدات الحوكمة, واغلفة الاستعلامات Typed.

A CLI não está disponível para download no SDK do site para que você possa usar o SDK no seu computador. المعاملات والارصدة ومخرجات الاستعلام.