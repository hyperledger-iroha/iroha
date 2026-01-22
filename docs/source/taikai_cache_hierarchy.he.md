---
lang: he
direction: rtl
source: docs/source/taikai_cache_hierarchy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5de9b350eb15f31f54ec4f4eca4fb89ac89138b32deea1f60d90510dbf65974
source_last_modified: "2026-01-04T10:50:53.693968+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/taikai_cache_hierarchy.md -->

# היררכיית המטמון של Taikai

_סטטוס: הושלם (שערי אמינות + יציאת hedging פעילים)_ — בעלי תפקיד: SRE / Networking TL / DA Program  
פריט רודמאפ: **SNNet-14 — היררכיית מטמון Taikai ואכיפת QoS**

היררכיית המטמון של Taikai מספקת שכבת אחסון דטרמיניסטית ורב-שכבתית לפיילוטים של
הפצה ב-SoraNet. היא מגבה את backlog של "SNNet-14" בכך שהיא מחברת מטמון ברמת
production לתוך crate `sorafs_orchestrator` עם קיבולת, retention וערבויות QoS
ברי-תצורה.

## Highlights

- **מטמון תלת-שכבתי (hot/warm/cold).** לכל שכבה יש הגדרות קיבולת ו-retention
  עצמאיות, כך שקטעי live נשארים ב-hot, חומר שניגש לאחרונה נוזל ל-warm, ותוכן
  ארכיוני נשמר ב-cold. יישום LRU קל משקל מטפל בקידום/הורדה ומעקב פינוי
  (`CacheEviction`, `CachePromotion`).
- **דלי אסימונים QoS.** המטמון אוכף תקציבים לפי מחלקה
  (priority/standard/bulk) באמצעות דלי אסימונים דטרמיניסטיים (`QosEnforcer`)
  כך ששכבות hot/warm/cold מאשרות/מקדמות תעבורה באופן צפוי. הפצת תורים אינה
  צורכת עוד תקציבי shaping; שערי אמינות של shard מטפלים בניתוב בעוד QoS בצד
  המטמון שומר על עומסי insert/promotion צפויים.
- **שערי אמינות.** מפסקי זרם ברמת shard מופעלים לאחר כשלונות רצופים (חלון פתוח
  של 2 ש׳ כברירת מחדל), מסיטים אצוות ל-shards בריאים בטבעת hash עקבית, וחושפים
  את מצבם דרך gauges ומוני failover כך ש-brownouts נראים הן ב-JSON והן ב-Grafana.
