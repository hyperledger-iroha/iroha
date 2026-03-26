---
lang: he
direction: rtl
source: docs/source/soracles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4f46b6cd6b0a1007a286930473f5a36b8b2284a6ffe0b77342e1034b8a777b28
source_last_modified: "2025-12-14T09:58:32.061422+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soracles.md -->

# Soracles — שכבת אורקל מגובה מאמתים

מסמך זה מתעד את הסכמות הקנוניות וכללי התזמון הדטרמיניסטיים עבור שכבת האורקל
המופעלת על ידי מאמתים. הוא משלים את סעיפי המפה OR-1 עד OR-3, OR-6 ו‑OR-12
באמצעות קיבוע פריסות Norito/JSON, נגזרות ועדה/מנהיג, hashing/קצב/השמטת PII
למחברים, ומשטח ה‑replay עבור gossip.

- **הפניה לקוד:** `crates/iroha_data_model/src/oracle/mod.rs`
- **Fixtures:** `fixtures/oracle/*.json`

## מודל נתונים

### הגדרת פיד (`FeedConfig`)
- `feed_id: FeedId` — שם מנורמל (UTS‑46/NFC נאכפים ע"י `Name`).
- `feed_config_version: u32` — מונוטוני לכל פיד.
- `providers: Vec<OracleId>` — מפתחות אורקל קשורי‑מאמתים הזכאים להגרלות.
- `connector_id/version: string/u32` — מחבר off‑chain מוצמד לפיד.
- `cadence_slots: NonZeroU64` — הפיד פעיל כאשר `slot % cadence == 0`.
- `aggregation: AggregationRule` — `MedianMad(u16)` או `Percentile(u16)`; Norito
  JSON מקודד זאת כ‑`{ "aggregation_rule": "<Variant>", "value": <u16> }`.
- `outlier_policy: OutlierPolicy` — `Mad(u16)` או `Absolute { max_delta }`; פריסת
  JSON משתמשת ב‑`{ "outlier_policy": "<Variant>", "value": ... }`.
- `min_signers/committee_size: u16` — גבולות בטיחות (חייבים לעלות על byzantine `f`).
- `risk_class: RiskClass` — `Low | Medium | High` (מניע ממשל/quorum).
  JSON משתמש ב‑`{ "risk_class": "<Variant>", "value": null }` עבור וריאנטים יחידתיים.
- תקרות: `max_observers`, `max_value_len`, `max_error_rate_bps`,
  `dispute_window_slots`, `replay_window_slots`.

### תצפיות ודוחות
- `ObservationBody` — `{feed_id, feed_config_version, slot, provider_id,
  connector_id/version, request_hash: Hash, outcome, timestamp_ms?}`.
  - `ObservationOutcome` — `Value(ObservationValue)` או
    `Error(ObservationErrorCode)`.
  - `ObservationValue` — fixed-point `{mantissa, scale}`.
  - `ObservationErrorCode` — `ResourceUnavailable | AuthFailed | Timeout |
    Missing | Other(u16)`.
    - מחלקות כשל: הוגן (`ResourceUnavailable | Timeout | Other`),
      תצורה שגויה (`AuthFailed`), חסר (`Missing`, ללא payload/שגיאת parse).
  - עזרי hash וחתימה: `ObservationBody::hash()` ו‑`Observation::hash()`.
- `ReportBody` — `{feed_id, feed_config_version, slot, request_hash, entries[],
  submitter}` כאשר `entries` מסודרות לפי `oracle_id`.
  - `ReportEntry` — `{oracle_id, observation_hash, value, outlier}`.
  - עזרי hash/חתימה: `ReportBody::hash()` ו‑`Report::hash()`.
- `FeedEventOutcome` — `Success { value, entries } | Error { code } | Missing`,
  נפלט כ‑`FeedEvent` `{feed_id, feed_config_version, slot, outcome}`.

### בקשות מחבר (OR-6)
- סכמת קאנון: `{feed_id, feed_config_version, slot, connector_id/version,
  method, endpoint, query: BTreeMap, headers: BTreeMap<String,
  RedactedHeaderValue>, body_hash}`.
- כותרות יכולות להיות `Plain` או `Hashed`; העדיפו `Hashed` עבור API keys כדי
  שסודות יישארו off‑chain. `validate_redaction` דוחה שמות כותרות רגישים
  (`authorization`, `cookie`, `x-api-*`, וכו') אלא אם הם hashed. `body_hash`
  מגבב את ה‑payload של הבקשה במקום לאחסן אותו.
- `ConnectorRequest::hash()` מניב את ה‑`request_hash` הקנוני שמפורסם ע"י
  observations/reports/events (דוגמת XOR/USD:
  `hash:26A12D920ACC7312746C7534926D971D58FF443C1345B9C14DAF5C3C5E3E6A69#D88C`).
- `ConnectorResponse` משקף את hash ה‑payload + קוד שגיאה אופציונלי כאשר מפעילים
  צריכים לארכב תגובות מחבר בלי לחשוף תוכן.
- סינון PII: השתמשו ב‑`KeyedHash { pepper_id, digest }` עבור מזהים חברתיים או
  PII אחרים, עם נגזרת `digest = Hash::new(pepper || payload)`; `KeyedHash::verify`
  בודק keyed hashes לביקורת תוך שמירת טקסט ברור מחוץ לשרשרת.

### Fixtures
דוגמאות fixtures נמצאות תחת `fixtures/oracle/`:
- `feed_config_price_xor_usd.json`
- `connector_request_price_xor_usd.json`
- `observation_price_xor_usd.json`
- `report_price_xor_usd.json`
- `feed_event_price_xor_usd.json`
- `feed_config_social_follow.json`
- `connector_request_social_follow.json`
- `observation_social_follow.json`
- `report_social_follow.json`
- `feed_event_social_follow.json`
- **Twitter binding registry:** פיד המעקב בטוויטר מאחסן כעת אישורי keyed‑hash
  באמצעות `RecordTwitterBinding` ותומך בניקוי ידני עם `RevokeTwitterBinding`.
  אישורים נשמרים תחת `HMAC(pepper, twitter_user_id||epoch)` → `{uaid, status, tweet_id,
  challenge_hash, expires_ms}` ללא PII גלוי; שאילתות משתמשות ב‑keyed hash דרך
  `FindTwitterBindingByHash`, וביטולים פולטים אירועים טיפוסיים למנויים.

#### זרימות תמריץ ויראלי (SOC-2)

לאחר שאישורי מעקב בטוויטר נוחתים ברג'יסטרי, חוזה התמריץ הוויראלי חושף סט קטן
של ISIs מעל binding keyed‑hash:

- `ClaimTwitterFollowReward { binding_hash }` משלם תגמול מוגדר ממאגר התמריצים
  הוויראלי לחשבון הקשור ל‑UAID (פעם אחת לכל binding, עם תקרה לכל UAID/יום ולכל
  binding), מעדכן `viral_daily_counters` / `viral_binding_claims`, ומחייב את
  התקציב היומי שמוגן ע"י `governance.viral_incentives`. אם escrow שנוצר קודם
  קיים לאותו binding, נתיב התגמול משחרר את ה‑escrow לחשבון UAID ומקליט בונוס
  שולח חד‑פעמי.
- `SendToTwitter { binding_hash, amount }` שולח את הסכום מיידית לחשבון UAID
  המשויך (כאשר אישור `Following` טרי קיים) או מפקיד את הכספים תחת `viral_escrows`
  עד שמופיע binding תואם, ואז `ClaimTwitterFollowReward` משחרר את ה‑escrow כחלק
  מנתיב התגמול.
- `CancelTwitterEscrow { binding_hash }` מאפשר לשולח להשיב escrow פתוח כאשר אין
  binding, בכפוף למתג `halt` הגלובלי ולרשימות חסימה בתצורה.

הממשל מגדיר את מאגר התמריצים, חשבון ה‑escrow, סכומי תגמול/בונוס, תקרות לכל UAID/יום,
תקרות לכל binding, תקציב יומי, ורשימות חסימה דרך מקטע `ViralIncentives` של
`iroha_config::parameters::Governance`. בקרות הקידום כוללות כעת `promo_starts_at_ms` /
`promo_ends_at_ms` (אכיפת חלון הן עבור שליחות והן עבור תביעות) ותקרת קמפיין כללית
`campaign_cap` שעוקבת אחר כל תגמול ובונוס שולח לאורך הקמפיין כולו. ניסיונות מחוץ
לחלון או לאחר מיצוי התקרה נדחים דטרמיניסטית לפני כל שינויי מצב. אירועי
`ViralRewardApplied` נושאים כעת את דגל הקידום הפעיל, דגל halt, תמונת תקציב הקמפיין
והתקרה המוגדרת, ומזינים את מדדי `iroha_social_campaign_*`/`iroha_social_promo_active`/
`iroha_social_halted` ואת הדשבורד הנלווה (`dashboards/grafana/social_follow_campaign.json`).
דגל `halt` מקפיא את כל הזרימות הוויראליות בלי לגעת ברג'יסטרי ה‑Twitter binding.

לצורכי tooling:
- ה‑CLI מציע תתי‑פקודות `iroha social claim-twitter-follow-reward|send-to-twitter|cancel-twitter-escrow`
  שמקבלות payload Norito JSON מסוג `KeyedHash` (binding hash) ולשליחות גם סכום
  `Numeric`, ואז בונות ושולחות את ההוראות המתאימות.
- ה‑JS SDK משקף עזרי אלו דרך
  `buildClaimTwitterFollowRewardInstruction`, `buildSendToTwitterInstruction`, ו‑
  `buildCancelTwitterEscrowInstruction`, שמקבלים את אותו מבנה keyed‑hash וכמויות
  ומחזירים אובייקטי instruction מוכנים ל‑Norito להכללה בעסקאות.

fixtures אלו משתמשים בצורת hash קנונית (`hash:...#...`), חתימות באותיות גדולות,
ומזהי ספק i105 שנגזרים ממפתחות ed25519 דטרמיניסטיים (למשל,
`sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D`).

ל‑feeds חברתיים/נושאי PII, `ObservationValue::from_hash`, `from_keyed_hash`,
ו‑`from_uaid` גוזרים ערכי fixed‑point דטרמיניסטיים ממזהים מגובבים כדי שערכת
המעקב החברתי תישאר בטוחה ל‑PII תוך שמירה על דטרמיניזם בין מאמתים. העזרים הללו
משאירים את ה‑scale באפס ומנקים את ה‑top bit של ה‑mantissa כדי להימנע מקידודים
שליליים כאשר ממירים hashes לערכים על‑שרשרת.

ערכות ייחוס לדוגמאות SDK/CLI נמצאות ב‑`crates/iroha_data_model/src/oracle/mod.rs::kits`.
הן טוענות את אותם fixtures כדי לספק חבילות `OracleKit` מוכנות עבור פיד מחיר XOR/USD
ופיד קישור המעקב בטוויטר, כך שדוגמאות קוד ובדיקות יישארו מיושרות עם payloads JSON
הקנוניים.
- ערכות ייחוס נוספות:
  - פיד `twitter_follow_binding`: `feed_config_twitter_follow.json`,
    `connector_request_twitter_follow.json`,
    `observation_twitter_follow.json`, `report_twitter_follow.json`,
    `feed_event_twitter_follow.json` מתעדים את מיפוי ה‑UAID keyed‑hash עבור
    אורקל המעקב בטוויטר (pepper `pepper-social-v1`, request hash
    `hash:25BD3C09F859A100396B5FD39066A3AEC5FB2EE6458D8FEDD0E403A35B5B3745#8EF1`).
  - ה‑fixtures הקיימים `price_xor_usd` רועננו כדי להתאים ל‑connector hash הנוכחי
    (`hash:59CFEA4268FB255E2FDB550B37CB83DF836D62744F835F121E5731AB62679BDB#844C`)
    ול‑observation/report/event digests כדי לשמור על תאימות בין SDKs.

### עזר צבירה

`aggregate_observations` בונה `ReportBody` יחד עם `FeedEventOutcome` תוך אכיפת
תקרות ומדיניות outlier:

```rust
let output = aggregate_observations(
    &feed_config,
    slot,
    request_hash,
    submitter_oracle_id,
    &observations,
)?;
assert!(matches!(output.outcome, FeedEventOutcome::Success(_)));
```

האימותים כוללים:
- הצמדה של feed/config/connector וה‑cadence (`validate_observation_meta`).
- עקביות slot + request‑hash, חברות ספקים, וזיהוי כפילויות.
- תקרות: `max_observers`, `max_value_len`, כפילויות oracle (דרך `validate_report_caps`).
- סימון outlier באמצעות `OutlierPolicy::Mad` או `OutlierPolicy::Absolute`; צבירה
  לפי median או percentile באמצעות `AggregationRule`.

שגיאות מוצגות כ‑`OracleAggregationError` כדי להתחבר לנתיבי admission על‑שרשרת.
כאשר כל התצפיות הן שגיאות (עם אותו קוד), התוצאה היא `FeedEventOutcome::Error`;
כאשר אין תצפיות, התוצאה היא `FeedEventOutcome::Missing`.

צרכני host: `iroha_core::oracle::OracleAggregator` ו‑`ObservationAdmission`
עוטפים את אותם עזרים עבור אימות על‑צמתי, הגנות replay, ויצירת דוח/תוצאה.

## בחירת ועדה ומנהיג (OR-2)

הגרלות ועדה דטרמיניסטיות ומוגבלות ל‑`(feed_id, feed_config_version, epoch,
validator_set_root, providers[])`. העזר
`derive_committee(feed_id, version, epoch, root, providers, committee_size)`
מחזיר `CommitteeDraw { seed, members[] }` שבו `members` הם הספקים בעלי הציון
הנמוך ביותר לאחר גיבוב `(seed || provider_id)` עם Blake2b‑256. ספקים כפולים
נמחקים לפני החישוב.

המנהיגים דטרמיניסטיים לכל slot:

```rust
let draw = derive_committee(...);
let leader = draw.leader_for_slot(slot);
```

אינדקס המנהיג נגזר מ‑`Hash(seed || slot)` כך שכל מאמת יכול לחשב את אותו submitter
עבור slot נתון ללא תיאום.

</div>
