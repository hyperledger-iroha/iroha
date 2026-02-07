---
lang: dz
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

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

འདི་ མཛོད་ཁང་གི་ནུས་ཤུགས་ CLI ནང་ཡོད་པའི་ Rust crets དང་ ནང་ན་བཙུགས་ཚུགས།
སྲོལ་སྒྲིག་སྒྲ་དབྱངས་ཡང་ན་ཞབས་ཏོག། འོག་གི་ཆ་ཤས་ཚུ་ གྲོགས་རམ་པ་མང་ཤོས་ཅིག་ གསལ་སྟོན་འབདཝ་ཨིན།
གོང་འཕེལ་གཏང་མི་ཚུ་གིས་ ཞུ་དོ་ཡོདཔ་ཨིན།

## བདེན་པའི་རྒྱུན་ལམ་རོགས་སྐྱོར།

ཨེཆ་ཊི་ཊི་པི་ཅིག་ལས་ མེ་ཊིགསི་ཚུ་ བསྡོམས་རྩིས་འབད་ནི་ལུ་ ད་ལྟོ་ཡོད་པའི་བདེན་དཔང་རྒྱུན་ལམ་དབྱེ་དཔྱད་པ་འདི་ ལོག་ལག་ལེན་འཐབ།
ལན་གསལ:

I18NF0000002X

ཐོན་རིམ་ཆ་ཚང་ (བརྟག་དཔྱད་དང་བཅས་) `docs/examples/sorafs_rust_proof_stream.rs` ནང་སྡོད་ཡོད།
`ProofStreamSummary::to_json()` གིས་ CLI དང་འདྲ་བའི་ མེ་ཊིག་ཚུ་ JSON འདི་ བཟོཝ་ཨིན།
བལྟ་རྟོགས་འབད་ཚུགས་པའི་རྒྱབ་རྟེན་ཡང་ན་ CI བདེན་བཤད་ཚུ་ འཇམ་ཏོང་ཏོ་ཨིན།

## ཐོན་ཁུངས་མང་པོའི་ཐོབ་ཐང་སྐུགས་སྐུལ།

I18NI000000007X ཚད་གཞི་འདི་གིས་ དུས་མཉམ་མེན་པའི་ ཕེཆ་གི་དུས་ཚོད་བཀོད་མི་འདི་ གསལ་སྟོན་འབདཝ་ཨིན།
CLI གིས་ལག་ལེན་འཐབ་ཡོདཔ། `sorafs_car::multi_fetch::ScorePolicy` ལག་ལེན་འཐབ་ཞིནམ་ལས་ བརྒྱུད་དེ་འགྱོ་དགོ།
བརྒྱུད་དེ་ `FetchOptions::score_policy` གིས་ བྱིན་མི་བཀའ་རྒྱ་ལུ་ བསྒྱུར་བཅོས་འབདཝ་ཨིན། ཡུ་ནིཊ་བརྟག་དཔྱད།
I18NI000000010X གིས་ བསྟར་སྤྱོད་འབད་ཐངས་སྟོནམ་ཨིན།
སྲོལ་སྒྲིག་དགའ་གདམ་ཚུ།

གཞན་ཡང་ མཛུབ་མོ་མེ་ལོང་ CLI དར་ཚད།

- I18NI000000011X སི་ཨའི་གི་དོན་ལུ་ `--retry-budget` དར་ཆ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
  བསམ་བཞིན་དུ་ retris ཚུ་ བཀག་འཛིན་འབདཝ་ཨིན།
- I18NI000000013X འདི་ I18NI000000014X དང་གཅིག་ཁར་ མཉམ་བསྡོམས་འབད་དེ་ དེ་ མཐོ་ཚད་ལུ་ཨིན།
  དུས་མཉམ་བྱིན་མི་ཚུ་གི་གྲངས།
