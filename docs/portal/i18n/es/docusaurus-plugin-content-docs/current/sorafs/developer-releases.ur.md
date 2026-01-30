---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Release process
summary: CLI/SDK release gate چلائیں، مشترکہ versioning policy لاگو کریں، اور canonical release notes شائع کریں۔
---

# Release process

SoraFS binaries (`sorafs_cli`, `sorafs_fetch`, helpers) اور SDK crates
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) ایک ساتھ ship ہوتے ہیں۔ Release
pipeline CLI اور libraries کو aligned رکھتا ہے، lint/test coverage یقینی بناتا ہے،
اور downstream consumers کے لیے artifacts capture کرتا ہے۔ ہر candidate tag کے لیے
نیچے دی گئی checklist چلائیں۔

## 0. Security review sign-off کی تصدیق

Technical release gate چلانے سے پہلے تازہ ترین security review artifacts capture کریں:

- سب سے تازہ SF-6 security review memo ڈاؤن لوڈ کریں ([reports/sf6-security-review](./reports/sf6-security-review.md))
  اور اس کا SHA256 hash release ticket میں درج کریں۔
- Remediation ticket link (مثلاً `governance/tickets/SF6-SR-2026.md`) منسلک کریں اور
  Security Engineering اور Tooling Working Group کے sign-off approvers نوٹ کریں۔
- تصدیق کریں کہ memo کی remediation checklist بند ہے؛ unresolved items release کو block کرتے ہیں۔
- Parity harness logs (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) کو
  manifest bundle کے ساتھ upload کرنے کے لیے تیار رہیں۔
- یہ بھی تصدیق کریں کہ signing command میں `--identity-token-provider` کے ساتھ
  واضح `--identity-token-audience=<aud>` شامل ہو تاکہ Fulcio scope release evidence میں capture ہو۔

Governance کو اطلاع دیتے وقت اور release publish کرتے وقت ان artifacts کو شامل کریں۔

## 1. Release/test gate چلائیں

`ci/check_sorafs_cli_release.sh` helper CLI اور SDK crates پر formatting، Clippy اور tests
چلاتا ہے، اور workspace-local target directory (`.target`) استعمال کرتا ہے تاکہ CI
containers میں permission conflicts سے بچا جا سکے۔

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

