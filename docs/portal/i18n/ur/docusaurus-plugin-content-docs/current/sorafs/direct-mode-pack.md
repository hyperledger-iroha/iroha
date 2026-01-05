<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: direct-mode-pack
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

SoraNet circuits SoraFS کے لیے default transport ہیں، مگر roadmap item **SNNet-5a** ایک regulated fallback کا تقاضا کرتا ہے تاکہ anonymity rollout مکمل ہونے تک operators deterministic read-access برقرار رکھ سکیں۔ یہ pack CLI/SDK knobs، config profiles، compliance tests اور deployment checklist کو اکٹھا کرتا ہے جو privacy transports کو چھیڑے بغیر SoraFS کو direct Torii/QUIC mode میں چلانے کے لیے درکار ہیں۔

یہ fallback staging اور regulated production environments پر لاگو ہوتا ہے جب تک SNNet-5 سے SNNet-9 اپنے readiness gates پاس نہ کر لیں۔ نیچے والے artifacts کو معمول کے SoraFS deployment collateral کے ساتھ رکھیں تاکہ operators ضرورت پڑنے پر anonymous اور direct modes کے درمیان سوئچ کر سکیں۔

## 1. CLI اور SDK flags

- `sorafs_cli fetch --transport-policy=direct-only ...` relay scheduling disable کرتا ہے اور Torii/QUIC transports enforce کرتا ہے۔ CLI help اب `direct-only` کو accepted value کے طور پر دکھاتی ہے۔
- SDKs کو `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` set کرنا چاہیے جب بھی وہ "direct mode" toggle expose کریں۔ `iroha::ClientOptions` اور `iroha_android` میں generated bindings وہی enum forward کرتے ہیں۔
- Gateway harnesses (`sorafs_fetch`, Python bindings) shared Norito JSON helpers کے ذریعے direct-only toggle parse کر سکتے ہیں تاکہ automation کو ایک جیسا behavior ملے۔

Partner-facing runbooks میں flag document کریں اور feature toggles کو environment variables کے بجائے `iroha_config` کے ذریعے wire کریں۔

## 2. Gateway policy profiles

Deterministic orchestrator config persist کرنے کے لیے Norito JSON استعمال کریں۔ `docs/examples/sorafs_direct_mode_policy.json` میں example profile یہ encode کرتا ہے:

- `transport_policy: "direct_only"` — ان providers کو reject کرتا ہے جو صرف SoraNet relay transports advertise کرتے ہیں۔
- `max_providers: 2` — direct peers کو سب سے reliable Torii/QUIC endpoints تک محدود کرتا ہے۔ Regional compliance allowances کے مطابق adjust کریں۔
- `telemetry_region: "regulated-eu"` — emitted metrics کو label کرتا ہے تاکہ dashboards/audits fallback runs کو الگ پہچان سکیں۔
- Conservative retry budgets (`retry_budget: 2`, `provider_failure_threshold: 3`) تاکہ misconfigured gateways mask نہ ہوں۔

JSON کو `sorafs_cli fetch --config` (automation) یا SDK bindings (`config_from_json`) کے ذریعے load کریں، پھر operators کے سامنے policy expose کریں۔ Audit trails کے لیے scoreboard output (`persist_path`) محفوظ کریں۔

Gateway-side enforcement knobs `docs/examples/sorafs_gateway_direct_mode.toml` میں درج ہیں۔ یہ template `iroha sorafs gateway direct-mode enable` کی output کو mirror کرتا ہے، envelope/admission checks disable کرتا ہے، rate-limit defaults wire کرتا ہے، اور `direct_mode` table کو plan-derived hostnames اور manifest digests سے populate کرتا ہے۔ Configuration management میں snippet commit کرنے سے پہلے placeholder values کو اپنے rollout plan سے replace کریں۔

## 3. Compliance test suite

Direct-mode readiness اب orchestrator اور CLI crates دونوں میں coverage شامل کرتی ہے:

