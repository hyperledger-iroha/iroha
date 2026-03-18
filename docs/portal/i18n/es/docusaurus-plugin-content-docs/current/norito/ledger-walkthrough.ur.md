---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: لیجر واک تھرو
descripción: `iroha` CLI کے ساتھ registro determinista -> mint -> transferir فلو دوبارہ بنائیں اور نتیجے میں لیجر اسٹیٹ کی تصدیق کریں۔
babosa: /norito/ledger-walkthrough
---

یہ tutorial [Norito inicio rápido](./quickstart.md) کی تکمیل کرتا ہے اور دکھاتا ہے کہ `iroha` CLI کے ساتھ لیجر اسٹیٹ کو کیسے بدلیں اور چیک کریں۔ آپ ایک نئی definición de activo رجسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ میں unidades mint کریں گے، بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں گے، اور نتیجے میں آنے والی transacciones اور tenencias کی تصدیق کریں گے۔ Inicio rápido del SDK de Rust/Python/JavaScript Configuración del SDK de CLI y paridad del SDK کر سکیں۔

## پیشگی تقاضے

- [inicio rápido](./quickstart.md) فالو کریں تاکہ سنگل-پیئر نیٹ ورک کو
  `docker compose -f defaults/docker-compose.single.yml up --build` کے ذریعے بوٹ کیا جا سکے۔
- یقینی بنائیں کہ `iroha` (CLI) build یا descargar ہے اور آپ `defaults/client.toml` سے peer تک پہنچ سکتے ہیں۔
- Formato: `jq` (respuestas JSON y formato) y shell POSIX para crear fragmentos de variables de entorno.

اس گائیڈ میں `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو ان ID de cuenta سے بدلیں جو آپ استعمال کرنا چاہتے ہیں۔ paquete predeterminado پہلے ہی claves de demostración سے اخذ کیے گئے دو cuentas شامل کرتا ہے:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

پہلے چند cuentas لسٹ کر کے ویلیوز کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. génesis اسٹیٹ کا معائنہCLI جس لیجر کو ٹارگٹ کر رہا ہے اسے ایکسپلور کریں:

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

یہ کمانڈز Norito respuestas respaldadas por پر انحصار کرتی ہیں، لہذا filtrado اور paginación determinista ہیں اور وہی ہیں جو SDK حاصل کرتے ہیں۔

## 2. definición de activo رجسٹر کریں

`wonderland` Nombre del activo `coffee` Nombre del activo mintable:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

Hash de transacción enviada por CLI (مثلاً `0x5f…`) پرنٹ کرتا ہے۔ اسے محفوظ کریں تاکہ بعد میں estado کو consulta کیا جا سکے۔

## 3. آپریٹر اکاؤنٹ میں unidades nuevas کریں

cantidades de activos `(asset definition, account)` کے جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` Mیں `coffee#wonderland` کی 250 unidades nuevas کریں:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Salida CLI سے hash de transacción (`$MINT_HASH`) محفوظ کریں۔ بیلنس دوبارہ چیک کرنے کے لئے چلائیں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا صرف نئے activo کو ٹارگٹ کرنے کے لئے:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں

آپریٹر اکاؤنٹ سے `$RECEIVER_ACCOUNT` کو 50 unidades de کریں:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

hash de transacción کو `$TRANSFER_HASH` کے طور پر محفوظ کریں۔ دونوں اکاؤنٹس پر consulta de tenencias کریں تاکہ نئی saldos کی تصدیق ہو:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر ایویڈنس کی تصدیق

محفوظ hashes استعمال کریں تاکہ دونوں transacciones کے commit ہونے کی تصدیق ہو:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ bloquea بھی flujo کر سکتے ہیں تاکہ دیکھا جا سکے کہ transferencia کس bloque میں شامل ہوا:

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```اوپر کی ہر کمانڈ وہی Norito cargas útiles استعمال کرتی ہے جو SDK استعمال کرتے ہیں۔ اگر آپ اس فلو کو کوڈ کے ذریعے دہرائیں (نیچے SDK Quickstarts دیکھیں), تو hashes اور balances اس وقت تک align رہیں گے جب تک آپ اسی نیٹ ورک اور defaults کو ٹارگٹ کرتے ہیں۔

## Paridad del SDK

- [Inicio rápido de Rust SDK] (../sdks/rust) — Instrucciones de Rust para enviar transacciones, enviar una encuesta de estado, para enviar una encuesta de estado
- [Inicio rápido del SDK de Python](../sdks/python) — Ayudantes JSON respaldados por Norito کے ساتھ وہی operaciones de registro/mint دکھاتا ہے۔
- [Inicio rápido del SDK de JavaScript](../sdks/javascript) — Solicitudes Torii, ayudantes de gobernanza, envoltorios de consultas escritas کو کور کرتا ہے۔

Tutorial CLI hashes de transacciones, saldos, resultados de consultas پر متفق ہوں۔