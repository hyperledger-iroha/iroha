---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Norito کوئک اسٹارٹ
descripción: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
babosa: /norito/inicio rápido
---

یہ tutorial اس ورک فلو کی عکاسی کرتا ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز پہلی بار Norito اور Kotodama سیکھتے وقت فالو کریں: ایک ڈیٹرمنسٹک سنگل-پیئر نیٹ ورک بوٹ کریں، کنٹریکٹ کمپائل کریں، اسے مقامی طور پر dry-run کریں، پھر ریفرنس CLI کے ساتھ Torii کے ذریعے بھیجیں۔

Efecto secundario del efecto secundario de la clave/valor del elemento clave/valor کی توثیق کر سکیں۔

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) جس میں Compose V2 فعال ہو (اسے `defaults/docker-compose.single.yml` میں متعین sample peer شروع کرنے کے لئے استعمال کیا جاتا ہے).
- Cadena de herramientas Rust (1.76+) تاکہ binarios auxiliares بنائی جا سکیں اگر آپ شائع شدہ binarios ڈاؤن لوڈ نہیں کرتے۔
- `koto_compile`, `ivm_run`, y binarios `iroha_cli`. آپ انہیں checkout del espacio de trabajo سے نیچے دکھائے گئے طریقے کے مطابق بنا سکتے ہیں یا artefactos de lanzamiento coincidentes ڈاؤن لوڈ کر سکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> اوپر والے binarios کو باقی espacio de trabajo کے ساتھ انسٹال کرنا محفوظ ہے۔
> یہ کبھی `serde`/`serde_json` سے لنک نہیں کرتے؛ Códecs Norito de extremo a extremo نافذ ہوتے ہیں۔

## 1. سنگل-پیئر dev نیٹ ورک شروع کریںریپو میں `kagami swarm` سے جنریٹ کیا گیا Docker Compone paquete شامل ہے (`defaults/docker-compose.single.yml`). یہ ڈیفالٹ genesis, configuración del cliente, اور sondas de salud کو جوڑتا ہے تاکہ Torii `http://127.0.0.1:8080` پر قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (primer plano میں یا separado). بعد کے تمام CLI کالز `defaults/client.toml` کے ذریعے اسی peer کو ہدف بناتی ہیں۔

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

> Kotodama سورسز کو control de versiones میں رکھنا بہتر ہے۔ پورٹل پر hosting مثالیں [Norito galería de ejemplos](./examples/) میں بھی دستیاب ہیں اگر آپ زیادہ بھرپور نقطہ آغاز چاہتے ہیں۔

## 3. IVM کے ساتھ کمپائل اور seco کریں

Código de bytes IVM/Norito (`.to`) چلائیں تاکہ نیٹ ورک کو چھونے سے پہلے host syscalls کی کامیابی کی تصدیق ہو:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner `info("Hello from Kotodama")` لاگ پرنٹ کرتا ہے اور burlado host کے خلاف `SET_ACCOUNT_DETAIL` syscall انجام دیتا ہے۔ Configuración binaria `ivm_tool` y encabezado ABI `ivm_tool inspect target/quickstart/hello.to`, bits de características y puntos de entrada exportados

## 4. Torii کے ذریعے código de bytes بھیجیں

جب نوڈ ابھی چل رہا ہو، کمپائل شدہ bytecode کو CLI کے ذریعے Torii پر بھیجیں۔ ڈیفالٹ identidad de desarrollo `defaults/client.toml` میں موجود clave pública سے اخذ ہوتی ہے، اس لئے ID de cuenta یہ ہے:
```
<katakana-i105-account-id>
```Torii URL, ID de cadena y clave de firma فراہم کرنے کے لئے config فائل ستعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI Norito کے ساتھ ٹرانزیکشن کو codificar کرتا ہے، اسے dev key سے sign کرتا ہے اور چلتے ہوئے peer کو enviar کرتا ہے۔ `set_account_detail` llamada al sistema کے لئے Docker registra دیکھیں یا hash de transacción comprometida کے لئے Salida CLI کو مانیٹر کریں۔

## 5. cambio de estado کی توثیق کریں

اسی CLI پروفائل کے ساتھ وہ detalle de cuenta لائیں جو کنٹریکٹ نے لکھا تھا:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

Esta es la carga útil JSON respaldada por Norito:

```json
{
  "hello": "world"
}
```

اگر ویلیو موجود نہ ہو تو تصدیق کریں کہ Docker componer سروس ابھی بھی چل رہی ہے اور `iroha` کے رپورٹ کردہ ٹرانزیکشن ہیش نے `Committed` حالت حاصل کر لی ہے۔

## اگلے مراحل

- خودکار طور پر تیار کی گئی [galería de ejemplos](./examples/) دیکھیں تاکہ
  Varios fragmentos avanzados de Kotodama y llamadas al sistema Norito
- مزید گہرائی کے لئے [Norito guía de introducción](./getting-started) پڑھیں جس میں
  herramientas de compilador/ejecudor, implementación de manifiesto y metadatos IVM کی وضاحت ہے۔
- اپنے کنٹریکٹس پر iteración کے دوران espacio de trabajo میں `npm run sync-norito-snippets` چلائیں تاکہ
  Fragmentos descargables disponibles en documentos y artefactos, `crates/ivm/docs/examples/`, fuentes sincronizadas y sincronizadas.