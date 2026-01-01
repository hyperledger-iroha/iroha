---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-11-08T20:22:34+00:00"
translation_last_reviewed: 2026-01-01
---

# انسیڈنٹ رن بکس اور رول بیک ڈرلز

## مقصد

روڈمیپ آئٹم **DOCS-9** قابل عمل playbooks اور rehearsal پلان کا تقاضا کرتا ہے تاکہ
پورٹل آپریٹرز ڈلیوری فیلئرز سے بغیر اندازے کے ریکور کر سکیں۔ یہ نوٹ تین ہائی سگنل
انسیڈنٹس — ناکام deployments، replication degradation، اور analytics outages — کو کور کرتا ہے
اور quarterly drills کو دستاویزی بناتا ہے جو ثابت کرتے ہیں کہ alias rollback اور synthetic validation
اب بھی end to end کام کرتے ہیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — packaging، signing، اور alias promotion workflow۔
- [`devportal/observability`](./observability) — release tags، analytics، اور probes جن کا نیچے حوالہ ہے۔
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — registry telemetry اور escalation thresholds۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور `npm run probe:*` helpers
  جو چیک لسٹس میں ریفرنس ہیں۔

### مشترکہ telemetry اور tooling

| Signal / Tool | مقصد |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | replication stalls اور SLA breaches detect کرتا ہے۔ |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | backlog depth اور completion latency کو triage کے لئے quantify کرتا ہے۔ |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | gateway-side failures دکھاتا ہے جو اکثر خراب deploy کے بعد آتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | synthetic probes جو releases gate کرتے ہیں اور rollbacks validate کرتے ہیں۔ |
| `npm run check:links` | broken-link gate; ہر mitigation کے بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے) | alias promotion/reversion mechanism۔ |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | refusals/alias/TLS/replication telemetry کو aggregate کرتا ہے۔ PagerDuty alerts ان panels کو evidence کے طور پر refer کرتے ہیں۔ |

## Runbook - ناکام deployment یا خراب artefact

### شروع ہونے کی شرائط

- preview/production probes fail (`npm run probe:portal -- --expect-release=...`).
- Grafana alerts on `torii_sorafs_gateway_refusals_total` یا
  `torii_sorafs_manifest_submit_total{status="error"}` rollout کے بعد۔
- manual QA alias promotion کے فوراً بعد broken routes یا Try it proxy failures نوٹ کرے۔

### فوری روک تھام

1. **Deployments freeze کریں:** CI pipeline کو `DEPLOY_FREEZE=1` سے mark کریں (GitHub workflow input)
   یا Jenkins job pause کریں تاکہ مزید artefacts نہ نکلیں۔
2. **Artefacts capture کریں:** failing build کے `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`، اور probe output ڈاؤن لوڈ کریں تاکہ rollback
   عین digests کو reference کرے۔
3. **Stakeholders کو اطلاع دیں:** storage SRE، Docs/DevRel lead، اور governance duty officer
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. last-known-good (LKG) manifest کی شناخت کریں۔ production workflow انہیں
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. shipping helper سے alias کو اس manifest پر دوبارہ bind کریں:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. rollback summary کو incident ticket میں LKG اور failed manifest digests کے ساتھ ریکارڈ کریں۔

### توثیق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   (deploy guide دیکھیں) تاکہ تصدیق ہو کہ repromoted manifest archived CAR کے ساتھ match کرتا ہے۔
4. `npm run probe:tryit-proxy` تاکہ Try-It staging proxy کی بحالی یقینی ہو۔

### واقعے کے بعد

1. root cause سمجھ میں آنے کے بعد ہی deployment pipeline دوبارہ فعال کریں۔
2. [`devportal/deploy-guide`](./deploy-guide) میں "Lessons learned" entries کو نئے points سے بھر دیں، اگر ہوں۔
3. failing test suite (probe, link checker, وغیرہ) کے لئے defects فائل کریں۔

## Runbook - ریپلیکیشن میں گراوٹ

### شروع ہونے کی شرائط

- الرٹ: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 منٹ تک۔
- `torii_sorafs_replication_backlog_total > 10` 10 منٹ تک (`pin-registry-ops.md` دیکھیں)۔
- Governance ریلیز کے بعد alias کی دستیابی سست ہونے کی رپورٹ کرے۔

### ابتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) dashboards دیکھیں تاکہ معلوم ہو سکے کہ backlog
   کسی storage class یا provider fleet تک محدود ہے یا نہیں۔
2. Torii logs میں `sorafs_registry::submit_manifest` warnings چیک کریں تاکہ معلوم ہو سکے کہ submissions fail ہو رہی ہیں یا نہیں۔
3. `sorafs_cli manifest status --manifest ...` کے ذریعے replica health sample کریں (per-provider outcomes دکھاتا ہے)۔

### تخفیفی اقدامات

1. `scripts/sorafs-pin-release.sh` کے ذریعے زیادہ replica count (`--pin-min-replicas 7`) کے ساتھ manifest دوبارہ جاری کریں
   تاکہ scheduler load کو زیادہ providers پر پھیلا دے۔ نیا digest incident log میں ریکارڈ کریں۔
2. اگر backlog کسی ایک provider سے جڑا ہو تو replication scheduler کے ذریعے اسے عارضی طور پر disable کریں
   (`pin-registry-ops.md` میں documented) اور نیا manifest submit کریں جو دوسرے providers کو alias refresh کرنے پر مجبور کرے۔
