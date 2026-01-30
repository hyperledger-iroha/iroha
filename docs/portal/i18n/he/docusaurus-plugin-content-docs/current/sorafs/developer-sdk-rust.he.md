---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5afe34fb14a5172b742d98532f48f80541c4bdf4f276242511e099a897412858
source_last_modified: "2025-11-14T04:43:21.684475+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/sdk/rust.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של Sphinx יופסק.
:::

ה-crates של Rust במאגר זה מניעים את ה-CLI וניתנים להטמעה בתוך orchestrators או שירותים
מותאמים. המקטעים למטה מדגישים את ה-helpers שהמפתחים מבקשים הכי הרבה.

## Helper ל-proof stream

השתמשו שוב ב-parser הקיים של proof stream כדי לאגד מדדים מתגובה HTTP:

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

הגרסה המלאה (כולל בדיקות) נמצאת ב-`docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` מפיק את אותו JSON מדדים כמו ה-CLI, כך שקל להזין
מערכות observability או assertions של CI.

## ניקוד fetch רב-מקור

המודול `sorafs_car::multi_fetch` חושף את מתזמן ה-fetch האסינכרוני שמשמש את ה-CLI.
יישמו `sorafs_car::multi_fetch::ScorePolicy` והעבירו אותו דרך
`FetchOptions::score_policy` כדי לכוונן את סדר הספקים. בדיקת היחידה
`multi_fetch::tests::score_policy_can_filter_providers` מראה כיצד לאכוף
העדפות מותאמות.

כפתורים נוספים משקפים דגלי CLI:

- `FetchOptions::per_chunk_retry_limit` תואם לדגל `--retry-budget` לריצות CI שמגבילות
  ניסיונות בכוונה.
- שלבו `FetchOptions::global_parallel_limit` עם `--max-peers` כדי להגביל את מספר
  הספקים במקביל.
- `OrchestratorConfig::with_telemetry_region("region")` מתייג את מדדי
  `sorafs_orchestrator_*`, בעוד `OrchestratorConfig::with_transport_policy`
  משקף את דגל ה-CLI `--transport-policy`. `TransportPolicy::SoranetPreferred`
  מגיע כברירת מחדל בכל המשטחים CLI/SDK; השתמשו ב-`TransportPolicy::DirectOnly`
  רק בעת הכנת downgrade או בהתאם להנחיית compliance, ושמרו את `SoranetStrict`
  לפיילוטים PQ-only עם אישור מפורש.
- הגדירו `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` כדי לכפות uploads ב-PQ-only; ה-helper יקדם
  אוטומטית את מדיניות ה-transport/anonymity אלא אם overridden במפורש.
- השתמשו ב-`SorafsGatewayFetchOptions::policy_override` כדי להצמיד tier זמני של
  transport או anonymity לבקשה יחידה; אספקת אחד השדות מדלגת על brownout demotion
  ונכשלת אם tier המבוקש אינו יכול להתממש.
- ה-bindings של Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) ושל
  JavaScript (`sorafsMultiFetchLocal`) משתמשים באותו scheduler, לכן הגדירו
  `return_scoreboard=true` באותם helpers כדי לקבל את המשקלים המחושבים לצד
  קבלות chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` רושם את זרם ה-OTLP
  שהפיק adoption bundle. כאשר הוא מושמט, הלקוח גוזר אוטומטית
  `region:<telemetry_region>` (או `chain:<chain_id>`) כדי שהמטאדאטה תמיד תישא תווית
  תיאורית.

## Fetch דרך `iroha::Client`

ה-SDK של Rust כולל את helper ה-gateway fetch; ספקו manifest יחד עם descriptors של
providers (כולל stream tokens) ותנו ל-client להניע את ה-fetch הרב-מקורי:

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

הגדירו `transport_policy` ל-`Some(TransportPolicy::SoranetStrict)` כאשר uploads חייבים
לסרב ל-relays קלאסיים, או ל-`Some(TransportPolicy::DirectOnly)` כאשר חייבים לעקוף את
SoraNet לחלוטין. כוונו `scoreboard.persist_path` לתיקיית ארטיפקטי release, אפשר
לקבע `scoreboard.now_unix_secs`, ומלאו את `scoreboard.metadata` בהקשר לכידה (labels
של fixtures, יעד Torii, וכו') כדי ש-`cargo xtask sorafs-adoption-check` יצרוך JSON
דטרמיניסטי בין SDKs עם blob provenance שמצופה ב-SF-6c.
`Client::sorafs_fetch_via_gateway` מוסיף כעת למטאדאטה הזו את מזהה ה-manifest,
ציפיית manifest CID אופציונלית ואת הדגל `gateway_manifest_provided` על ידי
בחינת ה-`GatewayFetchConfig` שסופק, כך שלכידות הכוללות מעטפת manifest חתומה
עומדות בדרישת ראיות SF-6c בלי לשכפל את השדות ידנית.

## Helpers של manifest

`ManifestBuilder` נשאר הדרך הקנונית להרכבת payloads של Norito באופן פרוגרמטי:

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

שלבו את ה-builder בכל מקום שבו שירותים צריכים לייצר manifests על הדרך; ה-CLI
נשאר המסלול המומלץ לצינורות דטרמיניסטיים.
