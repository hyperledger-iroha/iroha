---
id: developer-sdk-rust
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust SDK Snippets
sidebar_label: Rust snippets
description: Minimal Rust examples for consuming proof streams and manifests.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

Rust ამ საცავში აძლიერებს CLI-ს და შეიძლება ჩამონტაჟდეს შიგნით
საბაჟო ორკესტრატორები ან სერვისები. ქვემოთ მოყვანილი ფრაგმენტები ყველაზე მეტად ხაზს უსვამს დამხმარეებს
დეველოპერები ითხოვენ.

## მტკიცებულება ნაკადის დამხმარე

ხელახლა გამოიყენეთ არსებული მტკიცებულების ნაკადის პარსერი HTTP-დან მეტრიკის აგრეგაციისთვის
პასუხი:

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

სრული ვერსია (ტესტებით) ცხოვრობს `docs/examples/sorafs_rust_proof_stream.rs`-ში.
`ProofStreamSummary::to_json()` გამოაქვს იგივე მეტრიკა JSON, როგორც CLI, რაც
ადვილია დაკვირვებადობის უკანა ნაწილის ან CI მტკიცებების შესანახი.

## მრავალ წყაროს მოპოვების ქულა

`sorafs_car::multi_fetch` მოდული ამჟღავნებს ასინქრონული მიღების გრაფიკს
გამოიყენება CLI-ს მიერ. დანერგე `sorafs_car::multi_fetch::ScorePolicy` და გაიარე
`FetchOptions::score_policy`-ის საშუალებით პროვაიდერის შეკვეთის დასარეგულირებლად. ერთეულის ტესტი
`multi_fetch::tests::score_policy_can_filter_providers` გვიჩვენებს, თუ როგორ უნდა აღსრულდეს
მორგებული პრეფერენციები.

სხვა სახელურები ასახავს CLI დროშებს:

- `FetchOptions::per_chunk_retry_limit` ემთხვევა `--retry-budget` დროშას CI-სთვის
  გაშვებები, რომლებიც განზრახ ზღუდავს განმეორებით ცდებს.
- შეუთავსეთ `FetchOptions::global_parallel_limit` `--max-peers`-თან, რომ დაიხუროს
  თანმხლები პროვაიდერების რაოდენობა.
- `OrchestratorConfig::with_telemetry_region("region")` იტირებს
  `sorafs_orchestrator_*` მეტრიკა, ხოლო
  `OrchestratorConfig::with_transport_policy` ასახავს CLI-ს
  `--transport-policy` დროშა. `TransportPolicy::SoranetPreferred` ახლა იგზავნება როგორც
  ნაგულისხმევი CLI/SDK ზედაპირებზე; გამოიყენეთ მხოლოდ `TransportPolicy::DirectOnly`
  რეიტინგის დაქვეითების ან შესაბამისობის დირექტივის შესრულებისას და რეზერვი
  `SoranetStrict` მხოლოდ PQ პილოტებისთვის აშკარა დამტკიცებით.
- დააყენეთ `SorafsGatewayFetchOptions::write_mode_hint =
  ზოგიერთი (WriteModeHint::UploadPqOnly)` მხოლოდ PQ ატვირთვის იძულებით; დამხმარე იქნება
  ავტომატურად შეუწყოს ხელი სატრანსპორტო/ანონიმურობის პოლიტიკას, გარდა იმ შემთხვევებისა, როცა ცალსახა
  გადააჭარბა.
- გამოიყენეთ `SorafsGatewayFetchOptions::policy_override` დროებითი ტრანსპორტის დასამაგრებლად
  ან ანონიმურობის დონე ერთი მოთხოვნისთვის; მიწოდების ორივე სფეროში გამოტოვებს
  დაქვეითება და მარცხი, როდესაც მოთხოვნილი დონე ვერ დაკმაყოფილდება.
- პითონი (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) და
  JavaScript (`sorafsMultiFetchLocal`) აკინძები ხელახლა იყენებს იმავე განრიგს, ასე რომ
  დააყენეთ `return_scoreboard=true` იმ დამხმარეებში გამოთვლილი წონების მოსაპოვებლად
  ქვითრებთან ერთად.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` ჩაწერს OTLP-ს
  ნაკადი, რომელმაც შექმნა შვილად აყვანის პაკეტი. როდესაც გამოტოვებულია, კლიენტი წარმოიქმნება
  `region:<telemetry_region>` (ან `chain:<chain_id>`) ავტომატურად ასე მეტამონაცემები
  ყოველთვის ატარებს აღწერილ ეტიკეტს.

## მიღება `iroha::Client`-ით

Rust SDK აერთიანებს კარიბჭის მოპოვების დამხმარეს; მოგვაწოდეთ მანიფესტის პლუს პროვაიდერი
დესკრიპტორები (სტრიმინგის ტოკენების ჩათვლით) და მიეცით საშუალება კლიენტს მართოს მრავალ წყარო
მოტანა:

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

დააყენეთ `transport_policy`-ზე `Some(TransportPolicy::SoranetStrict)` ატვირთვისას
უარი უნდა თქვას კლასიკურ რელეებზე, ან `Some(TransportPolicy::DirectOnly)` როდესაც SoraNet
მთლიანად უნდა იყოს გვერდის ავლით. გამოშვებისას მიუთითეთ `scoreboard.persist_path`
არტეფაქტის დირექტორია, სურვილისამებრ შეასწორეთ `scoreboard.now_unix_secs` და შეავსეთ
`scoreboard.metadata` გადაღების კონტექსტით (ფიქსირების ეტიკეტები, Torii სამიზნე და ა.შ.)
ასე რომ, `cargo xtask sorafs-adoption-check` მოიხმარს დეტერმინისტულ JSON-ს SDK-ებში
ერთად წარმოშობის blob SF-6c მოელის.
`Client::sorafs_fetch_via_gateway` ახლა აძლიერებს ამ მეტამონაცემებს manifest-თან ერთად
იდენტიფიკატორი, სურვილისამებრ მანიფესტი CID მოლოდინი და
`gateway_manifest_provided` დროშა მოწოდებულის შემოწმებით
`GatewayFetchConfig`, ასე რომ, გადაღებები, რომლებიც მოიცავს ხელმოწერილ მანიფესტის კონვერტს, დააკმაყოფილებს
SF-6c მტკიცებულების მოთხოვნა ამ ველების ხელით დუბლირების გარეშე.

## მანიფესტ დამხმარეები

`ManifestBuilder` რჩება Norito ტვირთამწეობის აწყობის კანონიკურ გზად
პროგრამულად:

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

მშენებლის ჩაშენება იქ, სადაც სერვისებს სჭირდებათ მანიფესტების გენერირება; The
CLI რჩება რეკომენდებული გზა დეტერმინისტული მილსადენებისთვის.