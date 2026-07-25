---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61b0d714fee41bed9801e9d9acd32885b812afecde81ead2a8543b80a377b66f
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-plan
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/pin_registry_plan.md`. יש לשמור על שתי הגרסאות מסונכרנות כל עוד התיעוד הישן פעיל.
:::

# תוכנית מימוש Pin Registry של SoraFS (SF-4)

SF-4 מספק את חוזה Pin Registry ואת שירותי התשתית התומכים המאחסנים התחייבויות manifest,
אוכפים מדיניות pin ומספקים API ל-Torii, לשערים ולמתזמרים. מסמך זה מרחיב את תוכנית
האימות במשימות מימוש קונקרטיות, כולל לוגיקה on-chain, שירותי host, fixtures
ודרישות תפעוליות.

## היקף

1. **מכונת מצבים של registry**: רשומות Norito עבור manifests, aliases, שרשראות יורשים,
   אפוקי שימור ומטא-דאטה של ממשל.
2. **מימוש החוזה**: פעולות CRUD דטרמיניסטיות למחזור חיי pin (`ReplicationOrder`,
   `Precommit`, `Completion`, eviction).
3. **חזית שירות**: נקודות קצה gRPC/REST מגובות registry ש-Torii ו-SDKs צורכים,
   כולל עימוד ואטסטציה.
4. **tooling ו-fixtures**: עוזרי CLI, וקטורי בדיקה ותיעוד לשמירה על סנכרון
   manifests, aliases ו-envelopes של ממשל.
5. **טלמטריה ותפעול**: מדדים, התראות ו-runbooks לבריאות registry.

## מודל נתונים

### רשומות ליבה (Norito)

| Struct | תיאור | שדות |
|--------|-------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | מיפוי alias -> CID של manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | הוראה ל-providers להצמיד manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | אישור ספק. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | צילום מצב של מדיניות ממשל. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Fixtures ו-CI

- תיקיית fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` מאחסנת snapshots חתומים של manifest/alias/order שנוצרים מחדש על ידי `cargo run -p iroha_core --example gen_pin_snapshot`.
- שלב CI: `ci/check_sorafs_fixtures.sh` מייצר מחדש את ה-snapshot ונכשל אם יש diffs, כדי לשמור על תאימות fixtures של CI.
- בדיקות אינטגרציה (`crates/iroha_core/tests/pin_registry.rs`) מכסות את המסלול התקין וכן דחיית alias כפול, guards לאישור/שימור alias, handles של chunker שאינם תואמים, אימות ספירת רפליקות וכשלי guard של ירושה (מצביעים לא ידועים/מאושרים מראש/הוצאו/הפניה עצמית); ראו מקרי `register_manifest_rejects_*` לפרטי כיסוי.
- בדיקות יחידה מכסות כעת אימות alias, guards לשימור ובדיקות יורש ב-`crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; זיהוי ירושה רב-שלבית יגיע עם מכונת המצבים.
- JSON זהב לאירועים המשמשים בצנרות observability.

## טלמטריה ותצפיתיות

מדדים (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- טלמטריית providers קיימת (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) נשארת בתחום עבור dashboards end-to-end.

לוגים:
- זרם אירועים Norito מובנה לביקורות ממשל (חתום?).

התראות:
- הזמנות שכפול ממתינות שחורגות מה-SLA.
- תפוגת alias מתחת לסף.
- הפרות שימור (manifest לא חודש לפני תפוגה).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` עוקב אחרי סך מחזור החיים של manifests, כיסוי alias, רוויה של backlog, יחס SLA, חפיפות latency מול slack ושיעורי הזמנות שהוחמצו לסקירת on-call.

## Runbooks ותיעוד

- לעדכן את `docs/source/sorafs/migration_ledger.md` כדי לכלול עדכוני סטטוס של registry.
- מדריך מפעילים: `docs/source/sorafs/runbooks/pin_registry_ops.md` (כבר פורסם) המכסה מדדים, התראות, פריסה, גיבוי ושחזור.
- מדריך ממשל: לתאר פרמטרי מדיניות, תהליך אישור, טיפול במחלוקות.
- דפי עזר API לכל נקודת קצה (Docusaurus docs).

## תלות ורצף

1. להשלים משימות תוכנית האימות (שילוב ManifestValidator).
2. לסיים סכמת Norito + ברירות מחדל של מדיניות.
3. לממש חוזה + שירות ולחבר טלמטריה.
4. לייצר מחדש fixtures ולהריץ חבילות אינטגרציה.
5. לעדכן docs/runbooks ולסמן פריטי roadmap כהושלמו.

כל פריט צ'ק-ליסט תחת SF-4 חייב להפנות לתוכנית זו בעת התקדמות.
חזית ה-REST מספקת כעת נקודות קצה של רשימה עם אטסטציה:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` ו-`GET /v1/sorafs/replication` מציגות קטלוג alias פעיל
  ו-backlog של הזמנות שכפול עם עימוד עקבי וסינוני סטטוס.

ה-CLI עוטף קריאות אלו (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) כדי שמפעילים יוכלו לאוטומט בדיקות registry ללא נגיעה
ב-API ברמת נמוכה.