- I18NI000000015X ངོ་རྟགས་ཚུ།
  I18NI000000016X མེ་ཊིགསི་,
  I18NI000000017X གིས་ སི་ཨེལ་ཨའི་ མེ་ལོང་བཟོཝ་ཨིན།
  `--transport-policy` དར་། I18NI000000019X ད་ལྟ་
  CLI/SDK ཁ་ཐོག་ཚུ་ནང་ སྔོན་སྒྲིག་འདི་; I18NI00000020 1017 རྐྱངམ་ཅིག་ལག་ལེན་འཐབ།
  མར་ཕབ་ཅིག་རྩེད་པའི་སྐབས་ ཡང་ན་ བསྟར་སྤྱོད་བཀོད་རྒྱ་རྗེས་སུ་འབྲངས་པའི་སྐབས་དང་ གསོག་འཇོག་འབད་ནི།
  I18NI000000021X གསལ་ཏོག་ཏོ་སྦེ་ གནང་བ་ཡོད་པའི་ པི་ཀིའུ་རྐྱངམ་གཅིག་གི་ མཁའ་འགྲུལ་པ་ཚུ་གི་དོན་ལུ་ཨིན།
- `སོ་རེསི་གེ་ཊི་ཝེ་ཕེཊ་ཆི་དངོས་པོ་ཚུ་གཞི་སྒྲིག་འབད།::འབྲི་ནི་_མོ་ཌི་_ཧོནཊི་ = གཞི་སྒྲིག་འབད།
  ལ་ལུ་ཅིག་(བྲིས་པའི་མོ་ཌི་ཧིནཊི་::ཨཔ་ཨཔ་པི་ཀིའུ་ཨོན་ལི་)` པི་ཀིའུ་རྐྱངམ་ཅིག་སྐྱེལ་བཙུགས་ཚུ་བཙན་ཤེད་འབད་ནི་ལུ་; རོགས་རམ་འབད་འོང་།
  རང་བཞིན་གྱིས་ སྐྱེལ་འདྲེན་/མིང་མ་བཀོད་པའི་སྲིད་བྱུས་ཚུ་ གསལ་ཏོག་ཏོ་སྦེ་ ཡར་འཕེལ་གཏང་དགོ།
  དབང་བཟུང་འབད་ཡོདཔ།
- གནས་སྐབས་སྐྱེལ་འདྲེན་ཅིག་ འབད་ནིའི་དོན་ལུ་ I18NI0000022X ལག་ལེན་འཐབ།
  ཡང་ན་ ཞུ་བ་གཅིག་གི་དོན་ལུ་ མིང་མ་བཀོད་པའི་རིམ་པ་; ས་སྒོའི་གོམ་པ་ གཉིས་ཀ་བཀྲམ་སྤེལ་འབད་ནི།
  རྒྱ་སྨུག་འདི་ ཉམས་རྒུད་དང་ ཞུ་བ་འབད་མི་ རིམ་པ་དེ་ བསམ་པ་མ་རྫོགས་པའི་སྐབས་ འཐུས་ཤོར་འགྱོཝ་ཨིན།
- པའི་ཐོན་ (I18NI0000023X / I18NI0000024X) དང་།
  ཇ་བ་ཨིསི་ཀིརིཔཊི་ (I18NI0000025X) བཱའིན་ཌིཔ་ཚུ་ དུས་ཚོད་བཀོད་མི་གཅིག་པ་དེ་ ལོག་ལག་ལེན་འཐབ།
  རྩིས་སྟོན་ཡོད་པའི་ལྗིད་ཚད་ཚུ་ ལོག་ཐོབ་ནིའི་དོན་ལུ་ གྲོགས་རམ་འབད་མི་དེ་ཚུ་ནང་ I18NI000000026X གཞི་སྒྲིག་འབད།
  དེ་དང་གཅིག་ཁར་ ཆ་ཤས་ཀྱི་ བྱུང་འཛིན་ཚུ།
- I18NI000000027X OTLP ཐོ་བཀོད་འབདཝ་ཨིན།
  བུ་ལོན་གྱི་ བང་སྒྲིག་ཅིག་བཟོ་མི་ རྒྱུན་རྟོག། བཀོ་བཞག་པའི་སྐབས་ མཁོ་མངགས་འབད་མི་འདི་ལས་ཐོན་འགྱོཝ་ཨིན།
  `region:<telemetry_region>` (ཡང་ན་ `chain:<chain_id>`) རང་བཞིན་གྱིས་ དེ་སྦེ་ མེ་ཊ་ཌེ་ཊ་
  ཨ་རྟག་རང་ འགྲེལ་བཤད་ཀྱི་ཁ་ཡིག་ཅིག་ འབག་འོང་།

## `iroha::Client` བརྒྱུད་དེ་ ཕེཆ་།

