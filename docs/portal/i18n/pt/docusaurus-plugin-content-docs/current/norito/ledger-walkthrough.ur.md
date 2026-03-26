---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: لیجر واک تھرو
description: `iroha` CLI کے ساتھ registro determinístico -> mint -> transferência تصدیق کریں۔
slug: /norito/ledger-passo a passo
---

Passo a passo [Norito quickstart](./quickstart.md) کی تکمیل کرتا ہے اور دکھاتا ہے کہ `iroha` CLI کے ساتھ لیجر اسٹیٹ کو کیسے بدلیں اور چیک کریں۔ آپ ایک نئی definição de ativo رجسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ میں unidades mint کریں گے، بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں گے، اور نتیجے میں آنے والی transações e participações کی تصدیق کریں گے۔ ہر قدم Rust/Python/JavaScript SDK quickstarts میں کورڈ فلو کی عکاسی کرتا ہے تاکہ آپ CLI اور SDK کے درمیان paridade کی تصدیق کر سکیں۔

## پیشگی تقاضے

- [início rápido](./quickstart.md) فالو کریں تاکہ سنگل-پیئر نیٹ ورک کو
  `docker compose -f defaults/docker-compose.single.yml up --build` کے ذریعے بوٹ کیا جا سکے۔
- یقینی بنائیں کہ `iroha` (CLI) build یا download ہے اور آپ `defaults/client.toml` سے peer تک پہنچ سکتے ہیں۔
- اختیاری مددگار: `jq` (respostas JSON کی formatação) اور shell POSIX تاکہ نیچے دیے گئے snippets de variáveis de ambiente استعمال ہو سکیں۔

اس گائیڈ میں `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو ان IDs de conta سے بدلیں جو آپ استعمال کرنا چاہتے ہیں۔ pacote de padrões پہلے ہی chaves de demonstração سے اخذ کیے گئے دو contas شامل کرتا ہے:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

پہلے چند contas لسٹ کر کے ویلیوز کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Gênesis اسٹیٹ کا معائنہ

CLI é o que você precisa para fazer o download do seu cartão:

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

یہ کمانڈز Respostas apoiadas por Norito پر انحصار کرتی ہیں, لہذا filtragem e paginação determinística ہیں اور وہی ہیں جو SDKs حاصل کرتے ہیں۔

## 2. definição de ativo رجسٹر کریں

`wonderland` ڈومین کے اندر `coffee` نام کا ایک نیا, لامحدود mintable assets بنائیں:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Hash de transação enviado pela CLI (مثلاً `0x5f…`) پرنٹ کرتا ہے۔ اسے محفوظ کریں تاکہ بعد میں status کو consulta کیا جا سکے۔

## 3. آپریٹر اکاؤنٹ میں unidades de hortelã کریں

quantidades de ativos `(asset definition, account)` کے جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` میں `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` کی 250 unidades hortelã کریں:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Saída CLI سے hash de transação (`$MINT_HASH`) محفوظ کریں۔ بیلنس دوبارہ چیک کرنے کے لئے چلائیں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

Qual é o ativo que você possui:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. O que você precisa saber sobre o seu negócio

O produto é `$RECEIVER_ACCOUNT` com 50 unidades de tamanho:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

hash de transação کو `$TRANSFER_HASH` کے طور پر محفوظ کریں۔ دونوں اکاؤنٹس پر consulta de participações کریں تاکہ نئی saldos کی تصدیق ہو:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر ایویڈنس کی تصدیق

محفوظ hashes استعمال کریں تاکہ دونوں transações کے commit ہونے کی تصدیق ہو:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ blocos بھی fluxo کر سکتے ہیں تاکہ دیکھا جا سکے کہ transferência کس bloco میں شامل ہوا:

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

اوپر کی ہر کمانڈ وہی Cargas úteis Norito استعمال کرتی ہے جو SDKs استعمال کرتے ہیں۔ Você pode usar o SDK (iniciais rápidos do SDK دیکھیں) تو hashes اور saldos اس وقت تک alinhar رہیں گے جب تک آپ اسی نیٹ ورک اور padrões کو ٹارگٹ کرتے ہیں۔

## paridade do SDK- [Início rápido do Rust SDK](../sdks/rust) — Rust سے instruções رجسٹر کرنا, envio de transações کرنا, اور pesquisa de status کرنا دکھاتا ہے۔
- [Início rápido do SDK do Python](../sdks/python) — Ajudantes JSON apoiados por Norito کے ساتھ وہی operações de registro/mint دکھاتا ہے۔
- [Início rápido do SDK JavaScript](../sdks/javascript) — Solicitações Torii, auxiliares de governança, e wrappers de consulta digitados کو کور کرتا ہے۔

پہلے CLI walkthrough چلائیں, پھر اپنے پسندیدہ SDK کے ساتھ وہی منظرنامہ دہرائیں تاکہ دونوں سطحیں hashes de transação, saldos, e saídas de consulta پر متفق ہوں۔