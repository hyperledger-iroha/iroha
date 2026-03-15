<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: he
direction: rtl
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_sdk_quickstarts.md -->

# התחלה מהירה ל-SDK של Nexus

**קישור למפת דרכים:** NX-14 - תיעוד Nexus ו-runbooks למפעילים  
**סטטוס:** טיוטה 2026-03-24  
**קהל יעד:** צוותי SDK ומפתחים שותפים שמתחברים לרשת Sora Nexus (Iroha 3).

המדריך הזה מתעד את הצעדים המינימליים הנדרשים כדי לכוון כל SDK ראשון-צד אל הרשת הציבורית של
Nexus. הוא משלים את מדריכי ה-SDK המפורטים תחת `docs/source/sdk/*` ואת runbook ההפעלה
(`docs/source/nexus_operations.md`).

## דרישות מקדימות משותפות

- התקינו את גרסאות ה-toolchain שמוגדרות ב-`rust-toolchain.toml` וב-`package.json`/`Package.swift`
  עבור ה-SDK שאתם בודקים.
- הורידו את חבילת התצורה של Nexus (ראו `docs/source/sora_nexus_operator_onboarding.md`) כדי לקבל
  נקודות קצה של Torii, תעודות TLS וקטלוג נתיבי settlement.
- יצאו את משתני הסביבה הבאים (התאימו לסביבה שלכם):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

ערכים אלה מוזנים אל בלוקי התצורה הספציפיים ל-SDK בהמשך.

## SDK Rust (`iroha_client`)

```rust
use iroha_client::client::{Client, ClientConfiguration};
use iroha_core::crypto::KeyPair;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keypair = KeyPair::from_hex("ed0120...")?;
    let config = ClientConfiguration::with_url(NEXUS_TORII_URL.parse()?)
        .with_chain_id(NEXUS_CHAIN_ID.parse()?)
        .with_pipeline_url(NEXUS_PIPELINE_URL.parse()?)
        .with_tls("nexus_ca.pem")
        .with_trusted_peers(vec![NEXUS_TRUSTED_PUBKEY.parse()?]);
    let client = Client::new(config, keypair)?;
    let status = client.get_network_status()?;
    println!("Latest block: {}", status.latest_block_height);
    Ok(())
}
```

- בניה/בדיקות: `cargo test -p iroha_client -- --nocapture`
- סקריפט דמו: `cargo run --bin nexus_quickstart --features sdk`
- תיעוד: `docs/source/sdk/rust.md`

### עוזר זמינות נתונים (DA-8)

```rust
use iroha_client::client::{Client, ClientConfiguration};
use iroha_client::da::DaIngestParams;
use iroha_data_model::da::types::ExtraMetadata;
use eyre::Result;

fn persist_da_payload(client: &Client, payload: Vec<u8>, storage_ticket: &str) -> Result<()> {
    client
        .submit_da_blob(payload, &DaIngestParams::default(), ExtraMetadata::default(), None)?;
    let persisted = client.fetch_da_manifest_to_dir(storage_ticket, "artifacts/da")?;
    println!(
        "manifest saved to {} / chunk plan {}",
        persisted.manifest_raw.display(),
        persisted.chunk_plan.display()
    );
    Ok(())
}
```

- תיעוד: `docs/source/sdk/rust.md`

### עוזר QR של Explorer (ADDR-6b)

```rust
use iroha_client::client::{
    Client,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "i105...",
    )?;
    println!("I105 literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

האובייקט המוחזר `ExplorerAccountQrSnapshot` כולל מזהה חשבון קנוני, literal מבוקש, הגדרות תיקון
שגיאות ומטען SVG מוטמע המשמש בזרימות שיתוף של wallet/explorer.

## JavaScript / TypeScript (`@iroha/iroha-js`)

```ts
import { ToriiClient } from '@iroha/iroha-js';

