---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# سیکیورٹی ہارڈننگ اور pen-test چیک لسٹ

## جائزہ

روڈمیپ آئٹم **DOCS-1b** OAuth device-code login، مضبوط content security policies،
اور repeatable penetration tests کا تقاضا کرتا ہے اس سے پہلے کہ preview پورٹل لیب سے باہر
نیٹ ورکس پر چل سکے۔ یہ ضمیمہ threat model، repo میں implement کئے گئے controls، اور go-live
چیک لسٹ بیان کرتا ہے جسے gate reviews کو execute کرنا ہوتا ہے۔

- **اسکوپ:** Try it proxy، embedded Swagger/RapiDoc panels، اور custom Try it console جو
  `docs/portal/src/components/TryItConsole.jsx` سے render ہوتی ہے۔
- **آؤٹ آف اسکوپ:** Torii خود (Torii readiness reviews میں کور) اور SoraFS publishing
  (DOCS-3/7 میں کور).

## Threat model

| Asset | Risk | Mitigation |
| --- | --- | --- |
| Torii bearer tokens | docs sandbox کے باہر theft یا reuse | device-code login (`DOCS_OAUTH_*`) short-lived tokens mint کرتا ہے، proxy headers کو redact کرتا ہے، اور console cached credentials کو auto-expire کرتی ہے۔ |
| Try it proxy | open relay کے طور پر abuse یا Torii rate limits کا bypass | `scripts/tryit-proxy*.mjs` origin allowlists، rate limiting، health probes، اور `X-TryIt-Auth` کی explicit forwarding enforce کرتا ہے؛ کوئی credentials persist نہیں ہوتے۔ |
| Portal runtime | cross-site scripting یا malicious embeds | `docusaurus.config.js` Content-Security-Policy، Trusted Types، اور Permissions-Policy headers inject کرتا ہے؛ inline scripts Docusaurus runtime تک محدود ہیں۔ |
| Observability data | missing telemetry یا tampering | `docs/portal/docs/devportal/observability.md` probes/dashboards document کرتا ہے؛ `scripts/portal-probe.mjs` publish سے پہلے CI میں چلتا ہے۔ |

Adversaries میں public preview دیکھنے والے curious users، چوری شدہ links کو test کرنے والے malicious actors،
اور compromised browsers شامل ہیں جو stored credentials scrape کرنے کی کوشش کرتے ہیں۔ تمام controls کو
trusted networks کے بغیر commodity browsers پر کام کرنا چاہیے۔

## Required controls

1. **OAuth device-code login**
   - `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` اور متعلقہ knobs کو build environment میں configure کریں۔
   - Try it card ایک sign-in widget (`OAuthDeviceLogin.jsx`) render کرتا ہے جو device code fetch کرتا ہے،
     token endpoint پر poll کرتا ہے، اور expiry پر tokens کو auto-clear کرتا ہے۔ Manual Bearer overrides
     emergency fallback کے لئے دستیاب رہتے ہیں۔
   - OAuth config missing ہو یا fallback TTLs DOCS-1b کے 300-900 s window سے باہر ہوں تو builds fail ہوتے ہیں؛
     `DOCS_OAUTH_ALLOW_INSECURE=1` صرف disposable local previews کے لئے set کریں۔
2. **Proxy guardrails**
   - `scripts/tryit-proxy.mjs` allowed origins، rate limits، request size caps، اور upstream timeouts enforce کرتا ہے
     جبکہ `X-TryIt-Client` کے ساتھ traffic tag کرتا ہے اور logs سے tokens redact کرتا ہے۔
   - `scripts/tryit-proxy-probe.mjs` اور `docs/portal/docs/devportal/observability.md` liveness probe
     اور dashboard rules define کرتے ہیں؛ ہر rollout سے پہلے انہیں چلائیں۔
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` اب deterministic security headers export کرتا ہے:
     `Content-Security-Policy` (default-src self، سخت connect/img/script lists، Trusted Types requirements)،
     `Permissions-Policy`، اور `Referrer-Policy: no-referrer`۔
   - CSP connect list OAuth device-code اور token endpoints کو whitelist کرتی ہے
     (صرف HTTPS، جب تک `DOCS_SECURITY_ALLOW_INSECURE=1` نہ ہو) تاکہ device login کام کرے
     اور دوسرے origins کے لئے sandbox relax نہ ہو۔
   - headers براہ راست generated HTML میں embed ہوتے ہیں، اس لئے static hosts کو اضافی configuration کی ضرورت نہیں۔
     inline scripts کو Docusaurus bootstrap تک محدود رکھیں۔
4. **Runbooks, observability, اور rollback**
   - `docs/portal/docs/devportal/observability.md` probes اور dashboards بیان کرتا ہے جو login failures، proxy response codes،
     اور request budgets پر نظر رکھتے ہیں۔
   - `docs/portal/docs/devportal/incident-runbooks.md` escalation path کور کرتا ہے اگر sandbox abuse ہو؛
     `scripts/tryit-proxy-rollback.mjs` کے ساتھ combine کریں تاکہ endpoints محفوظ طریقے سے switch ہوں۔

## Pen-test اور release checklist

ہر preview promotion کے لئے یہ فہرست مکمل کریں (results کو release ticket کے ساتھ attach کریں):

1. **OAuth wiring verify کریں**
   - `npm run start` کو production `DOCS_OAUTH_*` exports کے ساتھ local چلائیں۔
   - صاف browser profile سے Try it console کھولیں اور confirm کریں کہ device-code flow token mint کرتا ہے،
     lifetime count down کرتا ہے، اور expiry یا sign-out کے بعد field clear کرتا ہے۔
2. **Proxy probe کریں**
   - `npm run tryit-proxy` کو staging Torii کے خلاف چلائیں، پھر
     `npm run probe:tryit-proxy` configured sample path کے ساتھ چلائیں۔
   - logs میں `authSource=override` entries دیکھیں اور confirm کریں کہ rate limiting window exceed ہونے پر
     counters increase ہوتے ہیں۔
3. **CSP/Trusted Types confirm کریں**
   - `npm run build` چلائیں اور `build/index.html` کھولیں۔ یقینی بنائیں کہ `<meta
     http-equiv="Content-Security-Policy">` tag expected directives سے match کرتا ہے
     اور preview load ہوتے وقت DevTools کوئی CSP violations نہیں دکھاتا۔
   - `npm run probe:portal` (یا curl) سے deployed HTML fetch کریں؛ probe اب fail ہو جاتا ہے اگر
     `Content-Security-Policy`, `Permissions-Policy`, یا `Referrer-Policy` meta tags غائب ہوں یا
     `docusaurus.config.js` میں declared values سے مختلف ہوں، لہذا governance reviewers exit code پر
     اعتماد کر سکتے ہیں بجائے curl output دیکھنے کے۔
4. **Observability review کریں**
   - Try it proxy dashboard سبز ہو (rate limits, error ratios, health probe metrics)۔
   - اگر host بدلا ہو (نیا Netlify/SoraFS deployment) تو `docs/portal/docs/devportal/incident-runbooks.md`
     میں incident drill چلائیں۔
5. **نتائج document کریں**
   - screenshots/logs کو release ticket کے ساتھ attach کریں۔
   - ہر finding کو remediation report template میں capture کریں
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     تاکہ owners، SLAs، اور retest evidence بعد میں آسانی سے audit ہو سکے۔
   - اس checklist کو link کریں تاکہ DOCS-1b roadmap item auditable رہے۔

اگر کوئی قدم fail ہو جائے تو promotion روک دیں، ایک blocking issue فائل کریں، اور `status.md` میں remediation plan نوٹ کریں۔
