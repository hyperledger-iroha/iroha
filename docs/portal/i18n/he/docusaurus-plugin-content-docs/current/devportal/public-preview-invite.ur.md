---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# پبلک پریویو دعوتی پلے بک

## پروگرام کے مقاصد

یہ پلے بک وضاحت کرتی ہے کہ ریویور آن بورڈنگ ورک فلو فعال ہونے کے بعد پبلک پریویو کیسے اعلان اور چلایا جائے۔
یہ DOCS-SORA روڈمیپ کو دیانت دار رکھتی ہے کیونکہ ہر دعوت کے ساتھ قابل تصدیق artifacts، سیکیورٹی رہنمائی،
اور واضح feedback راستہ شامل ہونا یقینی بنایا جاتا ہے۔

- **آڈیئنس:** کمیونٹی ممبرز، پارٹنرز اور maintainers کی curated فہرست جنہوں نے preview acceptable-use پالیسی سائن کی ہے۔
- **سیلنگز:** default wave size <= 25 ریویورز، 14 دن کی access window، اور 24h کے اندر incident response۔

## لانچ گیٹ چیک لسٹ

کوئی بھی دعوت بھیجنے سے پہلے یہ کام مکمل کریں:

1. تازہ ترین preview artifacts CI میں اپلوڈ ہوں (`docs-portal-preview`,
   checksum manifest, descriptor, SoraFS bundle)۔
2. `npm run --prefix docs/portal serve` (checksum-gated) اسی tag پر ٹیسٹ کیا گیا ہو۔
3. ریویور آن بورڈنگ ٹکٹس approve ہوں اور invite wave سے لنک ہوں۔
4. سیکیورٹی، observability، اور incident ڈاکس validate ہوں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))۔
5. feedback فارم یا issue template تیار ہو (severity، reproduction steps، screenshots، اور environment info کے فیلڈز شامل ہوں)۔
6. اعلان کی کاپی Docs/DevRel + Governance نے ریویو کی ہو۔

## دعوتی پیکیج

ہر دعوت میں شامل ہونا چاہیے:

1. **Verified artifacts** — SoraFS manifest/plan یا GitHub artefact کے لنکس دیں،
   ساتھ میں checksum manifest اور descriptor بھی دیں۔ verification کمانڈ واضح طور پر لکھیں تاکہ
   ریویورز site لانچ کرنے سے پہلے اسے چلا سکیں۔
2. **Serve instructions** — checksum-gated preview کمانڈ شامل کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Security reminders** — واضح کریں کہ tokens خود بخود expire ہوتے ہیں، لنکس شیئر نہیں کیے جائیں،
   اور incidents فوراً رپورٹ کیے جائیں۔
4. **Feedback channel** — issue template/form لنک کریں اور response time expectations واضح کریں۔
5. **Program dates** — start/end dates، office hours یا syncs، اور اگلی refresh window فراہم کریں۔

نمونہ ای میل
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
میں دستیاب ہے اور یہ requirements پوری کرتا ہے۔ بھیجنے سے پہلے placeholders (dates, URLs, contacts)
اپ ڈیٹ کریں۔

## پریویو host کو expose کریں

جب تک onboarding مکمل نہ ہو اور change ticket منظور نہ ہو تب تک preview host کو promote نہ کریں۔
اس سیکشن کے build/publish/verify end-to-end steps کے لئے
[preview host exposure guide](./preview-host-exposure.md) دیکھیں۔

1. **Build اور پیکیجنگ:** release tag stamp کریں اور deterministic artifacts تیار کریں۔

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   pin script `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   اور `portal.dns-cutover.json` کو `artifacts/sorafs/` میں لکھتا ہے۔ ان فائلوں کو invite wave
   کے ساتھ attach کریں تاکہ ہر ریویور وہی bits verify کر سکے۔

2. **Preview alias publish کریں:** کمانڈ کو `--skip-submit` کے بغیر دوبارہ چلائیں
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` اور governance-issued alias proof فراہم کریں)۔
   اسکرپٹ `docs-preview.sora` پر manifest bind کرے گا اور evidence bundle کے لئے
   `portal.manifest.submit.summary.json` اور `portal.pin.report.json` نکالے گا۔

