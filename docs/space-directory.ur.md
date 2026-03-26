---
lang: ur
direction: rtl
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/space-directory.md (Space Directory Operator Playbook) -->

# Space Directory آپریٹر پلے بُک

یہ پلے بُک واضح کرتا ہے کہ Nexus dataspaces کے لیے **Space Directory** انٹریز کو
کیسے لکھا، شائع، آڈٹ اور rotate کیا جائے۔ یہ `docs/source/nexus.md` میں موجود
آرکیٹیکچر نوٹس اور `docs/source/cbdc_lane_playbook.md` میں CBDC آن بورڈنگ پلان کو
پریکٹیکل پروسیجرز، fixtures اور گورننس ٹیمپلیٹس کے ساتھ مکمل کرتا ہے۔

> **اسکوپ.** Space Directory، dataspace manifests، UAID (Universal Account ID) capability
> پالیسیز، اور audit trail کے لیے canonical رجسٹری کا کردار ادا کرتا ہے جس پر
> ریگولیٹرز انحصار کرتے ہیں۔ اگرچہ بیک اینڈ کنٹریکٹ اب بھی فعال development (NX‑15)
> میں ہے، مگر نیچے دیے گئے fixtures اور پروسیسز tooling اور integration tests میں
> وائر کرنے کے لیے تیار ہیں۔

## 1. بنیادی تصورات

| اصطلاح | وضاحت | حوالہ جات |
|--------|-------|-----------|
| Dataspace | execution context/lane جو گورننس سے منظور شدہ کنٹریکٹس کے سیٹ کو چلاتا ہے۔ | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (blake2b‑32 hash) جو cross‑dataspace permissions کو anchor کرنے کے لیے استعمال ہوتا ہے۔ | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` جو UAID/dataspace جوڑی کے لیے deterministic allow/deny رولز بیان کرتا ہے (deny ہمیشہ غالب ہوتا ہے)۔ | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | گورننس + DA میٹا ڈیٹا جو manifests کے ساتھ شائع ہوتا ہے تاکہ operators، validator sets، composability whitelists، اور audit hooks کو reconstruct کر سکیں۔ | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Norito‑encoded ایونٹس جو manifests کے activate/expire/revoke ہونے پر emit ہوتے ہیں۔ | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Manifest کا لائف سائیکل

Space Directory، **epoch‑بیسڈ lifecycle management** نافذ کرتا ہے۔ ہر تبدیلی کے نتیجے
میں ایک signed manifest bundle اور ایک event بنتا ہے:

| ایونٹ | ٹرِگر | ضروری اقدامات |
|-------|-------|----------------|
| `ManifestActivated` | نیا manifest، `activation_epoch` تک پہنچ جائے۔ | bundle کو براڈکاسٹ کریں، caches اپ ڈیٹ کریں، گورننس approval کو آرکائیو کریں۔ |
| `ManifestExpired` | `expiry_epoch` بغیر renewal کے گزر جائے۔ | operators کو notify کریں، UAID handles کلین کریں، replacement manifest تیار کریں۔ |
| `ManifestRevoked` | expiry سے پہلے ہنگامی deny‑wins فیصلہ۔ | UAID فوراً revoke کریں، incident رپورٹ جاری کریں، follow‑up گورننس ریویو شیڈول کریں۔ |

سبسکرائبرز کو چاہیے کہ مخصوص dataspaces یا UAIDs کو مانیٹر کرنے کے لیے
`DataEventFilter::SpaceDirectory` استعمال کریں۔ Rust میں مثال:

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. آپریٹر ورک فلو

| فیز | مالک(ان) | اقدامات | ثبوت (Evidence) |
|-----|----------|---------|-----------------|
| Draft | Dataspace مالک | fixture کو کلون کریں، allowances/gov میں ترمیم کریں، `cargo test -p iroha_data_model nexus::manifest` چلائیں۔ | Git diff، test log۔ |
| Review | Governance WG | manifest JSON + Norito bytes کو validate کریں، decision log پر دستخط کریں۔ | دستخط شدہ minutes، manifest hash (BLAKE3 + Norito `.to`)۔ |
| Publish | Lane ops | CLI (`iroha app space-directory manifest publish`) کے ذریعے publish کریں، Norito `.to` payload یا raw JSON استعمال کرتے ہوئے **یا** Torii پر `/v1/space-directory/manifests` POST کریں، Torii رسپانس verify کریں، اور `SpaceDirectoryEvent` capture کریں۔ | CLI/Torii receipt، event log۔ |
| Expire | Lane ops / گورننس | جب manifest طے شدہ end‑of‑life پر پہنچ جائے تو `iroha app space-directory manifest expire` (UAID، dataspace، epoch) چلائیں، `SpaceDirectoryEvent::ManifestExpired` کو verify کریں، bindings cleanup کے شواہد آرکائیو کریں۔ | CLI آؤٹ پٹ، event log۔ |
| Revoke | گورننس + Lane ops | `iroha app space-directory manifest revoke` (UAID، dataspace، epoch، reason) چلائیں **یا** Torii پر `/v1/space-directory/manifests/revoke` POST کریں، `SpaceDirectoryEvent::ManifestRevoked` verify کریں، اور evidence bundle اپ ڈیٹ کریں۔ | CLI/Torii receipt، event log، ٹکٹ نوٹ۔ |
| Monitor | SRE/Compliance | telemetry + audit logs مانیٹر کریں، revocations/expiry کے لیے alerts سیٹ کریں۔ | Grafana اسکرین شاٹ، archived logs۔ |
| Rotate/Revoke | Lane ops + گورننس | replacement manifest (نیا epoch) تیار کریں، tabletop exercise کریں، اور revoke کی صورت میں incident فائل کریں۔ | rotation ticket، incident post‑mortem۔ |

کسی rollout کے تمام artefacts کو `artifacts/nexus/<dataspace>/<timestamp>/` کے اندر
اسٹور کریں، اور checksums manifest کے ساتھ رکھیں تاکہ ریگولیٹر کی evidence
requests کو آسانی سے پورا کیا جا سکے۔

## 4. Manifest ٹیمپلیٹ اور fixtures

schema کے canonical ریفرنس کے طور پر curated fixtures استعمال کریں۔ wholesale CBDC
نمونہ (`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) میں allow اور
deny دونوں entries شامل ہیں:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

