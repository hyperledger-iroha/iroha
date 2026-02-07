---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4fb8761802761aad2b91202fbb11136734036d46c0245814616492ad90b12258
source_last_modified: "2026-01-05T09:28:11.869572+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-rust
title: Rust SDK Snippets
sidebar_label: Rust snippets
description: Minimal Rust examples for consuming proof streams and manifests.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

ဤသိုလှောင်ခန်းရှိ သံချေးသေတ္တာများသည် CLI ကို စွမ်းအားရှိပြီး အတွင်းတွင် ထည့်သွင်းနိုင်သည်။
စိတ်ကြိုက်တီးမှုတ်သူများ သို့မဟုတ် ဝန်ဆောင်မှုများ။ အောက်ဖော်ပြပါ အတိုအထွာများသည် ကူညီပေးသူများကို အများဆုံး မီးမောင်းထိုးပြပါသည်။
developer များကတောင်းဆိုသည်။

## အထောက်အထားစီးကြောင်းအကူအညီပေးသူ

HTTP တစ်ခုမှ မက်ထရစ်များကို စုစည်းရန် ရှိပြီးသား အထောက်အထား stream parser ကို ပြန်သုံးပါ။
တုံ့ပြန်မှု-

```rust
use std::error::Error;
use std::io::{BufRead, BufReader};

use reqwest::blocking::Response;
use sorafs_car::proof_stream::{ProofStreamItem, ProofStreamMetrics, ProofStreamSummary};

/// Consume an NDJSON proof stream and return aggregated metrics.
pub fn collect_proof_metrics(response: Response) -> Result<ProofStreamSummary, Box<dyn Error>> {
    if !response.status().is_success() {
        return Err(format!("gateway returned {}", response.status()).into());
    }

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut metrics = ProofStreamMetrics::default();
    let mut failures = Vec::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        let item = ProofStreamItem::from_ndjson(trimmed.as_bytes())?;
        if item.status.is_failure() && failures.len() < 5 {
            failures.push(item.clone());
        }
        metrics.record(&item);
        line.clear();
    }

    Ok(ProofStreamSummary::new(metrics, failures))
}
```

ဗားရှင်းအပြည့်အစုံ (စမ်းသပ်မှုများနှင့်အတူ) သည် `docs/examples/sorafs_rust_proof_stream.rs` တွင်နေထိုင်သည်။
`ProofStreamSummary::to_json()` သည် CLI ကဲ့သို့ တူညီသော မက်ထရစ်များကို JSON ထုတ်ပေးသည်၊
ကြည့်ရှုနိုင်မှု နောက်ခံများ သို့မဟုတ် CI အတည်ပြုချက်များကို ကျွေးမွေးရန် လွယ်ကူသည်။

## ရင်းမြစ်ပေါင်းစုံ အကျိူးဆောင် အမှတ်ပေး

`sorafs_car::multi_fetch` module သည် asynchronous fetch scheduler ကို ဖော်ထုတ်ပေးသည်
CLI မှအသုံးပြုသည်။ `sorafs_car::multi_fetch::ScorePolicy` ကို အကောင်အထည်ဖော်ပြီး ကျော်သွားပါ။
ဝန်ဆောင်မှုပေးသူ အော်ဒါမှာမှုကို ချိန်ညှိရန် `FetchOptions::score_policy` မှတဆင့်။ ယူနစ်စမ်းသပ်မှု
`multi_fetch::tests::score_policy_can_filter_providers` မည်ကဲ့သို့ ပြဋ္ဌာန်းရမည်ကို ပြသည်။
စိတ်ကြိုက်ရွေးချယ်မှုများ။

အခြားအဖုများသည် CLI အလံများကို ထင်ဟပ်သည်-

- `FetchOptions::per_chunk_retry_limit` သည် CI အတွက် `--retry-budget` အလံနှင့် ကိုက်ညီသည်
  ကြိုးစားခြင်းကို ရည်ရွယ်ချက်ရှိရှိ ကန့်သတ်ထားသော လုပ်ဆောင်သည်။
- ထုပ်ရန် `FetchOptions::global_parallel_limit` ကို `--max-peers` နှင့် ပေါင်းစပ်ပါ။
  တစ်ပြိုင်တည်းပံ့ပိုးပေးသူအရေအတွက်။