- **Exit hedging.** תור המשיכה של Taikai מפעיל כעת fetches מגודרות ישירות מתוך
  עטיפת ה-orchestrator: אצוות שנמצאות in-flight זמן ארוך מ-`hedge_after`
  (125 ms ברירת מחדל, נשלט ע"י `TaikaiPullQueueConfig`) מפעילות ניסיון fetch
  שני בעוד המקור ממשיך, עם מוני hedged שמופיעים בסטטיסטיקות התור ובסיכומי
  `taikai_cache_queue`. נתיב ההדג' מכוסה בבדיקה
  `taikai_fetch_wrapper_hedges_overdue_batches` ב-`crates/sorafs_orchestrator/src/lib.rs`.
- **Instrumentation-first.** פעולות insert/fetch/QoS מחזירות תוצאות מובנות
  (`TaikaiCacheInsertOutcome`, `TaikaiCacheQueryOutcome`, `QosError`) כך שה-orchestrator
  יכול להציג סיבות פינוי, מסלולי קידום ואירועי rate limit בטלמטריה.
- **Consistent hashing.** `TaikaiShardRing` מספק hash עקבי עם virtual-nodes עבור
  מיקום shard. כאשר רשת ה-CAA gossip של SNNet-14 תעלה, אותה טבעת תניע שיבוץ
  shards ואיזון מחדש.
- **רשומות CAA gossip.** `CacheAdmissionRecord` + `CacheAdmissionEnvelope` לוכדים
  קבלות/פינויים ל-hot/warm/cold, כוללים digests דטרמיניסטיים, מטא-דטה TTL,
  וחתימות עם מפתחות guard-directory. הודעות gossip עוטפות את המעטפה בתוך
  `CacheAdmissionGossipBody`/`CacheAdmissionGossip` עם nonce ו-ttl עצמאי,
  ו-`CacheAdmissionReplayFilter` אוכף חלון replay דטרמיניסטי לפני שהמטמונים
  פועלים (ראו `crates/sorafs_orchestrator/src/taikai_cache.rs`).
- **רמזי Taikai ברמת Chunk.** `ChunkFetchSpec` חושף בלוק `taikai_segment_hint`
  אופציונלי (JSON + בינארי) כדי ש-manifests של Taikai יוכלו לסמן את קטעי CMAF
  שצריכים לעבור דרך התור. ה-orchestrator מפעיל את גשר התור רק כאשר יש לפחות
  hint אחד, כך שאין השפעה על fetches כלליים של SoraFS; manifests של DA מעבירים
  את מטא-דטה Taikai ישירות לתוך תוכנית fetch כך שהרמזים מגיעים אוטומטית כאשר
  Torii מחזיר קטעי Taikai.
- **טבעת shard מונעת CAA.** `CacheAdmissionTracker` קולט `CacheAdmissionGossip`,
  שומר חלון replay, מפוגג רשומות ישנות, וכותב מחדש את טבעת ה-shards של תור המשיכה
  כך שה-hedging וה-failover יעקבו אחרי הטופולוגיה שמופצת ב-gossip.

## Configuration

ה-orchestrator מקבל כעת `taikai_cache` בתוך ה-JSON bindings שלו. התצורה משקפת
`TaikaiCacheConfig`:

```json
{
  "taikai_cache": {
    "hot_capacity_bytes": 8388608,
    "hot_retention_secs": 45,
    "warm_capacity_bytes": 33554432,
    "warm_retention_secs": 180,
    "cold_capacity_bytes": 268435456,
    "cold_retention_secs": 3600,
    "qos": {
      "priority_rate_bps": 83886080,
      "standard_rate_bps": 41943040,
      "bulk_rate_bps": 12582912,
      "burst_multiplier": 4
    }
  }
}
```

כל השדות אופציונליים ב-payload של orchestrator: השמטת המקטע מכבה את המטמון, בעוד
שכל מפתח בודד מקבל ברירת מחדל לפי `TaikaiCacheConfig::default`. ה-orchestrator
חושף handle חי דרך `Orchestrator::taikai_cache()` כך שהזרמים העתידיים של SNNet-14
יוכלו לשלב CAA gossip, exit hedging ודגימות observability.

## CAA Gossip Plane (SNNet-14A)

ה-mטמון מפיק כעת רשומות הכרזה דטרמיניסטיות בכל פעם שקטע מתקבל או מפונה משכבה.
כל הכרזה מיוצגת ע"י `CacheAdmissionRecord` (מפתח קטע, שכבה, QoS class,
digest/size של payload, issuer shard, חותמות issued/expiry) ומוכללת בתוך
`CacheAdmissionEnvelope` שמכיל מזהה guard directory, חותם וחתימה מנותקת:

```rust
let record = CacheAdmissionRecord::from_segment(
    TaikaiShardId(3),
    GuardDirectoryId::new("soranet/canary"),
    &cached_segment,
    CacheTierKind::Hot,
    CacheAdmissionAction::Admit,
    issued_unix_ms,
    Duration::from_secs(30),
)?;
let envelope = CacheAdmissionEnvelope::sign(record, guard_key_pair)?;
envelope.verify(now_unix_ms)?;
```