CBDC آن بورڈنگ کے لیے deny‑wins رول کی ایک عام مثال:

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

جب manifest شائع ہو جائے تو `uaid` + `dataspace` کا جوڑا، permissions رجسٹری میں
پرائمری کی بن جاتا ہے۔ بعد میں آنے والے manifests کو `activation_epoch` کے ذریعے
جوڑا جاتا ہے، اور رجسٹری اس بات کی ضمانت دیتی ہے کہ آخری active ورژن ہمیشہ
“deny wins” semantics کو برقرار رکھے۔

## 5. Torii API

### Manifest publish کرنا

آپریٹرز CLI پر انحصار کیے بغیر manifests کو براہِ راست Torii کے ذریعے publish کر
سکتے ہیں۔

```
POST /v1/space-directory/manifests
```

| فیلڈ | ٹائپ | وضاحت |
|------|------|--------|
| `authority` | `AccountId` | وہ اکاؤنٹ جو publish ٹرانزیکشن پر دستخط کرتا ہے۔ |
| `private_key` | `ExposedPrivateKey` | base64 میں پرائیویٹ کی جسے Torii، `authority` کی طرف سے سائننگ کے لیے استعمال کرے گا۔ |
| `manifest` | `SpaceDirectoryManifest` | مکمل manifest (JSON) جسے سرور سائیڈ پر Norito میں encode کیا جائے گا۔ |
| `reason` | `Option<String>` | audit trail کے لیے اختیاری میسج، جو lifecycle ڈیٹا کے ساتھ اسٹور ہوتا ہے۔ |

JSON باڈی کی مثال:

