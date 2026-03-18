---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Norito کوئک اسٹارٹ
description: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
slug: /norito/início rápido
---

یہ passo a passo Norito اور Kotodama سیکھتے وقت فالو کریں: ایک ڈیٹرمنسٹک سنگل-پیئر نیٹ ورک بوٹ کریں, کنٹریکٹ کمپائل کریں, اسے مقامی طور پر کریں, پھر ریفرنس CLI کے ساتھ Torii کے ذریعے بھیجیں۔

مثالی کنٹریکٹ کالر کے اکاؤنٹ پر chave/valor جوڑا لکھتا ہے تاکہ آپ `iroha_cli` کے ساتھ فوراً efeito colateral

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) جس میں Compose V2 فعال ہو (اسے `defaults/docker-compose.single.yml` میں متعین sample peer شروع کرنے کے لئے استعمال کیا جاتا ہے).
- Cadeia de ferramentas Rust (1.76+) تاکہ binários auxiliares
- `koto_compile`, `ivm_run`, e `iroha_cli` binários۔ آپ انہیں checkout do espaço de trabalho سے نیچے دکھائے گئے طریقے کے مطابق بنا سکتے ہیں یا artefatos de lançamento correspondentes ڈاؤن لوڈ O que fazer:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> اوپر والے binários کو باقی espaço de trabalho کے ساتھ انسٹال کرنا محفوظ ہے۔
> یہ کبھی `serde`/`serde_json` سے لنک نہیں کرتے؛ Codecs Norito de ponta a ponta

## 1. سنگل-پیئر dev نیٹ ورک شروع کریں

ریپو میں `kagami swarm` سے جنریٹ کیا گیا Docker Compose bundle شامل ہے (`defaults/docker-compose.single.yml`). یہ ڈیفالٹ genesis, configuração do cliente, e sondas de saúde کو جوڑتا ہے تاکہ Torii `http://127.0.0.1:8080` پر قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (primeiro plano میں یا desanexado). بعد کے تمام CLI کالز `defaults/client.toml` کے ذریعے اسی peer کو ہدف بناتی ہیں۔

## 2. کنٹریکٹ لکھیں

O valor mínimo de Kotodama é o valor mínimo de Kotodama:

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

> Kotodama سورسز کو controle de versão میں رکھنا بہتر ہے۔ پورٹل پر hospedado مثالیں [Galeria de exemplos Norito](./examples/) میں بھی دستیاب ہیں اگر آپ زیادہ بھرپور نقطہ آغاز چاہتے ہیں۔

## 3. IVM کے ساتھ کمپائل اور teste de simulação

کنٹریکٹ کو IVM/Norito bytecode (`.to`) میں کمپائل کریں اور اسے مقامی طور پر چلائیں تاکہ نیٹ ورک کو چھونے سے پہلے host syscalls کی کامیابی کی تصدیق ہو:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O Runner `info("Hello from Kotodama")` é um host zombado e um host zombado `SET_ACCOUNT_DETAIL` syscall é um host zombado اگر اختیاری `ivm_tool` binário دستیاب ہو تو `ivm_tool inspect target/quickstart/hello.to` cabeçalho ABI, bits de recursos e pontos de entrada exportados دکھاتا ہے۔

## 4. Torii کے ذریعے bytecode بھیجیں

Não há nenhum código de byte para CLI que seja Torii. ڈیفالٹ identidade de desenvolvimento `defaults/client.toml` موجود chave pública سے اخذ ہوتی ہے، اس لئے ID da conta یہ ہے:
```
i105...
```

URL Torii, ID da cadeia e chave de assinatura فراہم کرنے کے لئے config فائل استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI Norito کے ساتھ ٹرانزیکشن کو codificar کرتا ہے, اسے chave dev سے assinar کرتا ہے اور چلتے ہوئے peer کو enviar کرتا ہے۔ `set_account_detail` syscall کے لئے Docker logs دیکھیں یا hash de transação confirmada کے لئے saída CLI کو مانیٹر کریں۔

## 5. mudança de estado

اسی CLI پروفائل کے ساتھ وہ detalhes da conta لائیں جو کنٹریکٹ نے لکھا تھا:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

A carga útil JSON suportada por Norito é a seguinte:

```json
{
  "hello": "world"
}
```

اگر ویلیو موجود نہ ہو تو تصدیق کریں کہ Docker compose سروس ابھی بھی چل رہی ہے اور `iroha` کے رپورٹ کردہ ٹرانزیکشن ہیش نے `Committed` حالت حاصل کر لی ہے۔

## اگلے مراحل

- خودکار طور پر تیار کی گئی [galeria de exemplo](./examples/) دیکھیں تاکہ
  Obtenha snippets Kotodama avançados e syscalls Norito.
- مزید گہرائی کے لئے [Norito guia de primeiros passos](./getting-started) پڑھیں جس میں
  ferramentas de compilador/executor, implantação de manifesto e metadados IVM کی وضاحت ہے۔
- اپنے کنٹریکٹس پر iteração کے دوران espaço de trabalho میں `npm run sync-norito-snippets` چلائیں تاکہ
  snippets para download دوبارہ بن سکیں اور پورٹل docs اور artefatos, `crates/ivm/docs/examples/` کی fontes کے ساتھ sincronizadas رہیں۔