- רשומות אוכפות TTL מוגבל (`issued_unix_ms + ttl`) כך שרשומות ישנות נדחות אוטומטית.
- מעטפות חייבות לעבור אימות חתימה לפני שהמטמונים או orchestrators פועלים עליהן;
  אימות כושל מעלה `CacheAdmissionError::InvalidSignature`.
- סריאליזציה/דה-סריאליזציה של Norito שומרות על פורמט דטרמיניסטי הן ל-gossip
  והן לאחסון.
- הכרזות gossip עוטפות מעטפות עם nonce ו-TTL עצמאי (`CacheAdmissionGossipBody`
  → `CacheAdmissionGossip::sign/verify`), ו-`CacheAdmissionReplayFilter` דוחה
  כפילויות במהלך חלון תצורה כך ש-peers לא יוכלו להציף replays:

```rust
let gossip_body =
    CacheAdmissionGossipBody::new(envelope.clone(), issued_unix_ms, Duration::from_secs(15))?;
let gossip = CacheAdmissionGossip::sign(gossip_body, guard_key_pair)?;
gossip.verify(now_unix_ms)?;
let mut replay = CacheAdmissionReplayFilter::new(Duration::from_millis(500), 128)?;
if replay.observe(&gossip, now_unix_ms)? {
    // safe to process admission/eviction event
}
```

בדיקות אינטגרציה שמכסות round-trips, טיפול ב-expiry וטמפרינג נמצאות ב-
`crates/sorafs_orchestrator/tests/taikai_cache.rs`, ומבטיחות שהשינויים העתידיים
לפורמט יישארו דטרמיניסטיים.

## Telemetry

Instrumentation מוזן כעת לרישום `iroha_telemetry` כך שדשבורדים יכולים לעקוב
אחר יעילות המטמון לצד פאנלי `sorafs.fetch.*`. סדרות Prometheus הבאות (עם OTLP
מראות) מפיקות:

- `sorafs_taikai_cache_query_total{result="hit|miss",tier}` — מוני hit/miss
- `sorafs_taikai_cache_insert_total{tier}` — אירועי insert לכל שכבה, יחד עם
  `sorafs_taikai_cache_bytes_total{event="insert|hit",tier}` לחשיפת קצב בייטים
- `sorafs_taikai_cache_evictions_total{tier,reason}` — פינוי עקב קיבולת/expiry
- `sorafs_taikai_cache_promotions_total{from_tier,to_tier}` — קידומים warm/cold
- `sorafs_taikai_qos_denied_total{class}` — דחיות QoS לכל class
- `sorafs_taikai_queue_depth{state}` — gauge עומק תור (pending segments/bytes/batches,
  in-flight batches, open circuits)
- `sorafs_taikai_queue_events_total{event,class}` — אירועי תור (issued, hedged,
  rate-limited, backpressure) לפי class
- `sorafs_taikai_shard_circuits_open{shard}` ו-
  `sorafs_taikai_shard_failovers_total{preferred_shard,selected_shard}` — מדדים
  לאמינות מפסקי זרם של תור המשיכה.

מדדים אלו מופיעים ב-pack observability של SoraFS fetch
(`dashboards/grafana/sorafs_fetch_observability.json`) כך שאופרטורים יוכלו
להתריע על שיעורי miss/brownout במהלך rollouts של SNNet-14. דשבורד המטמון של
Taikai (`dashboards/grafana/taikai_cache.json`) כולל כעת פאנלים ייעודיים עבור
open circuits, עומק תור, אירועי hedging/rate-limit ושיעורי failover כדי ש-SREs
יוכלו לקשר brownouts לחוסר יציבות ברמת shard.

## בקרות אופרטור ו-Overrides ל-rollout

### CLI (`sorafs_cli fetch`)

`sorafs_cli fetch` מקבל כעת דגל `--taikai-cache-config=PATH` ייעודי כדי
לאפשר או לכוון את המטמון ללא עריכת תצורת orchestrator הבסיסית. הדגל מצפה
ל-payload JSON שמשקף `TaikaiCacheConfig` — אובייקט עצמאי או JSON מלא של
orchestrator עם מקטע `taikai_cache`. דוגמה:

