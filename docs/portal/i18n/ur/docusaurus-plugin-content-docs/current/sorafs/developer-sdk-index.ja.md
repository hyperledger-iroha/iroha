---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb9f2e9207249360e2bc0a8780753ad259db51328d661d94be8726a8573b726d
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-sdk-index
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

اس hub کو استعمال کریں تاکہ SoraFS toolchain کے ساتھ آنے والے language helpers track ہو سکیں۔
Rust مخصوص snippets کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں۔

## Language helpers

- **Python** — `sorafs_multi_fetch_local` (local orchestrator smoke tests) اور
  `sorafs_gateway_fetch` (gateway E2E exercises) اب optional `telemetry_region` اور
  `transport_policy` override قبول کرتے ہیں
  (`"soranet-first"`, `"soranet-strict"` یا `"direct-only"`)، بالکل CLI rollout knobs کی طرح۔
  جب local QUIC proxy چلتا ہے تو `sorafs_gateway_fetch` browser manifest کو
  `local_proxy_manifest` میں واپس کرتا ہے تاکہ tests trust bundle کو browser adapters تک
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Python helper کو mirror کرتا ہے، payload bytes
  اور receipt summaries واپس کرتا ہے، جبکہ `sorafsGatewayFetch` Torii gateways کو exercise کرتا ہے،
  local proxy manifests کو thread کرتا ہے، اور CLI جیسے telemetry/transport overrides expose کرتا ہے۔
- **Rust** — services scheduler کو براہ راست `sorafs_car::multi_fetch` کے ذریعے embed کر سکتے ہیں؛
  proof-stream helpers اور orchestrator integration کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں۔
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP executor reuse کرتا ہے اور
  `GatewayFetchOptions` کو honour کرتا ہے۔ اسے `ClientConfig.Builder#setSorafsGatewayUri` اور
  PQ upload hint (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) کے ساتھ combine کریں جب uploads
  کو PQ-only paths پر رکھنا ضروری ہو۔

## Scoreboard اور policy knobs

Python (`sorafs_multi_fetch_local`) اور JavaScript (`sorafsMultiFetchLocal`) helpers CLI کے
telemetry-aware scheduler scoreboard کو expose کرتے ہیں:

- Production binaries scoreboard default طور پر enable کرتے ہیں؛ fixtures replay کرتے وقت
  `use_scoreboard=True` (یا `telemetry` entries) دیں تاکہ helper advert metadata اور recent telemetry
  snapshots سے weighted provider ordering derive کرے۔
- `return_scoreboard=True` set کریں تاکہ computed weights chunk receipts کے ساتھ مل سکیں اور CI logs
  diagnostics capture کر سکیں۔
- `deny_providers` یا `boost_providers` arrays استعمال کریں تاکہ peers reject ہوں یا `priority_delta`
  add ہو جب scheduler providers select کرے۔
- Default `"soranet-first"` posture برقرار رکھیں جب تک downgrade stage نہ ہو؛ `"direct-only"` صرف تب دیں
  جب compliance region کو relays سے بچنا ہو یا SNNet-5a fallback rehearsal ہو، اور `"soranet-strict"`
  کو PQ-only pilots کے لیے governance approval کے ساتھ reserve کریں۔
- Gateway helpers `scoreboardOutPath` اور `scoreboardNowUnixSecs` بھی expose کرتے ہیں۔ `scoreboardOutPath`
  set کریں تاکہ computed scoreboard persist ہو (CLI `--scoreboard-out` flag کی طرح) اور
  `cargo xtask sorafs-adoption-check` SDK artifacts validate کر سکے، اور `scoreboardNowUnixSecs` تب دیں جب
  fixtures کو reproducible metadata کے لیے stable `assume_now` value چاہیے ہو۔ JavaScript helper میں
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی set کیے جا سکتے ہیں؛ اگر label omit ہو تو
  وہ `region:<telemetryRegion>` derive کرتا ہے (fallback `sdk:js`). Python helper جب scoreboard persist کرتا ہے تو
  `telemetry_source="sdk:python"` خودکار طور پر emit کرتا ہے اور implicit metadata کو disabled رکھتا ہے۔

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```
