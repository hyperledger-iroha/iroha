---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# پورٹل آبزرویبیلٹی اور اینالٹکس

DOCS-SORA روڈمیپ ہر preview build کے لئے analytics، synthetic probes، اور broken-link automation کا تقاضا کرتا ہے۔
یہ نوٹ وہ plumbing بیان کرتا ہے جو اب پورٹل کے ساتھ ship ہوتی ہے تاکہ operators monitoring جوڑ سکیں
بغیر visitors data لیک کئے۔

## Release tagging

- `DOCS_RELEASE_TAG=<identifier>` سیٹ کریں (fallback `GIT_COMMIT` یا `dev`) جب پورٹل build ہو۔
  ویلیو `<meta name="sora-release">` میں inject ہوتی ہے تاکہ probes اور dashboards deployments کو distinguish کر سکیں۔
- `npm run build` `build/release.json` emit کرتا ہے (جسے `scripts/write-checksums.mjs` لکھتا ہے)
  جو tag، timestamp، اور optional `DOCS_RELEASE_SOURCE` کو describe کرتا ہے۔ یہی فائل preview artifacts میں bundle
  ہوتی ہے اور link checker رپورٹ میں refer ہوتی ہے۔

## Privacy-preserving analytics

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` configure کریں تاکہ lightweight tracker enable ہو۔
  payloads میں `{ event, path, locale, release, ts }` شامل ہوتے ہیں بغیر referrer یا IP metadata کے، اور
  `navigator.sendBeacon` جہاں ممکن ہو استعمال ہوتا ہے تاکہ navigation block نہ ہو۔
- `DOCS_ANALYTICS_SAMPLE_RATE` (0-1) کے ساتھ sampling control کریں۔ tracker last-sent path ذخیرہ کرتا ہے
  اور ایک ہی navigation کے لئے duplicate events emit نہیں کرتا۔
- implementation `src/components/AnalyticsTracker.jsx` میں ہے اور `src/theme/Root.js` کے ذریعے globally mount ہوتی ہے۔

## Synthetic probes

- `npm run probe:portal` عام routes کے خلاف GET requests بھیجتا ہے
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغیرہ) اور verify کرتا ہے کہ
  `sora-release` meta tag `--expect-release` (یا `DOCS_RELEASE_TAG`) سے match کرتا ہے۔ مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Failures ہر path کے حساب سے report ہوتے ہیں، جس سے CD gate کرنا آسان ہو جاتا ہے۔

## Broken-link automation

- `npm run check:links` `build/sitemap.xml` scan کرتا ہے، ہر entry کو local file سے map ہونا یقینی بناتا ہے
  (`index.html` fallbacks چیک کرتا ہے)، اور `build/link-report.json` لکھتا ہے جس میں release metadata، totals، failures،
  اور `checksums.sha256` کا SHA-256 fingerprint شامل ہوتا ہے (جو `manifest.id` کے طور پر expose ہوتا ہے) تاکہ ہر رپورٹ
  artifact manifest سے جوڑی جا سکے۔
- script non-zero پر exit کرتا ہے جب کوئی صفحہ missing ہو، اس لئے CI پرانی یا broken routes پر releases روک سکتا ہے۔
  reports ان candidate paths کو cite کرتے ہیں جو try کئے گئے تھے، جو routing regressions کو docs tree تک trace کرنے میں مدد دیتا ہے۔

## Grafana dashboard اور alerts

- `dashboards/grafana/docs_portal.json` Grafana board **Docs Portal Publishing** publish کرتا ہے۔
  اس میں یہ panels شامل ہیں:
  - *Gateway Refusals (5m)* `torii_sorafs_gateway_refusals_total` کو `profile`/`reason` کے scope کے ساتھ استعمال کرتا ہے
    تاکہ SREs خراب policy pushes یا token failures detect کر سکیں۔
  - *Alias Cache Refresh Outcomes* اور *Alias Proof Age p90* `torii_sorafs_alias_cache_*` کو track کرتے ہیں
    تاکہ DNS cut over سے پہلے fresh proofs موجود ہونے کا ثبوت ملے۔
  - *Pin Registry Manifest Counts* اور *Active Alias Count* stat pin-registry backlog اور total aliases کو reflect کرتے ہیں
    تاکہ governance ہر release کو audit کر سکے۔
  - *Gateway TLS Expiry (hours)* اس وقت highlight کرتا ہے جب publishing gateway کا TLS cert expiry کے قریب ہو
    (alert threshold 72 h)۔
  - *Replication SLA Outcomes* اور *Replication Backlog* `torii_sorafs_replication_*` telemetry پر نظر رکھتے ہیں
    تاکہ publish کے بعد تمام replicas GA bar پر ہوں۔
- built-in template variables (`profile`, `reason`) استعمال کریں تاکہ `docs.sora` publishing profile پر focus کیا جا سکے
  یا تمام gateways میں spikes investigate کئے جا سکیں۔
- PagerDuty routing dashboard panels کو evidence کے طور پر استعمال کرتا ہے: alerts
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, اور `DocsPortal/TLSExpiry` تب fire کرتے ہیں جب متعلقہ series
  thresholds cross کرے۔ alert runbook کو اسی صفحے سے link کریں تاکہ on-call engineers exact Prometheus queries replay کر سکیں۔

## Putting it together

1. `npm run build` کے دوران release/analytics environment variables set کریں اور post-build step کو
   `checksums.sha256`, `release.json`, اور `link-report.json` emit کرنے دیں۔
2. preview hostname کے خلاف `npm run probe:portal` چلائیں اور `--expect-release` کو اسی tag سے wire کریں۔
   publishing checklist کے لئے stdout محفوظ کریں۔
3. `npm run check:links` چلائیں تاکہ broken sitemap entries پر جلد fail ہو اور generate ہونے والی JSON report کو
   preview artifacts کے ساتھ archive کریں۔ CI latest report کو `artifacts/docs_portal/link-report.json` میں drop کرتا ہے
   تاکہ governance build logs سے evidence bundle سیدھا download کر سکے۔
4. analytics endpoint کو اپنے privacy-preserving collector (Plausible، self-hosted OTEL ingest وغیرہ) کی طرف forward کریں
   اور ہر release کے لئے sampling rates document کریں تاکہ dashboards counts کو درست interpret کریں۔
5. CI پہلے ہی preview/deploy workflows میں ان steps کو wire کر چکا ہے
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`)، اس لئے local dry runs میں صرف secrets-specific behaviour cover کرنا ہوتا ہے۔
