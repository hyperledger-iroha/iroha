---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Norito کوئک اسٹارٹ
description: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
slug: /norito/quickstart
---

یہ walkthrough اس ورک فلو کی عکاسی کرتا ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز پہلی بار Norito اور Kotodama سیکھتے وقت فالو کریں: ایک ڈیٹرمنسٹک سنگل-پیئر نیٹ ورک بوٹ کریں، کنٹریکٹ کمپائل کریں، اسے مقامی طور پر dry-run کریں، پھر ریفرنس CLI کے ساتھ Torii کے ذریعے بھیجیں۔

مثالی کنٹریکٹ کالر کے اکاؤنٹ پر key/value جوڑا لکھتا ہے تاکہ آپ `iroha_cli` کے ساتھ فوراً side effect کی توثیق کر سکیں۔

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) جس میں Compose V2 فعال ہو (اسے `defaults/docker-compose.single.yml` میں متعین sample peer شروع کرنے کے لئے استعمال کیا جاتا ہے).
- Rust toolchain (1.76+) تاکہ helper binaries بنائی جا سکیں اگر آپ شائع شدہ binaries ڈاؤن لوڈ نہیں کرتے۔
- `koto_compile`, `ivm_run`, اور `iroha_cli` binaries۔ آپ انہیں workspace checkout سے نیچے دکھائے گئے طریقے کے مطابق بنا سکتے ہیں یا matching release artifacts ڈاؤن لوڈ کر سکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> اوپر والے binaries کو باقی workspace کے ساتھ انسٹال کرنا محفوظ ہے۔
> یہ کبھی `serde`/`serde_json` سے لنک نہیں کرتے؛ Norito codecs end-to-end نافذ ہوتے ہیں۔

## 1. سنگل-پیئر dev نیٹ ورک شروع کریں

ریپو میں `kagami swarm` سے جنریٹ کیا گیا Docker Compose bundle شامل ہے (`defaults/docker-compose.single.yml`). یہ ڈیفالٹ genesis، client configuration، اور health probes کو جوڑتا ہے تاکہ Torii `http://127.0.0.1:8080` پر قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (foreground میں یا detached). بعد کے تمام CLI کالز `defaults/client.toml` کے ذریعے اسی peer کو ہدف بناتی ہیں۔

## 2. کنٹریکٹ لکھیں

ایک ورکنگ ڈائریکٹری بنائیں اور minimal Kotodama مثال محفوظ کریں:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Kotodama سورسز کو version control میں رکھنا بہتر ہے۔ پورٹل پر hosted مثالیں [Norito examples gallery](./examples/) میں بھی دستیاب ہیں اگر آپ زیادہ بھرپور نقطہ آغاز چاہتے ہیں۔

## 3. IVM کے ساتھ کمپائل اور dry-run کریں

کنٹریکٹ کو IVM/Norito bytecode (`.to`) میں کمپائل کریں اور اسے مقامی طور پر چلائیں تاکہ نیٹ ورک کو چھونے سے پہلے host syscalls کی کامیابی کی تصدیق ہو:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner `info("Hello from Kotodama")` لاگ پرنٹ کرتا ہے اور mocked host کے خلاف `SET_ACCOUNT_DETAIL` syscall انجام دیتا ہے۔ اگر اختیاری `ivm_tool` binary دستیاب ہو تو `ivm_tool inspect target/quickstart/hello.to` ABI header، feature bits اور exported entrypoints دکھاتا ہے۔

## 4. Torii کے ذریعے bytecode بھیجیں

جب نوڈ ابھی چل رہا ہو، کمپائل شدہ bytecode کو CLI کے ذریعے Torii پر بھیجیں۔ ڈیفالٹ development identity `defaults/client.toml` میں موجود public key سے اخذ ہوتی ہے، اس لئے account ID یہ ہے:
```
ih58...
```

Torii URL، chain ID اور signing key فراہم کرنے کے لئے config فائل استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI Norito کے ساتھ ٹرانزیکشن کو encode کرتا ہے، اسے dev key سے sign کرتا ہے اور چلتے ہوئے peer کو submit کرتا ہے۔ `set_account_detail` syscall کے لئے Docker logs دیکھیں یا committed transaction hash کے لئے CLI output کو مانیٹر کریں۔

## 5. state change کی توثیق کریں

اسی CLI پروفائل کے ساتھ وہ account detail لائیں جو کنٹریکٹ نے لکھا تھا:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

آپ کو Norito-backed JSON payload نظر آنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر ویلیو موجود نہ ہو تو تصدیق کریں کہ Docker compose سروس ابھی بھی چل رہی ہے اور `iroha` کے رپورٹ کردہ ٹرانزیکشن ہیش نے `Committed` حالت حاصل کر لی ہے۔

## اگلے مراحل

- خودکار طور پر تیار کی گئی [example gallery](./examples/) دیکھیں تاکہ
  زیادہ advanced Kotodama snippets کو Norito syscalls سے میپ ہوتے ہوئے سمجھ سکیں۔
- مزید گہرائی کے لئے [Norito getting started guide](./getting-started) پڑھیں جس میں
  compiler/runner tooling، manifest deployment اور IVM metadata کی وضاحت ہے۔
- اپنے کنٹریکٹس پر iteration کے دوران workspace میں `npm run sync-norito-snippets` چلائیں تاکہ
  downloadable snippets دوبارہ بن سکیں اور پورٹل docs اور artifacts، `crates/ivm/docs/examples/` کی sources کے ساتھ synced رہیں۔
