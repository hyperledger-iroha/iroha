---
id: portal-publish-plan
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/portal_publish_plan.md` کی عکاسی کرتا ہے۔ جب ورک فلو بدل جائے تو دونوں کاپیز اپڈیٹ کریں۔
:::

روڈ میپ آئٹم DOCS-7 تقاضا کرتا ہے کہ ہر docs artefact (پورٹل build، OpenAPI spec،
SBOMs) SoraFS manifest pipeline سے گزرے اور `docs.sora` کے ذریعے `Sora-Proof`
headers کے ساتھ فراہم ہو۔ یہ چیک لسٹ موجودہ helpers کو جوڑتی ہے تاکہ Docs/DevRel، Storage اور Ops
متعدد runbooks کھنگالے بغیر ریلیز چلا سکیں۔

## 1. Build اور payloads کی پیکجنگ

پیکجنگ helper چلائیں (dry-run کے لیے skip options دستیاب ہیں):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` اگر CI پہلے ہی تیار کر چکا ہو تو `docs/portal/build` دوبارہ استعمال کرتا ہے۔
- جب `syft` دستیاب نہ ہو (مثلا air-gapped rehearsal) تو `--skip-sbom` شامل کریں۔
- اسکرپٹ پورٹل ٹیسٹس چلاتا ہے، `portal`, `openapi`, `portal-sbom`, `openapi-sbom` کے لیے CAR + manifest جوڑے بناتا ہے، `--proof` کے ساتھ ہر CAR verify کرتا ہے، اور `--sign` کے ساتھ Sigstore bundles بناتا ہے۔
- آؤٹ پٹ اسٹرکچر:

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

پورا فولڈر برقرار رکھیں (یا `artifacts/devportal/sorafs/latest` کے ذریعے symlink بنائیں) تاکہ
گورننس ریویورز build artifacts کو ٹریس کر سکیں۔

## 2. Manifests اور aliases کا Pin

`sorafs_cli manifest submit` کے ذریعے manifests کو Torii پر پش کریں اور aliases بائنڈ کریں۔
`${SUBMITTED_EPOCH}` کو تازہ ترین consensus epoch پر سیٹ کریں (مثلا
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` یا ڈیش بورڈ سے)۔

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<katakana-i105-account-id>"
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- `openapi.manifest.to` اور SBOM manifests کے لیے بھی دہرائیں (SBOM bundles کے لیے alias flags ہٹا دیں جب تک governance کوئی namespace تفویض نہ کرے)۔
- متبادل: `iroha app sorafs pin register` submit summary کے digest کے ساتھ کام کرتا ہے اگر بائنری پہلے سے نصب ہو۔
- registry کی حالت چیک کریں:
  `iroha app sorafs pin list --alias docs:portal --format json | jq`۔
- دیکھنے والے dashboards: `sorafs_pin_registry.json` (`torii_sorafs_replication_*` میٹرکس)۔

## 3. Gateway headers اور proofs

HTTP headers بلاک + binding metadata بنائیں:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- ٹیمپلیٹ میں `Sora-Name`, `Sora-CID`, `Sora-Proof`, `Sora-Proof-Status` کے ساتھ default CSP/HSTS/Permissions-Policy شامل ہے۔
- `--rollback-manifest-json` استعمال کریں تاکہ rollback headers سیٹ بن سکے۔

ٹریفک ایکسپوز کرنے سے پہلے چلائیں:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- probe GAR signature freshness، alias policy اور TLS fingerprints کو enforce کرتا ہے۔
- self-cert harness `sorafs_fetch` کے ذریعے manifest ڈاؤن لوڈ کرتا ہے اور CAR replay logs محفوظ کرتا ہے؛ outputs کو audit evidence کے لیے رکھیں۔

## 4. DNS اور telemetry guardrails

1. DNS skeleton ریفریش کریں تاکہ governance binding ثابت کر سکے:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. رول آؤٹ کے دوران مانیٹر کریں:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` اور pin registry بورڈ۔

3. alert rules کا smoke کریں (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) اور
   ریلیز آرکائیو کے لیے logs/screenshots محفوظ کریں۔

## 5. Evidence bundle

ریلیز ٹکٹ یا governance پیکج میں یہ شامل کریں:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  Sigstore bundles, submit summaries).
- Gateway probe + self-cert outputs
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleton + header templates (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Dashboard screenshots + alert acknowledgements.
- `status.md` اپڈیٹ جس میں manifest digest اور alias binding time درج ہو۔

اس چیک لسٹ پر عمل کرنے سے DOCS-7 پورا ہوتا ہے: portal/OpenAPI/SBOM payloads
deterministic طور پر پیکج ہوتے ہیں، aliases سے pin ہوتے ہیں، `Sora-Proof`
headers کے ذریعے محفوظ ہوتے ہیں، اور موجودہ observability stack سے end-to-end
مانیٹر ہوتے ہیں۔