```json
{
  "hot_capacity_bytes": 8388608,
  "hot_retention_secs": 45,
  "warm_capacity_bytes": 33554432,
  "warm_retention_secs": 180,
  "cold_capacity_bytes": 268435456,
  "cold_retention_secs": 3600,
  "qos": {
    "priority_rate_bps": 83886080,
    "standard_rate_bps": 41943040,
    "bulk_rate_bps": 12582912,
    "burst_multiplier": 4
  },
  "reliability": {
    "failures_to_trip": 3,
    "open_secs": 2
  }
}
```

Invoke:

```bash
sorafs_cli fetch \
  --plan=fixtures/taikai_plan.json \
  --manifest-id=deadbeef... \
  --provider name=edge-a,provider-id=...,base-url=...,stream-token=... \
  --taikai-cache-config=configs/taikai-cache/hot-warm-warmup.json \
  --output=/tmp/payload.car
```

הפצת מספר snippets של JSON מאפשרת ל-SREs לקבע גדלי שכבות שונים לפי שלב rollout
(canary vs. ramp). האפשרות מחוברת דרך
`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`.

שדות reliability אופציונליים; השמיטו אותם כדי להשתמש בברירות המחדל (3 כשלונות
להפעלת מפסק, חלון פתוח 2 ש׳).

סיכום fetch כולל כעת בלוק `taikai_cache_summary` כך שאופרטורים יוכלו לבדוק את
בריאות המטמון ללא סקרייפ Prometheus. דוגמה:

```json
"taikai_cache_summary": {
  "hits": {"hot": 12, "warm": 4, "cold": 0},
  "misses": 3,
  "inserts": {"hot": 16, "warm": 2, "cold": 1},
  "evictions": {
    "hot": {"expired": 0, "capacity": 5},
    "warm": {"expired": 0, "capacity": 1},
    "cold": {"expired": 0, "capacity": 0}
  },
  "promotions": {"warm_to_hot": 4, "cold_to_warm": 0, "cold_to_hot": 0},
  "qos_denials": {"priority": 1, "standard": 0, "bulk": 0}
}
```

ה-snapshot של התור מספק טלמטריית אמינות משלימה:

```json
"taikai_cache_queue": {
  "pending_segments": 18,
  "pending_bytes": 9437184,
  "pending_batches": 4,
  "in_flight_batches": 2,
  "hedged_batches": 1,
  "dropped_segments": 0,
  "failovers": 1,
  "open_circuits": 1
}
```

השתמשו ב-snapshot יחד עם הדשבורדים כדי לקשר bursts של miss, דחיות QoS ו-
churn של פינוי לפני שינוי מדיניות transport.

### Coalesced pull/push queue

SNNet-14 מספק כעת את החלק הראשון של orchestration משולב pull/push דרך
`TaikaiPullQueue`. כל מופע orchestrator שמפעיל `taikai_cache` מקצה אוטומטית:

- **אצוות דטרמיניסטיות.** `SegmentKey` נכנסים (עם רמזי QoS/גודל) מתלכדים לאצוות
  מודעות-shard (`TaikaiPullBatch`) שמכבדות את גבולות שכבות המטמון. התור אוכף
  ספים תצורתיים של `max_batch_segments`, `max_batch_bytes`, ו-`max_in_flight_batches`
  כדי שיציאות upstream לא יראו fan-out לא מוגבל.
- **פידבק back-pressure.** כאשר ה-backlog עולה מעל הסף המוגדר (ברירת מחדל: 256
  pending segments) התור נכשל מהר עם `TaikaiQueueError::Backpressure`, מה שמאפשר
  ל-callers להוריד עומס או לעקוף זמנית את המטמון במקום להציף יציאות.