```jsonc
{
  "authority": "<katakana-i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

Torii، ٹرانزیکشن کے queue میں داخل ہوتے ہی `202 Accepted` ریٹرن کرتا ہے۔ جب بلاک
execute ہوتا ہے تو `SpaceDirectoryEvent::ManifestActivated` (بشرطیکہ
`activation_epoch` پورا ہو) generate ہوتا ہے، bindings از خود rebuild ہوتے ہیں، اور
manifest inventory endpoint نیا payload ظاہر کرنا شروع کر دیتا ہے۔ Access controls،
Space Directory کی دوسری write APIs (CIDR / API token / fee policy gating) کے
مطابق ہوتے ہیں۔

### Manifest revocation API

ہنگامی revocation کے لیے CLI پر شیل آؤٹ کرنا اب ضروری نہیں: آپریٹرز براہِ راست Torii
پر POST کال کر کے canonical `RevokeSpaceDirectoryManifest` انسٹرکشن کو queue میں
ڈال سکتے ہیں۔ submit کرنے والے اکاؤنٹ کے پاس
`CanPublishSpaceDirectoryManifest { dataspace }` پرمیشن ہونی چاہیے، بالکل CLI والے
فلو کی طرح۔

```
POST /v1/space-directory/manifests/revoke
```

| فیلڈ | ٹائپ | وضاحت |
|------|------|--------|
| `authority` | `AccountId` | وہ اکاؤنٹ جو revocation ٹرانزیکشن پر دستخط کرتا ہے۔ |
| `private_key` | `ExposedPrivateKey` | base64 پرائیویٹ کی جسے Torii سائننگ کے لیے استعمال کرتا ہے۔ |
| `uaid` | `String` | UAID literal (`uaid:<hex>` یا 64 حروف پر مشتمل hex digest، LSB=1)۔ |
| `dataspace` | `u64` | وہ dataspace ID جہاں manifest موجود ہے۔ |
| `revoked_epoch` | `u64` | وہ epoch (شامل) جس سے revocation مؤثر ہونا چاہیے۔ |
| `reason` | `Option<String>` | audit trail کے لیے اختیاری میسج جو lifecycle ڈیٹا کے ساتھ محفوظ ہوتا ہے۔ |

JSON باڈی کی مثال:

```jsonc
{
  "authority": "<katakana-i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii، ٹرانزیکشن کے queue میں آنے پر `202 Accepted` واپس کرتا ہے۔ بلاک execute ہونے کے
بعد `SpaceDirectoryEvent::ManifestRevoked` موصول ہو گا، `uaid_dataspaces` map خود
بخود rebuild ہو جائے گا، اور دونوں `/portfolio` اور manifest inventory فوری طور پر
revoked اسٹیٹ ظاہر کریں گے۔ CIDR اور fee‑policy gates، read endpoints کے مطابق ہی
ہوں گے۔

## 6. Dataspace پروفائل ٹیمپلیٹ

پروفائلز، کسی بھی نئے validator کے لیے وہ تمام معلومات مہیا کرتے ہیں جن کی اسے
کنیکٹ ہونے سے پہلے ضرورت ہوتی ہے۔ `profile/cbdc_lane_profile.json` fixture درج ذیل
باتیں دستاویزی شکل میں رکھتا ہے:

- گورننس issuer/quorum (`i105...` + evidence ticket ID)۔
- validators کا سیٹ + quorum اور محفوظ namespaces (`cbdc`, `gov`)۔
- DA پروفائل (کلاس A، attesters کی فہرست، rotation cadence)۔
- composability group ID اور whitelist جو UAIDs کو capability manifests سے جوڑتی ہے۔
- audit hooks (event لسٹ، log schema، PagerDuty سروس)۔

نئے dataspaces کے لیے اسی JSON کو starting point کے طور پر استعمال کریں، اور
whitelist paths کو متعلقہ capability manifests کی طرف اشارہ کرنے کے لیے اپ ڈیٹ کریں۔

## 7. پبلشنگ اور روتیشن

1. **UAID کو encode کرنا۔** blake2b‑32 digest derive کریں اور `uaid:` prefix لگائیں:

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Norito payload encode کرنا۔**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   یا CLI helper چلائیں (جو `.to` فائل اور BLAKE3‑256 digest والی `.hash` فائل دونوں
   لکھے گا):

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Torii کے ذریعے publish کرنا۔**

   ```bash
   # اگر آپ نے manifest کو پہلے ہی Norito میں encode کر لیا ہے:
   iroha app space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   یا پھر اوپر بیان کردہ HTTP API کا استعمال کریں۔ دونوں صورتوں میں یہ evidence
   ضرور آرکائیو کریں:
   - manifest کا اصل JSON؛
   - Norito `.to` payload اور BLAKE3 hash؛
   - Torii کا response اور `SpaceDirectoryEvent`۔

4. **روٹیشن یا revocation۔**

   - منصوبہ بند rotation کے لیے: future `activation_epoch` کے ساتھ replacement manifest
     تیار کریں، tabletop exercise کریں، اور تمام متاثرہ dataspaces کے operators کے
     ساتھ activation کو coordinate کریں۔
   - ہنگامی revocation کے لیے: revocation playbook (workflow table میں “Revoke”
     والی فیز) پر عمل کریں، incident identifier کو `reason` میں شامل کریں، اور
     متعلقہ logs اور snapshots محفوظ رکھیں۔

اس پلے بُک پر عمل کر کے Space Directory، Nexus کے تمام dataspaces میں صلاحیتوں کو
دینے، محدود کرنے اور واپس لینے کے عمل کے بارے میں ایک canonical، قابلِ تصدیق اور
ریگولیٹر‑فرینڈلی ریکارڈ فراہم کرتا ہے۔

</div>