- `direct_only_policy_rejects_soranet_only_providers` اس بات کو یقینی بناتا ہے کہ `TransportPolicy::DirectOnly` تیزی سے fail کرے جب ہر candidate advert صرف SoraNet relays support کرتا ہو۔【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` یہ یقینی بناتا ہے کہ Torii/QUIC transports موجود ہوں تو انہی کو استعمال کیا جائے اور SoraNet relays کو session سے خارج رکھا جائے۔【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` parse کرتا ہے تاکہ docs helpers کے ساتھ aligned رہیں۔【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` کو mocked Torii gateway کے خلاف چلتا ہے، جو regulated environments کے لیے smoke test فراہم کرتا ہے جہاں direct transports pin ہوتے ہیں۔【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` اسی command کو policy JSON اور scoreboard persistence کے ساتھ wrap کرتا ہے تاکہ rollout automation ہو سکے۔

Updates publish کرنے سے پہلے focused suite چلائیں:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

اگر workspace compilation upstream changes کی وجہ سے fail ہو تو blocking error کو `status.md` میں record کریں اور dependency catch up ہونے پر دوبارہ چلائیں۔

## 4. Automated smoke runs

صرف CLI coverage environment-specific regressions نہیں دکھاتی (مثلاً gateway policy drift یا manifest mismatches)۔ ایک dedicated smoke helper `scripts/sorafs_direct_mode_smoke.sh` میں ہے جو `sorafs_cli fetch` کو direct-mode orchestrator policy، scoreboard persistence، اور summary capture کے ساتھ wrap کرتا ہے۔

Example usage:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- یہ script CLI flags اور key=value config files دونوں کو respect کرتا ہے (دیکھیں `docs/examples/sorafs_direct_mode_smoke.conf`)۔ چلانے سے پہلے manifest digest اور provider advert entries کو production values سے populate کریں۔
- `--policy` default طور پر `docs/examples/sorafs_direct_mode_policy.json` ہے، مگر `sorafs_orchestrator::bindings::config_to_json` سے بننے والا کوئی بھی orchestrator JSON دیا جا سکتا ہے۔ CLI اس policy کو `--orchestrator-config=PATH` کے ذریعے قبول کرتا ہے، جس سے reproducible runs ممکن ہوتی ہیں بغیر flags ہاتھ سے tune کیے۔
- جب `sorafs_cli` `PATH` میں نہ ہو تو helper اسے `sorafs_orchestrator` crate سے (release profile) build کرتا ہے تاکہ smoke runs shipped direct-mode plumbing کو exercise کریں۔
- Outputs:
  - Assembled payload (`--output`, default `artifacts/sorafs_direct_mode/payload.bin`).
  - Fetch summary (`--summary`, default payload کے ساتھ) جس میں telemetry region اور provider reports شامل ہوتے ہیں جو rollout evidence بنتے ہیں۔
  - Scoreboard snapshot policy JSON میں دیے گئے path پر persist ہوتا ہے (مثلاً `fetch_state/direct_mode_scoreboard.json`)۔ اسے summary کے ساتھ change tickets میں archive کریں۔
- Adoption gate automation: fetch مکمل ہونے کے بعد helper `cargo xtask sorafs-adoption-check` چلاتا ہے جس میں persisted scoreboard اور summary paths استعمال ہوتے ہیں۔ Required quorum default طور پر command line پر دیے گئے providers کی تعداد ہے؛ بڑی sample چاہیے تو `--min-providers=<n>` override کریں۔ Adoption reports summary کے ساتھ لکھے جاتے ہیں (`--adoption-report=<path>` custom location دے سکتا ہے) اور helper default طور پر `--require-direct-only` (fallback کے مطابق) اور `--require-telemetry` جب متعلقہ flag دیا جائے، pass کرتا ہے۔ اضافی xtask args کے لیے `XTASK_SORAFS_ADOPTION_FLAGS` استعمال کریں (مثلاً approved downgrade کے دوران `--allow-single-source` تاکہ gate fallback کو tolerate بھی کرے اور enforce بھی)۔ صرف local diagnostics میں `--skip-adoption-check` استعمال کریں؛ roadmap کے مطابق ہر regulated direct-mode run میں adoption report bundle لازمی ہے۔

## 5. Rollout checklist