- **Exit hedging.** אצוות שנשארות in-flight מעבר ל-`hedge_after` (125 ms כברירת
  מחדל) נשלחות מחדש עם הדגל `hedged=true`; עטיפת fetch ב-orchestrator מכבדת את
  האות ומריצה ניסיון fetch שני תוך שמירה על הבקשה המקורית פעילה. מוני hedged
  מוזנים ישירות לתקציר ה-JSON של CLI/Jenkins.
- **מפסקי זרם + failover.** שלושה כשלונות רצופים נגד shard מפעילים מפסק זרם של
  2 ש׳. בזמן שהמפסק פתוח, התור מקצה מחדש אצוות ל-shard בריא הבא בטבעת hash עקבית
  ורושם את מצב הפתיחה (`sorafs_taikai_shard_circuits_open`) ואת נתיב ה-failover
  (`sorafs_taikai_shard_failovers_total`) כדי ש-brownouts יהיו גלויים ל-SREs.
- **אינטגרציה עם shard ring.** `TaikaiCacheHandle::configure_shards` מחליף את
  טבעת ה-hash העקבית במקום, כך שעבודות CAA gossip עתידיות יוכלו לאזן שכבות מטמון
  ללא שינוי בצנרת התור.
- **Hookים ל-observability.** `FetchSession` לוכד גם `taikai_cache_summary`
  (hits/misses לפי שכבה) וגם `taikai_cache_queue` (pending segments/bytes/batches,
  hedged batches, drops, failovers, open circuits). `sorafs_cli --json-out`
  מוציא את בלוק `taikai_cache_queue` המעודכן וחבילת Grafana משקפת את אותם שדות,
  כך ש-SREs רואים בזמן אמת עומק תור, התנהגות hedging ובריאות shards ללא מוני
  shaping שהוסרו.

#### Hookים לשילוב orchestrator

`Orchestrator::taikai_cache_handle()` מחזיר כעת clone של `TaikaiCacheHandle`
הפנימי, ומספק fetchers בממשק בטוח מעל המטמון ותור המשיכה. ה-handle חושף helpers
לזרימת SN14-B המלאה, ונתיב ה-fetch מחבר את התור ישירות כאשר chunk מפרסם
`taikai_segment_hint` (השדה `ChunkFetchSpec` שמנפיקים manifests של Taikai).
עד שהכלי ingest/manifest יוסיפו רמזים כאלה התור יישאר idle, אך ברגע שהם יופיעו
ה-orchestrator מתחיל להנפיק/לבטל כניסות לתור במקביל ל-fetchים הרגילים. ה-helpers
החדשים כוללים:

- `enqueue_pull(..)` מוסיף `TaikaiPullRequest` ומחזיר `TaikaiQueueError::Backpressure`
  כאשר ה-backlog חורג מהסף המוגדר. נעילות מורעלות מעלות
  `TaikaiQueueError::Unavailable`, מה שמאפשר fallback ל-fetch ישיר אם ה-handle
  הופך ללא בריא.
- `issue_ready_batch[_at](..)` מפיק את `TaikaiPullBatch` הבא לפי shard, בעוד
  `hedge_overdue_batches[_at](..)` משכפל אצוות in-flight עם הדגל `hedged=true`
  לאחר שעבר `hedge_after` ללא אישור.
- `complete_batch(..)` מפנה את in-flight slot לאחר שהיציאה מאשרת מסירה, בעוד
  `fail_batch(..)` מסמן את ה-shard כלא בריא ופותח את מפסק הזרם/מדדים בעת כשל.
  לולאת fetch קוראת לכך אוטומטית דרך ה-bridge, כך שאופרטורים צריכים רק
  לחבר `taikai_segment_hint` ל-manifests כדי להפעיל את התור. ה-helper
  `wrap_fetcher_with_taikai_queue` מטפל במחזור חיי הטיקטים וה-hedging לכל fetch
  מתויג Taikai, מריץ בקשה שנייה כאשר התור מדווח על אצווה מאחרת בעוד הבקשה
  המקורית נשארת פעילה.