3. اگر alias freshness، replication parity سے زیادہ critical ہو تو alias کو پہلے سے staged گرم manifest (`docs-preview`) پر rebind کریں،
   پھر SRE کے backlog clear کرنے کے بعد follow-up manifest publish کریں۔

### بحالی اور اختتام

1. `torii_sorafs_replication_sla_total{outcome="missed"}` کو monitor کریں تاکہ count plateau ہو۔
2. `sorafs_cli manifest status` output کو evidence کے طور پر capture کریں کہ ہر replica دوبارہ compliant ہے۔
3. replication backlog post-mortem کو فائل یا اپ ڈیٹ کریں (provider scaling, chunker tuning, وغیرہ)۔

## Runbook - اینالیٹکس یا ٹیلیمیٹری آؤٹیج

### شروع ہونے کی شرائط

- `npm run probe:portal` کامیاب ہو لیکن dashboards `AnalyticsTracker` events کو >15 minutes تک ingest نہ کریں۔
- Privacy review dropped events میں غیر متوقع اضافہ رپورٹ کرے۔
- `npm run probe:tryit-proxy` `/probe/analytics` paths پر fail ہو۔

### جوابی اقدامات

1. build-time inputs verify کریں: `DOCS_ANALYTICS_ENDPOINT` اور `DOCS_ANALYTICS_SAMPLE_RATE`
   failing release artifact (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کو staging collector پر point کر کے `npm run probe:portal` دوبارہ چلائیں تاکہ tracker payloads emit کرنا ثابت ہو۔
3. اگر collectors down ہوں تو `DOCS_ANALYTICS_ENDPOINT=""` set کریں اور rebuild کریں تاکہ tracker short-circuit کرے؛ outage window incident timeline میں ریکارڈ کریں۔
4. validate کریں کہ `scripts/check-links.mjs` اب بھی `checksums.sha256` fingerprint کرتا ہے
   (analytics outages کو sitemap validation *block* نہیں کرنا چاہیے)۔
5. collector recover ہونے کے بعد `npm run test:widgets` چلائیں تاکہ analytics helper unit tests run ہوں پھر republish کریں۔

### واقعے کے بعد

1. [`devportal/observability`](./observability) میں نئی collector limitations یا sampling requirements اپ ڈیٹ کریں۔
2. اگر analytics data policy کے باہر drop یا redact ہوا ہو تو governance notice فائل کریں۔

## سہ ماہی استقامت کی مشقیں

دونوں drills **ہر quarter کے پہلے منگل** (Jan/Apr/Jul/Oct) کو چلائیں
یا کسی بڑے infrastructure change کے فوراً بعد۔ artifacts کو
`artifacts/devportal/drills/<YYYYMMDD>/` کے تحت محفوظ کریں۔

| مشق | مراحل | شواہد |
| ----- | ----- | -------- |
| Alias rollback کی مشق | 1. تازہ ترین production manifest کے ساتھ "Failed deployment" rollback دوبارہ چلائیں۔<br/>2. probes pass ہونے کے بعد production پر re-bind کریں۔<br/>3. `portal.manifest.submit.summary.json` اور probe logs کو drill فولڈر میں ریکارڈ کریں۔ | `rollback.submit.json`, probe output, اور rehearsal کا release tag۔ |
| مصنوعی توثیق کا آڈٹ | 1. production اور staging کے خلاف `npm run probe:portal` اور `npm run probe:tryit-proxy` چلائیں۔<br/>2. `npm run check:links` چلائیں اور `build/link-report.json` archive کریں۔<br/>3. Grafana panels کے screenshots/exports attach کریں جو probe success confirm کریں۔ | Probe logs + `link-report.json` جو manifest fingerprint کا حوالہ دیتا ہے۔ |

Missed drills کو Docs/DevRel manager اور SRE governance review تک escalate کریں، کیونکہ roadmap یہ تقاضا کرتا ہے کہ
alias rollback اور portal probes کے صحت مند رہنے کا deterministic، quarterly evidence موجود ہو۔

## PagerDuty اور on-call coordination

- PagerDuty service **Docs Portal Publishing** `dashboards/grafana/docs_portal.json` سے پیدا ہونے والے alerts کی مالک ہے۔
  قواعد `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, اور `DocsPortal/TLSExpiry` Docs/DevRel primary کو page کرتے ہیں
  اور Storage SRE secondary ہوتا ہے۔
- page آنے پر `DOCS_RELEASE_TAG` شامل کریں، متاثرہ Grafana panels کے screenshots attach کریں، اور mitigation شروع کرنے سے پہلے
  probe/link-check output کو incident notes میں link کریں۔
- mitigation (rollback یا redeploy) کے بعد `npm run probe:portal`, `npm run check:links` دوبارہ چلائیں، اور تازہ Grafana
  snapshots capture کریں جو metrics کو thresholds کے اندر دکھائیں۔ تمام evidence PagerDuty incident کے ساتھ attach کریں
  قبل ازیں کہ اسے resolve کیا جائے۔
- اگر دو alerts ایک ساتھ fire ہوں (مثال کے طور پر TLS expiry اور backlog)، تو refusals کو پہلے triage کریں
  (publishing روکیں)، rollback procedure چلائیں، پھر Storage SRE کے ساتھ bridge پر TLS/backlog صاف کریں۔