- `OrchestratorConfig::with_telemetry_region("region")` ကို တဂ်သည်။
  `sorafs_orchestrator_*` မက်ထရစ်များ၊
  `OrchestratorConfig::with_transport_policy` သည် CLI ကို ထင်ဟပ်စေသည်။
  `--transport-policy` အလံ။ `TransportPolicy::SoranetPreferred` သည် ယခုအတိုင်း တင်ပို့လိုက်ပြီဖြစ်သည်။
  CLI/SDK မျက်နှာပြင်များတစ်လျှောက် ပုံသေ၊ `TransportPolicy::DirectOnly` ကိုသာအသုံးပြုပါ။
  အဆင့်နှိမ့်ချခြင်း သို့မဟုတ် လိုက်နာမှုဆိုင်ရာ ညွှန်ကြားချက်ကို လိုက်နာသည့်အခါတွင်၊
  ပြတ်သားစွာအတည်ပြုချက်ဖြင့် PQ-သီးသန့်လေယာဉ်မှူးများအတွက် `SoranetStrict`။
- `SorafsGatewayFetchOptions::write_mode_hint= သတ်မှတ်ပါ။
  PQ-သီးသန့် အပ်လုဒ်များကို အတင်းအကြပ်လုပ်ရန် အချို့(WriteModeHint::UploadPqOnly)`။ အကူအညီပေးလိမ့်မည်။
  သယ်ယူပို့ဆောင်ရေး/အမည်ဝှက်ခြင်းမူဝါဒများကို ပြတ်သားစွာမဖော်ပြပါက အလိုအလျောက် မြှင့်တင်ပါ။
  လွမ်းမိုး။
- ယာယီသယ်ယူပို့ဆောင်ရေးကို ပင်ထိုးရန် `SorafsGatewayFetchOptions::policy_override` ကိုသုံးပါ။
  တောင်းဆိုချက်တစ်ခုအတွက် သို့မဟုတ် အမည်ဝှက်အဆင့်၊ ပံ့ပိုးပေးသော နယ်ပယ်တစ်ခုခုကို ကျော်သွားသည်
  တောင်းဆိုထားသောအဆင့်ကို မကျေနပ်နိုင်သောအခါတွင် အညိုရောင်ပြောင်းခြင်းအား ဖယ်ရှားခြင်း မအောင်မြင်ပါ။
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) နှင့်
  JavaScript (`sorafsMultiFetchLocal`) bindings များသည် တူညီသော အချိန်ဇယားကို ပြန်လည်အသုံးပြုသောကြောင့်၊
  တွက်ချက်ထားသောအလေးချိန်များကိုရယူရန် ထိုအကူအညီပေးသူများတွင် `return_scoreboard=true` ကိုသတ်မှတ်ပါ။
  အတုံးလိုက်ဖြတ်ပိုင်းများနှင့်အတူ။
- `SorafsGatewayScoreboardOptions::telemetry_source_label` သည် OTLP ကို မှတ်တမ်းတင်သည်။
  မွေးစားခြင်းအစုအဝေးကို ထုတ်လုပ်သည့် stream ။ ချန်လှပ်ထားသောအခါတွင်၊ ဖောက်သည်က ရရှိသည်။
  `region:<telemetry_region>` (သို့မဟုတ် `chain:<chain_id>`) အလိုအလျောက် မက်တာဒေတာ၊
  သရုပ်ဖော်အညွှန်းကို အမြဲဆောင်ထားပါ။

## `iroha::Client` မှတဆင့် ရယူပါ။

Rust SDK သည် gateway fetch helper ကို စုစည်းထားသည်။ မန်နီးဖက်စ် အပေါင်း ပံ့ပိုးပေးသူကို ပေးပါ။
ဖော်ပြချက်ပေးသူများ (စီးကြောင်းတိုကင်များ အပါအဝင်) နှင့် client သည် multi-source ကို မောင်းနှင်ခွင့်ပြုပါ။
ထုတ်ယူမှု-