یہ script درج ذیل assertions کرتا ہے:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` `sorafs_car` کے لیے (feature `cli` کے ساتھ)،
  `sorafs_manifest` اور `sorafs_chunker`
- `cargo test --locked --all-targets` انہی crates کے لیے

اگر کوئی قدم fail ہو تو tagging سے پہلے regression درست کریں۔ Release builds کو main
کے ساتھ continuous رہنا چاہیے؛ release branches میں fixes cherry-pick نہ کریں۔ Gate
یہ بھی چیک کرتا ہے کہ keyless signing flags (`--identity-token-issuer`, `--identity-token-audience`)
جہاں ضروری ہوں فراہم کیے گئے ہوں؛ missing arguments run کو fail کر دیتے ہیں۔

## 2. Versioning policy لاگو کریں

SoraFS CLI/SDK crates سب SemVer استعمال کرتے ہیں:

- `MAJOR`: پہلی 1.0 release میں introduce ہوتا ہے۔ 1.0 سے پہلے `0.y` minor bump
  **breaking changes** کو ظاہر کرتا ہے، چاہے وہ CLI surface میں ہوں یا Norito schemas میں۔
- `PATCH`: Bug fixes، documentation-only releases، اور dependency updates جو observable behavior کو تبدیل نہیں کرتے۔

`sorafs_car`، `sorafs_manifest` اور `sorafs_chunker` کو ہمیشہ ایک ہی version پر رکھیں تاکہ
SDK downstream consumers ایک aligned version string پر depend کر سکیں۔ Version bump کرتے وقت:

1. ہر crate کے `Cargo.toml` میں `version =` fields اپڈیٹ کریں۔
2. `cargo update -p <crate>@<new-version>` کے ذریعے `Cargo.lock` regenerate کریں (workspace explicit versions enforce کرتا ہے)۔
3. Release gate دوبارہ چلائیں تاکہ stale artifacts باقی نہ رہیں۔

## 3. Release notes تیار کریں

ہر release کو markdown changelog شائع کرنا چاہیے جو CLI، SDK اور governance-impacting changes
کو highlight کرے۔ `docs/examples/sorafs_release_notes.md` کا template استعمال کریں (اسے release
artifacts directory میں کاپی کریں اور sections کو concrete details سے بھر دیں)۔

Minimum content:

- **Highlights**: CLI اور SDK consumers کے لیے feature headlines۔
- **Upgrade steps**: cargo dependencies bump کرنے اور deterministic fixtures rerun کرنے کے TL;DR commands۔
- **Verification**: command output hashes یا envelopes اور `ci/check_sorafs_cli_release.sh` کی exact revision جو execute ہوئی۔

بھری ہوئی release notes کو tag کے ساتھ attach کریں (مثلاً GitHub release body) اور انہیں deterministically
generated artifacts کے ساتھ محفوظ کریں۔

## 4. Release hooks چلائیں

`scripts/release_sorafs_cli.sh` چلائیں تاکہ signature bundle اور verification summary generate ہو جو ہر release
کے ساتھ ship ہوتے ہیں۔ Wrapper ضرورت کے مطابق CLI build کرتا ہے، `sorafs_cli manifest sign` چلاتا ہے، اور فوراً
`manifest verify-signature` replay کرتا ہے تاکہ tagging سے پہلے failures سامنے آ جائیں۔ مثال:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Tips:

- Release inputs (payload, plans, summaries, expected token hash) کو repo یا deployment config میں track کریں تاکہ script reproducible رہے۔ `fixtures/sorafs_manifest/ci_sample/` والا fixture bundle canonical layout دکھاتا ہے۔
- CI automation کو `.github/workflows/sorafs-cli-release.yml` پر base کریں؛ یہ release gate چلاتا ہے، اوپر والا script چلاتا ہے، اور bundles/signatures کو workflow artifacts کے طور پر archive کرتا ہے۔ دوسرے CI systems میں بھی وہی command order (release gate -> sign -> verify) رکھیں تاکہ audit logs generated hashes کے ساتھ align رہیں۔
- `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`, اور `manifest.verify.summary.json` کو اکٹھا رکھیں؛ یہی packet governance notification میں refer ہوتا ہے۔
- جب release canonical fixtures اپڈیٹ کرے تو refreshed manifest، chunk plan، اور summaries کو `fixtures/sorafs_manifest/ci_sample/` میں کاپی کریں (اور `docs/examples/sorafs_ci_sample/manifest.template.json` اپڈیٹ کریں) tagging سے پہلے۔ Downstream operators committed fixtures پر depend کرتے ہیں تاکہ release bundle reproduce کر سکیں۔
- `sorafs_cli proof stream` bounded-channel verification کا run log capture کریں اور release packet کے ساتھ attach کریں تاکہ proof streaming safeguards فعال رہنے کا ثبوت ملے۔
- Signing میں استعمال ہونے والا exact `--identity-token-audience` release notes میں درج کریں؛ governance Fulcio policy کے خلاف audience کو cross-check کرتی ہے۔

جب release میں gateway rollout بھی شامل ہو تو `scripts/sorafs_gateway_self_cert.sh` استعمال کریں۔ اسی manifest bundle کی طرف point کریں تاکہ attestation candidate artifact سے match ہو:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag اور publish

Checks پاس ہونے اور hooks مکمل ہونے کے بعد:

1. `sorafs_cli --version` اور `sorafs_fetch --version` چلائیں تاکہ binaries نئی version report کریں۔
2. Release configuration کو checked-in `sorafs_release.toml` (preferred) یا کسی اور config file میں تیار کریں جو آپ کے deployment repo میں track ہو۔ Ad-hoc environment variables پر انحصار نہ کریں؛ CLI کو `--config` (یا equivalent) کے ذریعے paths دیں تاکہ release inputs واضح اور reproducible رہیں۔
3. Signed tag (preferred) یا annotated tag بنائیں:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artifacts (CAR bundles, manifests, proof summaries, release notes, attestation outputs) کو project registry میں upload کریں اور governance checklist (deployment guide: [deployment guide](./developer-deployment.md)) follow کریں۔ اگر release نے نئی fixtures بنائیں تو انہیں shared fixture repo یا object store میں push کریں تاکہ audit automation published bundle کا source control کے ساتھ diff کر سکے۔
5. Governance channel کو signed tag، release notes، manifest bundle/signature hashes، archived `manifest.sign/verify` summaries، اور attestation envelopes کے links کے ساتھ مطلع کریں۔ CI job URL (یا log archive) شامل کریں جس نے `ci/check_sorafs_cli_release.sh` اور `scripts/release_sorafs_cli.sh` چلایا۔ Governance ticket اپڈیٹ کریں تاکہ auditors approvals کو artifacts سے trace کر سکیں؛ جب `.github/workflows/sorafs-cli-release.yml` job notifications post کرے تو ad-hoc summaries کے بجائے recorded hashes link کریں۔

## 6. Post-release follow-up

- نئی version کی طرف اشارہ کرنے والی documentation (quickstarts, CI templates) اپڈیٹ کریں یا تصدیق کریں کہ کوئی تبدیلی درکار نہیں۔
- Release gate output logs کو auditors کے لیے archive کریں - انہیں signed artifacts کے ساتھ محفوظ رکھیں۔

اس pipeline کی پیروی ہر release cycle میں CLI، SDK crates اور governance collateral کو lock-step میں رکھتی ہے۔
