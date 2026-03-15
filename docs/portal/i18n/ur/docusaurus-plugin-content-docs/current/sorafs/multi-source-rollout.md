---
id: multi-source-rollout
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/multi_source_rollout.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن سیٹ ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک SRE اور آن کال انجینئرز کو دو اہم ورک فلو میں رہنمائی کرتی ہے:

1. ملٹی سورس آرکسٹریٹر کو کنٹرولڈ ویوز میں رول آؤٹ کرنا۔
2. موجودہ سیشنز کو غیر مستحکم کیے بغیر خراب کارکردگی والے پرووائیڈرز کو بلیک لسٹ کرنا یا ان کی ترجیح کم کرنا۔

یہ فرض کرتی ہے کہ SF-6 کے تحت فراہم کردہ orchestration stack پہلے ہی ڈیپلائے ہے (`sorafs_orchestrator`, gateway chunk-range API، اور telemetry exporters).

> **مزید دیکھیں:** [آرکسٹریٹر آپریشنز رن بک](./orchestrator-ops.md) فی رن طریقۂ کار (scoreboard capture، مرحلہ وار rollout toggles، اور rollback) میں تفصیل دیتی ہے۔ لائیو تبدیلیوں کے دوران دونوں حوالوں کو ساتھ استعمال کریں۔

## 1. قبل از عمل توثیق

1. **گورننس ان پٹس کی تصدیق کریں۔**
   - تمام امیدوار پرووائیڈرز کو range capability payloads اور stream budgets کے ساتھ `ProviderAdvertV1` envelopes شائع کرنے چاہئیں۔ `/v1/sorafs/providers` سے ویلیڈیٹ کریں اور متوقع capability فیلڈز سے موازنہ کریں۔
   - latency/failure rates فراہم کرنے والے telemetry snapshots ہر canary رن سے پہلے 15 منٹ سے کم پرانے ہونے چاہئیں۔
2. **کنفیگریشن اسٹیج کریں۔**
   - آرکسٹریٹر کی JSON کنفیگریشن کو layered `iroha_config` ٹری میں محفوظ کریں:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     JSON میں rollout سے متعلق حدود (`max_providers`, retry budgets) اپڈیٹ کریں۔ staging/production میں وہی فائل دیں تاکہ فرق کم رہے۔
3. **canonical fixtures چلائیں۔**
   - manifest/token environment variables سیٹ کریں اور deterministic fetch چلائیں:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     environment variables میں manifest payload digest (hex) اور ہر canary پرووائیڈر کے لیے base64-encoded stream tokens شامل ہونے چاہئیں۔
   - `artifacts/canary.scoreboard.json` کو پچھلے release سے compare کریں۔ کوئی نیا غیر اہل پرووائیڈر یا وزن میں >10% تبدیلی review مانگتی ہے۔
4. **ٹیلیمیٹری کی وائرنگ چیک کریں۔**
   - `docs/examples/sorafs_fetch_dashboard.json` میں Grafana export کھولیں۔ آگے بڑھنے سے پہلے یقینی بنائیں کہ `sorafs_orchestrator_*` metrics staging میں populate ہو رہی ہیں۔

## 2. ہنگامی پرووائیڈر بلیک لسٹنگ

جب کوئی پرووائیڈر خراب chunks فراہم کرے، مستقل timeouts دے، یا compliance checks میں فیل ہو تو یہ طریقہ کار اپنائیں۔

1. **ثبوت محفوظ کریں۔**
   - تازہ ترین fetch summary ایکسپورٹ کریں (`--json-out` کا output)۔ ناکام chunk indices، پرووائیڈر aliases، اور digest mismatches ریکارڈ کریں۔
   - `telemetry::sorafs.fetch.*` targets سے متعلقہ لاگ excerpts محفوظ کریں۔
2. **فوری override لگائیں۔**
   - آرکسٹریٹر کو دیے گئے telemetry snapshot میں پرووائیڈر کو penalized مارک کریں (`penalty=true` سیٹ کریں یا `token_health` کو `0` پر clamp کریں)۔ اگلا scoreboard build خود بخود پرووائیڈر کو exclude کر دے گا۔
   - ad-hoc smoke tests کے لیے `sorafs_cli fetch` میں `--deny-provider gw-alpha` پاس کریں تاکہ telemetry propagation کا انتظار کیے بغیر failure path exercise ہو۔
   - متاثرہ ماحول میں اپڈیٹ شدہ telemetry/config bundle دوبارہ deploy کریں (staging → canary → production)۔ تبدیلی کو incident log میں دستاویز کریں۔