1. **Configuration freeze:** direct-mode JSON profile کو `iroha_config` repo میں store کریں اور hash کو change ticket میں درج کریں۔
2. **Gateway audit:** direct mode پر switch کرنے سے پہلے تصدیق کریں کہ Torii endpoints TLS، capability TLVs اور audit logging enforce کر رہے ہیں۔ Gateway policy profile کو operators کے لیے publish کریں۔
3. **Compliance sign-off:** updated playbook کو compliance/regulatory reviewers کے ساتھ share کریں اور anonymity overlay سے باہر چلانے کی approvals capture کریں۔
4. **Dry run:** compliance test suite چلائیں اور staging fetch trusted Torii providers کے خلاف کریں۔ Scoreboard outputs اور CLI summaries archive کریں۔
5. **Production cutover:** change window announce کریں، `transport_policy` کو `direct_only` پر flip کریں (اگر `soranet-first` منتخب کیا تھا) اور direct-mode dashboards monitor کریں (`sorafs_fetch` latency، provider failure counters)۔ rollback plan document کریں تاکہ SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` میں graduate ہونے پر SoraNet-first واپس جا سکیں۔
6. **Post-change review:** scoreboard snapshots، fetch summaries اور monitoring results کو change ticket کے ساتھ attach کریں۔ `status.md` میں effective date اور anomalies update کریں۔

Checklist کو `sorafs_node_ops` runbook کے ساتھ رکھیں تاکہ operators live switch سے پہلے workflow rehearse کر سکیں۔ جب SNNet-5 GA تک پہنچے تو production telemetry میں parity confirm کرنے کے بعد fallback retire کریں۔

## 6. Evidence اور adoption gate requirements

Direct-mode captures کو ابھی بھی SF-6c adoption gate satisfy کرنا ہوتا ہے۔ ہر run کے لیے scoreboard، summary، manifest envelope اور adoption report bundle کریں تاکہ `cargo xtask sorafs-adoption-check` fallback posture validate کر سکے۔ Missing fields gate کو fail کرا دیتے ہیں، اس لیے change tickets میں expected metadata record کریں۔

- **Transport metadata:** `scoreboard.json` کو `transport_policy="direct_only"` declare کرنا چاہیے (اور جب downgrade force ہو تو `transport_policy_override=true` flip کریں)۔ Anonymity policy fields کو paired رکھیں چاہے وہ defaults inherit کریں، تاکہ reviewers دیکھ سکیں کہ staged anonymity plan سے deviation ہوا یا نہیں۔
- **Provider counters:** Gateway-only sessions کو `provider_count=0` persist کرنا چاہیے اور `gateway_provider_count=<n>` میں Torii providers کی تعداد populate ہونی چاہیے۔ JSON کو ہاتھ سے edit نہ کریں: CLI/SDK counts derive کرتا ہے اور adoption gate وہ captures reject کرتا ہے جن میں split missing ہو۔
- **Manifest evidence:** جب Torii gateways شامل ہوں تو signed `--gateway-manifest-envelope <path>` (یا SDK equivalent) pass کریں تاکہ `gateway_manifest_provided` کے ساتھ `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json` میں record ہوں۔ یقینی بنائیں کہ `summary.json` میں وہی `manifest_id`/`manifest_cid` موجود ہوں؛ کسی بھی فائل میں pair نہ ہو تو adoption check fail ہو جائے گا۔
- **Telemetry expectations:** جب telemetry capture کے ساتھ ہو تو gate کو `--require-telemetry` کے ساتھ چلائیں تاکہ adoption report metrics emit ہونے کا ثبوت دے۔ Air-gapped rehearsals میں flag omit کیا جا سکتا ہے، مگر CI اور change tickets کو absence document کرنا چاہیے۔

Example:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

`adoption_report.json` کو scoreboard، summary، manifest envelope اور smoke log bundle کے ساتھ attach کریں۔ یہ artifacts CI adoption job (`ci/check_sorafs_orchestrator_adoption.sh`) کی enforcement کو mirror کرتے ہیں اور direct-mode downgrades کو auditable رکھتے ہیں۔