דוגמת orchestration:

```rust
if let Some(handle) = orchestrator.taikai_cache_handle() {
    let key = TaikaiSegmentKey::from_envelope(&segment_envelope);
    let request = TaikaiPullRequest::new(key, TaikaiQosClass::Priority, segment_bytes);
    if let Err(TaikaiQueueError::Backpressure { .. }) = handle.enqueue_pull(request) {
        downgrade_to_single_source();
    }

    if let Ok(Some(batch)) = handle.issue_ready_batch() {
        dispatch_to_primary_exit(batch.clone());
        for hedged in handle.hedge_overdue_batches().unwrap_or_default() {
            dispatch_to_secondary_exit(hedged);
        }
        let _ = handle.complete_batch(batch.id.into());
    }
}
```

Cache admission gossip מנווט כעת את shard ring ישירות: ה-
`CacheAdmissionTracker` (זמין דרך `Orchestrator::taikai_cache_tracker()` או
`Orchestrator::apply_cache_admission_gossip(...)`) מאמת רשומות
`CacheAdmissionGossip`, מכבד חלונות replay/expiry, וכותב מחדש את טבעת ה-hash
כדי ש-hedging ו-failover יעקבו אחר טופולוגיית הקבלה שמפורסמת ב-CAA plane.

### SDK overrides

קליינטים של JavaScript יכולים כעת להעביר אובייקט `taikaiCache` ל-
`sorafsGatewayFetch(...)`. משטח TypeScript מיוצא דרך
`SorafsTaikaiCacheOptions`/`SorafsTaikaiCacheQosOptions`
(`javascript/iroha_js/index.d.ts`) ומשקף את ה-struct ב-Rust:

```ts
await sorafsGatewayFetch(manifestId, chunker, planJson, providers, {
  taikaiCache: {
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: {
      priorityRateBps: 83_886_080,
      standardRateBps: 41_943_040,
      bulkRateBps: 12_582_912,
      burstMultiplier: 4,
    },
  },
});
```

Bindings דוחים קיבולות לא תקינות (אפס/שלילי) ומכפילי burst, כך שאכיפת SDK
סימטרית ל-CLI (`javascript/iroha_js/src/sorafs.js`, `crates/iroha_js_host/src/lib.rs`).

אפליקציות Swift יכולות להזין payload זהה דרך `SorafsGatewayFetchOptions` עם
השדה החדש `taikaiCache`:

```swift
let cache = SorafsTaikaiCacheOptions(
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: SorafsTaikaiCacheQosOptions(
        priorityRateBps: 83_886_080,
        standardRateBps: 41_943_040,
        bulkRateBps: 12_582_912,
        burstMultiplier: 4
    )
)
let options = SorafsGatewayFetchOptions(taikaiCache: cache)
```

Python bindings (`iroha_python.sorafs_gateway_fetch`) מקבלים את אותה מבנה
באמצעות mapping מקונן:

```python
options = {
    "taikai_cache": {
        "hot_capacity_bytes": 8_388_608,
        "hot_retention_secs": 45,
        "warm_capacity_bytes": 33_554_432,
        "warm_retention_secs": 180,
        "cold_capacity_bytes": 268_435_456,
        "cold_retention_secs": 3_600,
        "qos": {
            "priority_rate_bps": 83_886_080,
            "standard_rate_bps": 41_943_040,
            "bulk_rate_bps": 12_582_912,
            "burst_multiplier": 4,
        },
    },
}
```