Rust SDK གིས་ འཛུལ་སྒོ་ཕེཆ་གྲོགས་རམ་པ་ བསྡུ་སྒྲིག་འབདཝ་ཨིན། གསལ་སྟོན་པ་ དང་ བྱིན་མི་ཅིག་བྱིན།
འགྲེལ་བཤད་པ་ཚུ་ (རྒྱུན་ལམ་བརྡ་མཚོན་ཚུ་རྩིས་ཏེ་) དེ་ལས་ མཁོ་སྤྲོད་པ་གིས་ སྣ་མང་འབྱུང་ཁུངས་འདི་ འདྲེན་འབད་བཅུག།
ལེན་འབག་འོང་ནི:

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

སྐྱེལ་བཙུགས་འབད་བའི་སྐབས་ `transport_policy` ལས་ `Some(TransportPolicy::SoranetStrict)` གཞི་སྒྲིག་འབད།
སྔར་སྲོལ་གྱི་ རི་ལེ་ཚུ་ ངོས་ལེན་མ་འབད་བར་ ཡང་ན་ སོ་ར་ནེཊ་གི་སྐབས་ལུ་ `Some(TransportPolicy::DirectOnly)`
ཡོངས་རྫོགས་སྦེ་ བརྒལ་དགོ། ས་ཚིགས་ I18NI000000034X གསར་བཏོན་ནང་།
ཅ་མཛོད་སྣོད་ཐོ། གདམ་ཁ་ཅན་སྦེ་ I18NI000000035X དང་ མི་རློབས་ཚུ་ བཅོ་ཁ་རྐྱབ།
I18NI000000036X འཛིན་བཟུང་སྐབས་དོན་ (གཏན་བཟོའི་ཁ་ཡིག་ Torii དམིགས་གཏད་ ལ་སོགས་པ་ཚུ་)།
དེ་འབདཝ་ལས་ I18NI000000037X གིས་ ཨེསི་ཌི་ཀེ་ཨེསི་ཚུ་ནང་ ཇེ་ཨེསི་ཨོ་ཨེན་ གཏན་འབེབས་བཟོཝ་ཨིན།
with with bainnance blob SF-6c དང་།
I18NI000000038X ད་ལྟ་ གསལ་སྟོན་དང་བཅས་པའི་མེ་ཊ་ཌེ་ཊ་དེ་ ཡར་སེང་འབདཝ་ཨིན།
ངོས་འཛིན་འབད་མི་, གདམ་ཁ་ཅན་གྱི་ གསལ་སྟོན་སི་ཨའི་ཌི་རེ་བ་, དང་ཨིན།
`gateway_manifest_provided` བཀྲམ་སྤེལ་འབད་ཡོད་པའི་ བརྟག་ཞིབ་ཐོག་ལས་ དར་ཚད།
`GatewayFetchConfig`, དེ་འབདཝ་ལས་ མིང་རྟགས་བཀོད་ཡོད་པའི་ གསལ་སྟོན་ཡིག་ཤུབས་ཅིག་ བསྒྲུབ་ཚུགས་པའི་ འཛིན་བཟུང་འབདཝ་ཨིན།
ལག་ཐོག་ལས་ ས་ཁོངས་དེ་ཚུ་ འདྲ་བཤུས་མ་རྐྱབ་པར་ SF-6c སྒྲུབ་བྱེད་ཀྱི་ དགོས་མཁོ།

## ངོ་མཚར་ཅན་གྱི་རོགས་རམ་པ།

I18NI000000041X འདི་ I18NT0000000X གི་ འབབ་ཁུངས་ཚུ་ བསྡུ་སྒྲིག་འབད་ནི་གི་ ཁྲིམས་ལུགས་ཐབས་ལམ་ཅིག་སྦེ་ ལུས་ཡོདཔ་ཨིན།
ལས་རིམ་གྱི་ཐོག་ལས་:

I18NF0000004X

ཞབས་ཏོག་ཚུ་ ག་སྟེ་ལུ་དགོཔ་ཨིན་ན་ འཕུར་འགྲུལ་འབད་དགོ་སར་ བཟོ་བསྐྲུན་པ་འདི་ བཙུགས་དགོ། ཚིག༌ཕྲད
CLI འདི་ གཏན་འབེབས་ཀྱི་ ཆུ་ལམ་གྱི་དོན་ལུ་ རྒྱབ་སྣོན་ལམ་ཅིག་སྦེ་ ལུས་ཡོདཔ་ཨིན།