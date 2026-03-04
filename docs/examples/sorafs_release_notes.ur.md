---
lang: ur
direction: rtl
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sorafs_release_notes.md کا اردو ترجمہ -->

# SoraFS CLI اور SDK - ریلیز نوٹس (v0.1.0)

## نمایاں نکات
- `sorafs_cli` اب مکمل پیکیجنگ پائپ لائن (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) کو ریپ کرتا ہے، لہذا CI runners
  مخصوص helpers کے بجائے ایک واحد بائنری چلاتے ہیں۔ نیا keyless signing flow ڈیفالٹ طور پر
  `SIGSTORE_ID_TOKEN` استعمال کرتا ہے، GitHub Actions کے OIDC providers کو سمجھتا ہے، اور signature
  bundle کے ساتھ ایک deterministic JSON summary جاری کرتا ہے۔
- multi-source fetch کا *scoreboard* `sorafs_car` کا حصہ ہے: یہ providers کی telemetry کو
  normalize کرتا ہے، capability penalties نافذ کرتا ہے، JSON/Norito reports محفوظ کرتا ہے، اور
  shared registry handle کے ذریعے orchestrator simulator (`sorafs_fetch`) کو feed کرتا ہے۔
  `fixtures/sorafs_manifest/ci_sample/` میں موجود fixtures وہ deterministic inputs اور outputs دکھاتے ہیں
  جن کے مقابلے میں CI/CD کو diff کرنا ہوتا ہے۔
- release automation کو `ci/check_sorafs_cli_release.sh` اور `scripts/release_sorafs_cli.sh` میں codify
  کیا گیا ہے۔ ہر release اب manifest bundle، signature، `manifest.sign/verify` summaries، اور scoreboard
  snapshot کو archive کرتا ہے تاکہ governance reviewers بغیر pipeline دوبارہ چلائے artefacts کو trace کر سکیں۔

## مطابقت
- Breaking changes: **کوئی نہیں۔** CLI میں تمام اضافے additive flags/subcommands ہیں؛ موجودہ invocations
  بغیر تبدیلی کے چلتے رہتے ہیں۔
- کم از کم gateway/node ورژنز: Torii `2.0.0-rc.2.0` (یا اس سے نیا) درکار ہے تاکہ chunk-range API،
  stream-token quotas، اور capability headers جو `crates/iroha_torii` expose کرتا ہے دستیاب ہوں۔
  storage nodes کو SoraFS host stack کے commit
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` پر چلنا چاہئے (نئے scoreboard inputs اور telemetry wiring شامل ہیں)۔
- Upstream dependencies: workspace baseline سے باہر کوئی third-party bump نہیں؛ release
  `Cargo.lock` میں موجود pinned `blake3`, `reqwest`, اور `sigstore` ورژنز دوبارہ استعمال کرتی ہے۔

## اپ گریڈ کے مراحل
1. اپنے workspace میں aligned crates اپ ڈیٹ کریں:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. fmt/clippy/tests کوریج کی تصدیق کے لئے release gate کو مقامی طور پر (یا CI میں) دوبارہ چلائیں:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. curated config کے ساتھ signed artefacts اور summaries دوبارہ بنائیں:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   اگر release canonical fixtures اپ ڈیٹ کرے تو bundles/proofs کو
   `fixtures/sorafs_manifest/ci_sample/` میں کاپی کریں۔

## تصدیق
- Release gate commit: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` gate کے کامیاب ہونے کے فوراً بعد)۔
- `ci/check_sorafs_cli_release.sh` output: `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` میں محفوظ
  (release bundle کے ساتھ منسلک)۔
- Manifest bundle digest: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`)۔
- Proof summary digest: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`)۔
- Manifest digest (downstream attestation cross-checks کے لئے):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json` سے)۔

## آپریٹرز کے لئے نوٹس
- Torii gateway اب capability header `X-Sora-Chunk-Range` نافذ کرتا ہے۔ allowlists اپ ڈیٹ کریں تاکہ
  نئے stream token scopes والے clients کو اجازت ملے؛ range claim کے بغیر پرانے tokens throttled ہوں گے۔
- `scripts/sorafs_gateway_self_cert.sh` manifest verification کو integrate کرتا ہے۔ self-cert harness
  چلانے پر تازہ generate کردہ manifest bundle دیں تاکہ signature drift پر wrapper جلد fail ہو جائے۔
- Telemetry dashboards کو نئے scoreboard export (`scoreboard.json`) کو ingest کرنا چاہئے تاکہ providers
  کی eligibility، weight assignments، اور refusal reasons میں ہم آہنگی ہو۔
- ہر rollout کے ساتھ چار canonical summaries archive کریں:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. governance tickets انہی درست فائلوں کا حوالہ دیتے ہیں۔

## شکریہ
- Storage Team - end-to-end CLI consolidation، chunk-plan renderer، اور scoreboard telemetry wiring.
- Tooling WG - release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) اور deterministic fixtures bundle.
- Gateway Operations - capability gating، stream-token policy review، اور اپ ڈیٹ شدہ self-cert playbooks.

</div>