ה-overrides עוברים דרך Norito bridge (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift:4`)
ועוזר ה-gateway של Python (`python/iroha_python/iroha_python_rs/src/lib.rs:2133`),
כך שכל SDK משתתף ברולאאוט SNNet-14 ללא חיבור JSON ידני.

### Governance DAG & checklist לרולאאוט מדורג

- לשמור פרופילי מטמון מאושרים תחת governance DAG (`configs/taikai_cache/…`) ולהפנות
  אליהם במניפסט הרולאאוט (`docs/source/sorafs_governance_dag_plan.md`).
- כל רשומה חייבת לכלול את ה-JSON לעיל ואת payload Norito החתום כך שממסרי SoraNS
  יוכלו לאמת provenance לפני קבלת הפרמטרים החדשים.
- בעת קידום פרופיל חדש, לפרסם את DAG CID ולצרף את אותו JSON ליומן incident/runbook
  כדי שתרגילי brownout יקבלו קלט דטרמיניסטי.

התיקיה `configs/taikai_cache/profiles/` מזריעה את קלטי ה-JSON הקנוניים
(`balanced-canary.json`, `ramp-2026q2.json`). הריצו
`cargo xtask sorafs-taikai-cache-bundle` כדי להמיר את הפרופילים לחבילות
ממשל תחת `artifacts/taikai_cache/<profile>/`, שמכילות את `profile.json` הקנוני,
את `cache_config.json` החולץ, את payload Norito (`profile.taikai_cache_profile.to`),
ואת קובץ המטא-דטה המוכן למניפסט חתום (`profile.manifest.json`). הפקודה גם מרעננת
את `artifacts/taikai_cache/index.json`, מה שמקל לצרף hashes עדכניים לרשומות שינוי.

### Playbook לפתרון תקלות

1. השתמשו ב-`sorafs_cli fetch --json-out …` כדי לבדוק `taikai_cache_summary` עבור
   bursts של miss לפני שעוברים ל-transport חד-מקור.
2. עקבו אחרי `sorafs_taikai_cache_query_total{result="miss"}` ו-
   `sorafs_taikai_cache_evictions_total{reason="capacity"}` — אם אחד מהם מזנק,
   בדקו מחדש את גדלי השכבות שנלכדו ב-DAG והריצו שוב `--taikai-cache-config` עם
   JSON מתקן.
3. אמתו את override של SDK באמצעות התאמת תקציר Norito של ה-payload שנמשכה מול
   רשומת ה-governance (ה-CLI כותב את שני הארטיפקטים ל-
   `artifacts/sorafs_orchestrator/<stamp>/`).
4. תעדו overrides ידניים (דגל CLI או אפשרות SDK) ביומן התפעול של SNNet-14 תוך
   24 ש׳ וחזרו לפרופיל ה-DAG המפורסם לאחר סגירת האירוע.

## Next Steps

1. *(Done)* לחבר את תוצאות המטמון לטלמטריית orchestrator
   (`sorafs.fetch.*`) ולדשבורדים.
2. *(Done)* ליישם את תור ה-pull/push המאוחד, לחשוף סטטיסטיקות תור ב-orchestrator/CLI,
   לשמור על מדדי hedging/back-pressure בטלמטריה, ולחבר את התור ל-plane של CAA gossip
   דרך `CacheAdmissionTracker`/`Orchestrator::apply_cache_admission_gossip(...)`.
   manifests של Taikai מעבירים כעת `taikai_segment_hint` ישירות לתוכניות fetch כך
   שהתור מופעל אוטומטית כאשר Torii מחזיר קטעי Taikai; מפסקי זרם וטלמטריית failover
   (SNNet-14C) פעילים.
3. להוסיף אכיפת slice פר-אירוע (R_hot / R_cold) ואינטגרציית registry.
4. *(Done)* לחשוף מדדי בריאות מטמון ומוני QoS דרך משטחי SDK נוספים מעבר ל-CLI
   (wrappers ב-JS/Python/Swift מפיקים כעת snapshots של `taikai_cache_summary` +
   `taikai_cache_queue` לצד דוחות gateway fetch).

משימות ההמשך נשארות במעקב תחת SNNet-14 ב-`roadmap.md`.

</div>
