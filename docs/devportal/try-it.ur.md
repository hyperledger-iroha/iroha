---
lang: ur
direction: rtl
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Try It سینڈ باکس گائیڈ
summary: Torii اسٹیجنگ پراکسی اور ڈیولپر پورٹل سینڈ باکس کو چلانے کا طریقہ۔
---

ڈیولپر پورٹل، Torii REST API کے لیے “Try it” کنسول فراہم کرتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ سپورٹنگ پراکسی کو کیسے لانچ کیا جائے اور کنسول کو staging گیٹ وے کے ساتھ کیسے
جوڑا جائے، بغیر credentials کو ایکسپوز کیے۔

## پیشگی ضروریات

- Iroha ریپوزٹری کا checkout (workspace root)۔
- Node.js 18.18+ (پورٹل baseline کے مطابق)۔
- Torii endpoint، جس تک آپ کی ورک سٹیشن سے رسائی ممکن ہو (staging یا لوکل)۔

## 1. OpenAPI snapshot جنریٹ کرنا (اختیاری)

کنسول، portal reference پیجز کے ساتھ وہی OpenAPI payload reuse کرتی ہے۔ اگر آپ نے Torii
روٹس میں تبدیلی کی ہے، تو snapshot کو دوبارہ جنریٹ کریں:

```bash
cargo xtask openapi
```

یہ ٹاسک `docs/portal/static/openapi/torii.json` فائل لکھتا ہے۔

## 2. Try It پراکسی شروع کرنا

ریپوزٹری روٹ سے:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### ماحول کے متغیرات (Environment variables)

| ویری ایبل | وضاحت |
|-----------|--------|
| `TRYIT_PROXY_TARGET` | Torii base URL (لازمی)۔ |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | comma‑separated origins جنہیں پراکسی استعمال کرنے کی اجازت ہو (ڈیفالٹ `http://localhost:3000`)۔ |
| `TRYIT_PROXY_BEARER` | اختیاری bearer token جو default کے طور پر ہر proxied request پر apply ہوتا ہے۔ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | اسے `1` پر سیٹ کریں تاکہ کلائنٹ کا `Authorization` ہیڈر بلا تبدیلی forward ہو۔ |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | in‑memory rate limiter سیٹنگز (ڈیفالٹ: 60 requests فی 60 s)۔ |
| `TRYIT_PROXY_MAX_BODY` | زیادہ سے زیادہ request payload سائز (bytes، ڈیفالٹ 1 MiB)۔ |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii requests کے لیے upstream timeout (ڈیفالٹ 10 000 ms)۔ |

پراکسی درج ذیل endpoints فراہم کرتی ہے:

- `GET /healthz` — readiness چیک۔
- `/proxy/*` — proxied requests، جن میں path اور query string برقرار رہتے ہیں۔

## 3. پورٹل لانچ کرنا

دوسرے ٹرمینل میں:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` کھولیں اور Try It کنسول استعمال کریں۔ یہی environment
variables، Swagger UI اور RapiDoc embeds کو بھی کنفیگر کرتے ہیں۔

## 4. یونٹ ٹیسٹس چلانا

پراکسی، Node‑بیسڈ فاسٹ test suite فراہم کرتی ہے:

```bash
npm run test:tryit-proxy
```

یہ ٹیسٹس، address parsing، origin handling، rate limiting اور bearer injection کو کور
کرتے ہیں۔

## 5. ہیلتھ پروب اور میٹرکس کو خودکار بنانا

بِلت‑اِن probe اسکرپٹ استعمال کریں تاکہ `/healthz` اور کسی نمونہ اینڈپوائنٹ کو چیک کیا
جا سکے:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

اہم environment variables:

- `TRYIT_PROXY_SAMPLE_PATH` — اختیاری Torii روٹ (بغیر `/proxy`) جسے exercise کرنا ہو۔
- `TRYIT_PROXY_SAMPLE_METHOD` — ڈیفالٹ `GET`؛ write روٹس کے لیے `POST` پر سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` — sample کال کے لیے عارضی bearer token inject کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 5 سیکنڈ کے ڈیفالٹ ٹائم آؤٹ کو override کرتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus textfile آؤٹ پٹ جہاں `probe_success` اور `probe_duration_seconds` لکھی جاتی ہیں۔
- `TRYIT_PROXY_PROBE_LABELS` — comma-separated `key=value` جو لیبلز میں شامل ہوتے ہیں (ڈیفالٹ `job=tryit-proxy` اور `instance=<پراکسی URL>`).

`TRYIT_PROXY_PROBE_METRICS_FILE` سیٹ ہونے پر اسکرپٹ فائل کو ایٹامک انداز میں دوبارہ
لکھتا ہے، لہٰذا node_exporter/textfile collector ہمیشہ مکمل payload پڑھتا ہے۔ مثال:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

میٹرکس کو Prometheus میں فارورڈ کریں اور وہی الرٹ رول استعمال کریں جو `probe_success` کے
صفر ہوتے ہی نوٹیفائی کرے۔

## 6. پروڈکشن hardening چیک لسٹ

پراکسی کو مقامی development سے باہر publish کرنے سے پہلے:

- TLS کو پراکسی سے پہلے terminate کریں (reverse proxy یا managed gateway کے ذریعے)۔
- structured logging کنفیگر کریں اور اسے observability pipelines تک forward کریں۔
- bearer tokens کو باقاعدگی سے rotate کریں اور انہیں secrets manager میں محفوظ کریں۔
- پراکسی کے `/healthz` endpoint کو مانیٹر کریں اور latency metrics کو aggregate کریں۔
- Torii staging quotas کے ساتھ rate limits کو align کریں؛ throttling کو کلائنٹس تک
  واضح انداز میں communicate کرنے کے لیے `Retry-After` کے behaviour کو adjust کریں۔

</div>