const client = new ToriiClient({
  baseUrl: process.env.NEXUS_TORII_URL!,
  pipelineUrl: process.env.NEXUS_PIPELINE_URL!,
  chainId: process.env.NEXUS_CHAIN_ID!,
  trustedPeers: [process.env.NEXUS_TRUSTED_PUBKEY!],
});

(async () => {
  const status = await client.fetchStatus();
  console.log(`Latest block: ${status.latestBlock.height}`);
})();
```

- התקנת תלויות: `npm ci`
- הרצת דוגמה: `NEXUS_TORII_URL=... npm run demo:nexus`
- תיעוד: `docs/source/sdk/js/quickstart.md` (שינוי שם מתוכנן כדי להתיישר עם המסמך הזה)

## Swift (`IrohaSwift`)

```swift
import IrohaSwift

let config = Torii.Configuration(
    toriiURL: URL(string: ProcessInfo.processInfo.environment["NEXUS_TORII_URL"]!)!,
    pipelineURL: URL(string: ProcessInfo.processInfo.environment["NEXUS_PIPELINE_URL"]!)!,
    chainId: ProcessInfo.processInfo.environment["NEXUS_CHAIN_ID"]!,
    trustedPeers: [ProcessInfo.processInfo.environment["NEXUS_TRUSTED_PUBKEY"]!],
    tls: .cafile(path: "nexus_ca.pem")
)
let client = try Torii.Client(configuration: config, keyPair: KeyPair.randomEd25519())
let status = try client.getNetworkStatus().get()
print("Latest block \(status.latestBlock.height)")
```

- בניה/בדיקות: `swift test`
- הרצת דמו: `make swift-nexus-demo`
- תיעוד: `docs/source/sdk/swift/index.md`

## Android (`iroha-android`)

```kotlin
val config = ClientConfig(
    toriiUrl = NEXUS_TORII_URL,
    pipelineUrl = NEXUS_PIPELINE_URL,
    chainId = NEXUS_CHAIN_ID,
    trustedPeers = listOf(NEXUS_TRUSTED_PUBKEY),
    tls = ClientConfig.Tls.CertFile("nexus_ca.pem")
)
val client = NoritoRpcClient(config, defaultDispatcher)
runBlocking {
    val status = client.fetchStatus()
    println("Latest block ${status.latestBlock.height}")
}
```

- בניה/בדיקות: `./gradlew :iroha-android:connectedDebugAndroidTest`
- אפליקצית דמו: `examples/android/retail-wallet` (עדכנו את `.env` עם ערכי Nexus)
- תיעוד: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

הפקודה שולחת שאילתת `FindNetworkStatus` ומדפיסה את התוצאה. לנתיבי כתיבה, העבירו ארגומנטים
`--lane-id`/`--dataspace-id` שתואמים לקטלוג ה-lanes.

## מטריצת בדיקות

| רכיב | פקודה | צפוי |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | פונה ל-endpoint staging של Nexus ומדפיסה את גובה הבלוק. |
| JS/TS | `npm run test:nexus` | בדיקת Jest שמוודאת ש-URL של Torii ו-pipeline עובדים. |
| Swift | `swift test --filter NexusQuickstartTests` | סימולטור iOS מאחזר סטטוס. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | מכשיר מנוהל ניגש ל-staging. |
| CLI | `iroha_cli app nexus quickstart --dry-run` | מאמת קונפיגורציה לפני קריאות רשת. |

## פתרון תקלות

- **שגיאות TLS/CA** - ודאו שחבילת ה-CA שמגיעה עם גרסת Nexus זמינה, ואשרו שה-`toriiUrl` משתמש ב-HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** - השהיית ממשל ברמת lane; בדקו
  `docs/source/nexus_operations.md` לתהליך תקריות.
- **`ERR_UNKNOWN_LANE`** - עדכנו את ה-SDK כדי להעביר `lane_id`/`dataspace_id` במפורש כאשר admission
  מרובה lanes נאכפת.

דווחו על באגים דרך GitHub issues עם התג `NX-14` וציינו את ה-SDK והפקודה כדי שהמתחזקים יוכלו
לשחזר במהירות.

</div>