3. **Deployment probe کریں:** invites بھیجنے سے پہلے alias resolve ہونا اور checksum کا tag سے match ہونا
   یقینی بنائیں۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) کو fallback کے طور پر handy رکھیں تاکہ
   اگر preview edge میں مسئلہ ہو تو ریویورز لوکل کاپی چلا سکیں۔

## کمیونیکیشن ٹائم لائن

| دن | ایکشن | Owner |
| --- | --- | --- |
| D-3 | دعوتی کاپی finalize کرنا، artifacts refresh کرنا، verification کا dry-run | Docs/DevRel |
| D-2 | Governance sign-off + change ticket | Docs/DevRel + Governance |
| D-1 | template کے ذریعے دعوتیں بھیجیں، tracker میں recipient list اپ ڈیٹ کریں | Docs/DevRel |
| D | kickoff call / office hours، telemetry dashboards مانیٹر کریں | Docs/DevRel + On-call |
| D+7 | midpoint feedback digest، blocking issues کی triage | Docs/DevRel |
| D+14 | wave بند کریں، عارضی رسائی revoke کریں، `status.md` میں خلاصہ شائع کریں | Docs/DevRel |

## Access tracking اور telemetry

1. ہر recipient، invite timestamp، اور revocation date کو preview feedback logger کے ساتھ ریکارڈ کریں
   (دیکھیں [`preview-feedback-log`](./preview-feedback-log)) تاکہ ہر wave ایک ہی evidence trail شیئر کرے:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Supported events ہیں `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, اور `access-revoked`۔ log ڈیفالٹ طور پر
   `artifacts/docs_portal_preview/feedback_log.json` میں موجود ہے؛ اسے invite wave ٹکٹ کے ساتھ
   consent forms سمیت attach کریں۔ close-out نوٹ سے پہلے summary helper استعمال کریں تاکہ
   ایک auditable roll-up تیار ہو:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   summary JSON ہر wave کے invites، کھلے recipients، feedback counts، اور حالیہ ترین event کے
   timestamp کو enumerate کرتا ہے۔ helper
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   پر مبنی ہے، اس لئے وہی workflow لوکل یا CI میں چل سکتا ہے۔ recap شائع کرتے وقت
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   والا digest template استعمال کریں۔
2. telemetry dashboards کو wave میں استعمال ہونے والے `DOCS_RELEASE_TAG` کے ساتھ tag کریں تاکہ
   spikes کو invite cohorts سے correlate کیا جا سکے۔
3. deploy کے بعد `npm run probe:portal -- --expect-release=<tag>` چلائیں تاکہ preview environment
   درست release metadata advertise کرے۔
4. کسی بھی incident کو runbook template میں capture کریں اور اسے cohort سے link کریں۔

## Feedback اور close-out

1. feedback کو shared doc یا issue board میں جمع کریں۔ items کو `docs-preview/<wave>` سے tag کریں تاکہ
   roadmap owners انہیں آسانی سے query کر سکیں۔
2. preview logger کی summary output سے wave report بھریں، پھر cohort کو `status.md` میں summarize کریں
   (participants، بڑے findings، planned fixes) اور اگر DOCS-SORA milestone بدلا ہو تو `roadmap.md` اپ ڈیٹ کریں۔
3. [`reviewer-onboarding`](./reviewer-onboarding.md) کے offboarding steps follow کریں: access revoke کریں،
   requests archive کریں، اور participants کا شکریہ ادا کریں۔
4. اگلی wave کے لئے artifacts refresh کریں، checksum gates دوبارہ چلائیں، اور invite template کو نئی dates سے اپ ڈیٹ کریں۔

اس playbook کو مسلسل لاگو کرنے سے preview پروگرام auditable رہتا ہے اور Docs/DevRel کو دعوتیں
اسکیل کرنے کا repeatable طریقہ ملتا ہے جیسے جیسے پورٹل GA کے قریب آتا ہے۔
