---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: SF-6 سیکیورٹی ریویو
summary: keyless signing، proof streaming، اور manifests بھیجنے والے pipelines کی آزادانہ جانچ کے نتائج اور follow-up آئٹمز۔
---

# SF-6 سیکیورٹی ریویو

**Assessment window:** 2026-02-10 → 2026-02-18  
**Review leads:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Scope:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`)، proof streaming APIs، Torii manifest handling، Sigstore/OIDC integration، CI release hooks۔  
**Artifacts:**  
- CLI source اور tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii manifest/proof handlers (`crates/iroha_torii/src/sorafs/api.rs`)  
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministic parity harness (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Methodology

1. **Threat modelling workshops** نے developer workstations، CI systems اور Torii nodes کے لیے attacker capabilities map کیں۔  
2. **Code review** نے credential surfaces (OIDC token exchange, keyless signing)، Norito manifest validation اور proof streaming back-pressure پر فوکس کیا۔  
3. **Dynamic testing** نے fixture manifests replay کیے اور failure modes simulate کیے (token replay، manifest tampering، truncated proof streams) parity harness اور bespoke fuzz drives کے ساتھ۔  
4. **Configuration inspection** نے `iroha_config` defaults، CLI flag handling اور release scripts validate کیے تاکہ deterministic، auditable runs یقینی ہوں۔  
5. **Process interview** نے remediation flow، escalation paths اور audit evidence capture کو Tooling WG کے release owners کے ساتھ confirm کیا۔

## Findings Summary

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Keyless signing | OIDC token audience defaults CI templates میں implicit تھے، جس سے cross-tenant replay کا خطرہ تھا۔ | release hooks اور CI templates میں `--identity-token-audience` کی explicit enforcement شامل کی گئی ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). audience omit ہونے پر CI اب fail ہوتا ہے۔ |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths نے unbounded subscriber buffers قبول کیے، جس سے memory exhaustion ممکن تھی۔ | `sorafs_cli proof stream` bounded channel sizes enforce کرتا ہے، deterministic truncation کے ساتھ Norito summaries log کرتا ہے اور stream abort کرتا ہے؛ Torii mirror کو response chunks محدود کرنے کے لیے اپڈیٹ کیا گیا (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medium | Manifest submission | CLI نے manifests قبول کیے بغیر embedded chunk plans verify کیے جب `--plan` موجود نہ تھا۔ | `sorafs_cli manifest submit` اب CAR digests دوبارہ compute اور compare کرتا ہے جب تک `--expect-plan-digest` دیا نہ جائے، mismatches reject کرتا ہے اور remediation hints دکھاتا ہے۔ tests success/failure cases cover کرتے ہیں (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Low | Audit trail | Release checklist میں سیکیورٹی ریویو کے لیے signed approval log شامل نہیں تھا۔ | [release process](../developer-releases.md) میں سیکشن شامل کیا گیا جو review memo hashes اور sign-off ticket URL کو GA سے پہلے attach کرنے کا تقاضا کرتا ہے۔ |

تمام high/medium findings review window کے دوران fix ہوئیں اور موجود parity harness سے validate ہوئیں۔ کوئی latent critical issues باقی نہیں۔

## Control Validation

- **Credential scope:** Default CI templates اب explicit audience اور issuer assertions لازم کرتے ہیں؛ CLI اور release helper `--identity-token-audience` کے بغیر `--identity-token-provider` کے fail fast ہوتے ہیں۔  
- **Deterministic replay:** Updated tests positive/negative manifest submission flows cover کرتے ہیں، یہ یقینی بناتے ہیں کہ mismatched digests non-deterministic failures رہیں اور network کو touch کرنے سے پہلے surface ہوں۔  
- **Proof streaming back-pressure:** Torii اب PoR/PoTR items کو bounded channels پر stream کرتا ہے، اور CLI صرف truncated latency samples + پانچ failure exemplars رکھتا ہے، unbounded subscriber growth روکتا ہے جبکہ deterministic summaries برقرار رکھتا ہے۔  
- **Observability:** Proof streaming counters (`torii_sorafs_proof_stream_*`) اور CLI summaries abort reasons capture کرتے ہیں، جس سے operators کے لیے audit breadcrumbs ملتے ہیں۔  
- **Documentation:** Developer guides ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) security-sensitive flags اور escalation workflows کو واضح کرتے ہیں۔

## Release Checklist Additions

Release managers کو GA candidate promote کرتے وقت درج ذیل evidence **لازمی** attach کرنا ہوگا:

1. تازہ ترین سیکیورٹی ریویو memo کا hash (یہ دستاویز)۔  
2. tracked remediation ticket کا link (مثال: `governance/tickets/SF6-SR-2026.md`)۔  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` کا output جس میں explicit audience/issuer arguments دکھیں۔  
4. parity harness کے captured logs (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)۔  
5. تصدیق کہ Torii release notes میں bounded proof streaming telemetry counters شامل ہیں۔

اوپر دیے گئے artefacts جمع نہ کرنا GA sign-off کو روکتا ہے۔

**Reference artefact hashes (2026-02-20 sign-off):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Outstanding Follow-ups

- **Threat model refresh:** یہ ریویو ہر سہ ماہی یا بڑے CLI flag additions سے پہلے دوبارہ کریں۔  
- **Fuzzing coverage:** Proof streaming transport encodings کو `fuzz/proof_stream_transport` کے ذریعے fuzz کیا جاتا ہے، جو identity، gzip، deflate اور zstd payloads کو cover کرتا ہے۔  
- **Incident rehearsal:** token compromise اور manifest rollback کو simulate کرنے والی operator exercise شیڈول کریں، تاکہ docs میں practiced procedures reflect ہوں۔

## Approval

- Security Engineering Guild representative: @sec-eng (2026-02-20)  
- Tooling Working Group representative: @tooling-wg (2026-02-20)

Signed approvals کو release artefact bundle کے ساتھ محفوظ کریں۔