```rust
use eyre::Result;
use iroha::{
    Client,
    client::{SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions},
};
use sorafs_car::CarBuildPlan;
use sorafs_orchestrator::{
    AnonymityPolicy, PolicyOverride,
    prelude::{GatewayFetchConfig, GatewayProviderInput, TransportPolicy},
};
use std::path::PathBuf;

pub async fn fetch_payload(
    client: &Client,
    plan: &CarBuildPlan,
    gateway: GatewayFetchConfig,
    providers: Vec<GatewayProviderInput>,
) -> Result<Vec<u8>> {
    let options = SorafsGatewayFetchOptions {
        transport_policy: Some(TransportPolicy::SoranetPreferred),
        // Pin Stage C for this fetch; omit `policy_override` to apply staged defaults.
        policy_override: PolicyOverride::new(
            Some(TransportPolicy::SoranetStrict),
            Some(AnonymityPolicy::StrictPq),
        ),
        write_mode_hint: None,
        scoreboard: Some(SorafsGatewayScoreboardOptions {
            persist_path: Some(
                PathBuf::from("artifacts/sorafs_orchestrator/latest/scoreboard.json"),
            ),
            now_unix_secs: None,
            metadata: Some(norito::json!({
                "capture_id": "sdk-smoke-run",
                "fixture": "multi_peer_parity_v1"
            })),
            telemetry_source_label: Some("otel::staging".into()),
        }),
        ..SorafsGatewayFetchOptions::default()
    };
    let outcome = client
        .sorafs_fetch_via_gateway(plan, gateway, providers, options)
        .await?;
    Ok(outcome.assemble_payload())
}
```

အပ်လုဒ်လုပ်သည့်အခါ `transport_policy` ကို `Some(TransportPolicy::SoranetStrict)` သို့ သတ်မှတ်ပါ
ရှေးရိုး relay များကို ငြင်းဆိုရမည် သို့မဟုတ် `Some(TransportPolicy::DirectOnly)` သည် SoraNet ဖြစ်သောအခါ
လုံးဝကျော်ဖြတ်ရမယ်။ ထုတ်ဝေမှုတွင် အမှတ် `scoreboard.persist_path`
artefact directory၊ optionally `scoreboard.now_unix_secs` ကို fix လုပ်ပြီးဖြည့်ပါ။
ဖမ်းယူဆက်စပ်မှုပါရှိသော `scoreboard.metadata` (တပ်ဆင်မှုအညွှန်းများ၊ Torii ပစ်မှတ် စသည်ဖြင့်)
ထို့ကြောင့် `cargo xtask sorafs-adoption-check` သည် SDK များတစ်လျှောက် အဆုံးအဖြတ် JSON ကိုစားသုံးသည်။
provenance blob SF-6c နှင့် မျှော်လင့်ထားသည်။
ယခု `Client::sorafs_fetch_via_gateway` သည် အဆိုပါ မက်တာဒေတာကို မန်နီးဖက်စ်ဖြင့် တိုးမြှင့်ထားသည်။
သတ်မှတ်ချက်၊ ရွေးချယ်နိုင်သော သရုပ်ဖော်ပြချက် CID မျှော်လင့်ချက် နှင့်
`gateway_manifest_provided` အလံတို့ကို ကြည့်ရှုစစ်ဆေးပြီး ထောက်ပံ့ပေးခဲ့သည်။
`GatewayFetchConfig`၊ ထို့ကြောင့် လက်မှတ်ရေးထိုးထားသော မန်နီးဖက်စ်စာအိတ်ပါ၀င်သည့် ဖမ်းယူမှုများကို ကျေနပ်စေပါသည်
ထိုနယ်ပယ်များကို ကိုယ်တိုင်မပွားဘဲ SF-6c အထောက်အထား လိုအပ်ချက်။

## အထောက်အပံများကို ဖော်ပြပါ။

`ManifestBuilder` သည် Norito payloads များကို စုစည်းရန် canonical way ဖြစ်သည်
ပရိုဂရမ်အရ-

```rust
use sorafs_manifest::{ManifestBuilder, ManifestV1, PinPolicy, StorageClass};

fn build_manifest(bytes: &[u8]) -> Result<ManifestV1, Box<dyn std::error::Error>> {
    let mut builder = ManifestBuilder::new();
    builder.pin_policy(PinPolicy {
        min_streams: 3,
        storage_class: StorageClass::Warm,
        retention_epoch: Some(48),
    });
    builder.payload(bytes)?;
    Ok(builder.build()?)
}
```

ဝန်ဆောင်မှုများ အလျင်အမြန်ထုတ်လုပ်ရန် လိုအပ်သည့်နေရာတိုင်းတွင် တည်ဆောက်သူကို မြှုပ်နှံပါ။ အဆိုပါ
CLI သည် အဆုံးအဖြတ်ပေးသော ပိုက်လိုင်းများအတွက် အကြံပြုထားသော လမ်းကြောင်းဖြစ်နေဆဲဖြစ်သည်။