3. **override ویلیڈیٹ کریں۔**
   - canonical fixture fetch دوبارہ چلائیں۔ تصدیق کریں کہ scoreboard نے پرووائیڈر کو `policy_denied` وجہ کے ساتھ ineligible مارک کیا ہے۔
   - `sorafs_orchestrator_provider_failures_total` چیک کریں تاکہ انکار شدہ پرووائیڈر کے لیے کاؤنٹر مزید نہ بڑھے۔
4. **طویل مدتی پابندی بڑھائیں۔**
   - اگر پرووائیڈر >24 h کے لیے بلاک رہے گا تو اس کے advert کو rotate یا suspend کرنے کے لیے governance ticket بنائیں۔ ووٹ پاس ہونے تک deny list برقرار رکھیں اور telemetry snapshots اپڈیٹ کرتے رہیں تاکہ پرووائیڈر دوبارہ scoreboard میں نہ آئے۔
5. **رول بیک پروٹوکول۔**
   - پرووائیڈر بحال کرنے کے لیے اسے deny list سے ہٹا دیں، دوبارہ deploy کریں، اور نیا scoreboard snapshot محفوظ کریں۔ تبدیلی کو incident postmortem کے ساتھ منسلک کریں۔

## 3. مرحلہ وار رول آؤٹ پلان

| مرحلہ | دائرہ کار | لازمی سگنلز | Go/No-Go معیار |
|-------|-----------|--------------|----------------|
| **Lab** | مخصوص integration کلسٹر | fixtures payloads کے ساتھ دستی CLI fetch | تمام chunks کامیاب ہوں، provider failure counters صفر پر رہیں، retry ratio < 5%. |
| **Staging** | مکمل control-plane staging | Grafana dashboard منسلک؛ alert rules صرف warning-only موڈ میں | `sorafs_orchestrator_active_fetches` ہر ٹیسٹ رن کے بعد صفر پر واپس آئے؛ کوئی `warn/critical` الرٹ نہ لگے۔ |
| **Canary** | پروڈکشن ٹریفک کا ≤10% | pager خاموش مگر ٹیلیمیٹری real-time مانیٹر | retry ratio < 10%، provider failures صرف معلوم noisy peers تک محدود، latency histogram staging baseline ±20% کے مطابق۔ |
| **General Availability** | 100% رول آؤٹ | pager rules فعال | 24 h تک `NoHealthyProviders` کی صفر errors، retry ratio مستحکم، dashboard SLA panels سبز۔ |

ہر مرحلے میں:

1. مطلوبہ `max_providers` اور retry budgets کے ساتھ آرکسٹریٹر JSON اپڈیٹ کریں۔
2. canonical fixture اور ماحول کے نمائندہ manifest کے خلاف `sorafs_cli fetch` یا SDK integration tests چلائیں۔
3. scoreboard + summary artifacts محفوظ کریں اور release record کے ساتھ منسلک کریں۔
4. اگلے مرحلے پر جانے سے پہلے on-call انجینئر کے ساتھ telemetry dashboards ریویو کریں۔

## 4. Observability اور Incident Hooks

- **Metrics:** یقینی بنائیں کہ Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` اور `sorafs_orchestrator_retries_total` کو مانیٹر کر رہا ہے۔ اچانک اسپائک عام طور پر یہ بتاتا ہے کہ پرووائیڈر لوڈ میں degrade ہو رہا ہے۔
- **Logs:** `telemetry::sorafs.fetch.*` targets کو مشترکہ لاگ ایگریگیٹر پر روٹ کریں۔ `event=complete status=failed` کے لیے محفوظ تلاشیں بنائیں تاکہ triage تیز ہو۔
- **Scoreboards:** ہر scoreboard artifact کو طویل مدتی اسٹوریج میں محفوظ کریں۔ JSON compliance reviews اور staged rollbacks کے لیے evidence trail بھی ہے۔
- **Dashboards:** canonical Grafana board (`docs/examples/sorafs_fetch_dashboard.json`) کو پروڈکشن فولڈر میں کلون کریں اور `docs/examples/sorafs_fetch_alerts.yaml` کی alert rules شامل کریں۔

## 5. Communication اور Documentation

- ہر deny/boost تبدیلی کو operations changelog میں timestamp، آپریٹر، وجہ اور متعلقہ incident کے ساتھ لاگ کریں۔
- جب provider weights یا retry budgets بدلیں تو SDK ٹیموں کو مطلع کریں تاکہ کلائنٹ سائیڈ توقعات ہم آہنگ رہیں۔
- GA مکمل ہونے کے بعد `status.md` میں rollout summary اپڈیٹ کریں اور اس رن بک ریفرنس کو release notes میں archive کریں۔
