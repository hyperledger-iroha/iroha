<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ar
direction: rtl
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_sdk_quickstarts.md -->

# ادلة البدء السريع لحزم SDK في Nexus

**رابط خارطة الطريق:** NX-14 - توثيق Nexus وrunbooks للمشغلين  
**الحالة:** مسودة 2026-03-24  
**الجمهور:** فرق SDK والمطورون الشركاء المتصلون بشبكة Sora Nexus (Iroha 3).

يوثق هذا الدليل الحد الادنى من الخطوات المطلوبة لتوجيه كل SDK من الطرف الاول نحو الشبكة العامة
لـ Nexus. وهو يكمل ادلة SDK التفصيلية ضمن `docs/source/sdk/*` ودليل التشغيل
(`docs/source/nexus_operations.md`).

## المتطلبات المشتركة

- ثبت نسخ toolchain المثبتة في `rust-toolchain.toml` و`package.json`/`Package.swift` للـ SDK الذي تختبره.
- حمل حزمة اعداد Nexus (انظر `docs/source/sora_nexus_operator_onboarding.md`) للحصول على نقاط نهاية
  Torii وشهادات TLS وكتالوج مسارات settlement.
- صدر متغيرات البيئة التالية (عدلها حسب بيئتك):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

تغذي هذه القيم كتل الضبط الخاصة بكل SDK ادناه.

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

- البناء/الاختبار: `cargo test -p iroha_client -- --nocapture`
- سكربت تجريبي: `cargo run --bin nexus_quickstart --features sdk`
- الوثائق: `docs/source/sdk/rust.md`

### مساعد توفر البيانات (DA-8)

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

- الوثائق: `docs/source/sdk/rust.md`

### مساعد QR للمستكشف (ADDR-6b)

```rust
use iroha_client::client::{
    AddressFormat, Client, ExplorerAccountQrOptions,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "ih58...",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("IH58 (preferred)/snx1 (second-best) literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

تتضمن القيمة المعادة `ExplorerAccountQrSnapshot` معرف الحساب القانوني والـ literal المطلوب
واعدادات تصحيح الاخطاء وحمولة SVG المضمنة المستخدمة في تدفقات مشاركة wallet/explorer.

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

- تثبيت التبعيات: `npm ci`
- تشغيل المثال: `NEXUS_TORII_URL=... npm run demo:nexus`
- الوثائق: `docs/source/sdk/js/quickstart.md` (اعادة تسمية قادمة لتتوافق مع هذا الدليل)

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

- البناء/الاختبارات: `swift test`
- ادوات العرض التجريبي: `make swift-nexus-demo`
- الوثائق: `docs/source/sdk/swift/index.md`

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

- البناء/الاختبارات: `./gradlew :iroha-android:connectedDebugAndroidTest`
- تطبيق تجريبي: `examples/android/retail-wallet` (حدث `.env` بقيم Nexus)
- الوثائق: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

يرسل الامر استعلام `FindNetworkStatus` ويطبع النتيجة. لمسارات الكتابة، مرر وسيطات
`--lane-id`/`--dataspace-id` المطابقة لكتالوج المسارات.

## مصفوفة الاختبار

| السطح | الامر | المتوقع |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | يصل الى endpoint staging لـ Nexus ويطبع ارتفاع الكتلة. |
| JS/TS | `npm run test:nexus` | اختبار Jest يؤكد ان عناوين Torii وpipeline تعمل. |
| Swift | `swift test --filter NexusQuickstartTests` | محاكي iOS يجلب الحالة. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | جهاز مدار يصل الى staging. |
| CLI | `iroha_cli nexus quickstart --dry-run` | يتحقق من الاعداد قبل ارسال نداءات الشبكة. |

## استكشاف الاعطال

- **اخطاء TLS/CA** - تحقق من حزمة CA المرفقة مع اصدار Nexus وتاكد من ان `toriiUrl` يستخدم HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** - توقف حوكمة على مستوى المسار؛ راجع
  `docs/source/nexus_operations.md` لخطوات الحوادث.
- **`ERR_UNKNOWN_LANE`** - حدث SDK لتمرير `lane_id`/`dataspace_id` بشكل صريح عند فرض admission
  متعدد المسارات.

ابلغ عن المشاكل عبر GitHub issues الموسومة `NX-14` واذكر SDK والامر المستخدم حتى يتمكن
المسؤولون من اعادة الانتاج بسرعة.

</div>
