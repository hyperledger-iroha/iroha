---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
ریٹائر ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Data Availability Replication Policy (DA-4)

_حالت: In Progress -- Owners: Core Protocol WG / Storage Team / SRE_

DA ingest pipeline اب `roadmap.md` (workstream DA-4) میں بیان کردہ ہر blob class
کے لئے deterministic retention targets نافذ کرتا ہے۔ Torii ایسے retention
envelopes کو persist کرنے سے انکار کرتا ہے جو caller فراہم کرے لیکن configured
policy سے match نہ کریں، تاکہ ہر validator/storage node مطلوبہ epochs اور
replicas کی تعداد retain کرے بغیر submitter intent پر انحصار کے۔

## Default policy

| Blob class | Hot retention | Cold retention | Required replicas | Storage class | Governance tag |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 hours | 7 days | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 hours | 180 days | 3 | `cold` | `da.governance` |
| _Default (all other classes)_ | 6 hours | 30 days | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں embedded ہیں اور تمام
`/v1/da/ingest` submissions پر لاگو ہوتی ہیں۔ Torii enforced retention profile
کے ساتھ manifests دوبارہ لکھتا ہے اور جب callers mismatch values فراہم کرتے ہیں
تو warning دیتا ہے تاکہ operators stale SDKs پکڑ سکیں۔

### Taikai availability classes

Taikai routing manifests (`taikai.trm`) ایک `availability_class` (`hot`, `warm`,
یا `cold`) declare کرتے ہیں۔ Torii chunking سے پہلے matching policy نافذ کرتا
ہے تاکہ operators global table edit کئے بغیر stream کے لحاظ سے replica counts
scale کر سکیں۔ Defaults:

| Availability class | Hot retention | Cold retention | Required replicas | Storage class | Governance tag |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 hours | 30 days | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hour | 180 days | 3 | `cold` | `da.taikai.archive` |

Missing hints default طور پر `hot` رکھتے ہیں تاکہ live broadcasts سب سے مضبوط
policy retain کریں۔ اگر آپ کا network مختلف targets استعمال کرتا ہے تو
`torii.da_ingest.replication_policy.taikai_availability` کے ذریعے defaults
override کریں۔

## Configuration

یہ policy `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک *default*
template کے ساتھ per-class overrides کا array فراہم کرتی ہے۔ Class identifiers
case-insensitive ہیں اور `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, یا `custom:<u16>` (governance-approved extensions) قبول
کرتے ہیں۔ Storage classes `hot`, `warm`, یا `cold` قبول کرتی ہیں۔

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Defaults کے ساتھ چلنے کے لئے block کو ویسا ہی رہنے دیں۔ کسی class کو tighten
کرنے کے لئے متعلقہ override update کریں؛ نئی classes کے baseline کو بدلنے کے لئے
`default_retention` edit کریں۔

Taikai availability classes کو الگ سے override کیا جا سکتا ہے via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Enforcement semantics

- Torii user-supplied `RetentionPolicy` کو enforced profile سے replace کرتا ہے
  chunking یا manifest emission سے پہلے۔
- Pre-built manifests جو mismatch retention profile declare کریں، `400 schema mismatch`
  کے ساتھ reject ہوتے ہیں تاکہ stale clients contract کو weaken نہ کر سکیں۔
- ہر override event log ہوتا ہے (`blob_class`, submitted vs expected policy)
  تاکہ rollout کے دوران non-compliant callers سامنے آئیں۔

Updated gate کے لئے [Data Availability Ingest Plan](ingest-plan.md)
(Validation checklist) دیکھیں جو retention enforcement کو cover کرتا ہے۔

## Re-replication workflow (DA-4 follow-up)

Retention enforcement صرف پہلا قدم ہے۔ Operators کو یہ بھی ثابت کرنا ہوگا کہ live
manifests اور replication orders configured policy کے مطابق رہیں تاکہ SoraFS
non-compliant blobs کو خودکار طور پر re-replicate کر سکے۔

1. **Drift پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` emit
   کرتا ہے جب caller stale retention values بھیجتا ہے۔ اس log کو
   `torii_sorafs_replication_*` telemetry کے ساتھ جوڑ کر replica shortfalls یا
   delayed redeployments کو پکڑیں۔
2. **Intent بمقابلہ live replicas کا diff۔** نیا audit helper استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   یہ command `torii.da_ingest.replication_policy` کو config سے لوڈ کرتا ہے، ہر
   manifest (JSON یا Norito) decode کرتا ہے، اور اختیاری طور پر `ReplicationOrderV1`
   payloads کو manifest digest سے match کرتا ہے۔ خلاصہ دو conditions flag کرتا ہے:

   - `policy_mismatch` - manifest retention profile enforced policy سے مختلف ہے
     (یہ Torii misconfig کے بغیر نہیں ہونا چاہئے).
   - `replica_shortfall` - live replication order `RetentionPolicy.required_replicas`
     سے کم replicas طلب کرتا ہے یا target سے کم assignments دیتا ہے۔

   Non-zero exit status فعال shortfall کی نشاندہی کرتا ہے تاکہ CI/on-call
   automation فوراً page کر سکے۔ JSON رپورٹ کو
   `docs/examples/da_manifest_review_template.md` packet کے ساتھ attach کریں
   تاکہ Parliament votes کے لئے دستیاب ہو۔
3. **Re-replication trigger کریں۔** جب audit shortfall رپورٹ کرے، ایک نیا
   `ReplicationOrderV1` جاری کریں via governance tooling جو
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   میں بیان ہے، اور audit دوبارہ چلائیں جب تک replica set converge نہ کرے۔
   Emergency overrides کے لئے CLI output کو `iroha app da prove-availability` کے
   ساتھ جوڑیں تاکہ SREs وہی digest اور PDP evidence refer کر سکیں۔

Regression coverage `integration_tests/tests/da/replication_policy.rs` میں ہے؛
suite `/v1/da/ingest` کو mismatched retention policy بھیجتی ہے اور verify کرتی ہے
کہ fetched manifest caller intent کے بجائے enforced profile expose کرے۔
