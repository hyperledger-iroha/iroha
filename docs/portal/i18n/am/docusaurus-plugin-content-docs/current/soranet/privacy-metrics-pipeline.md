---
id: privacy-metrics-pipeline
lang: am
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# የሶራኔት የግላዊነት መለኪያዎች ቧንቧ መስመር

SNNet-8 ግላዊነትን የሚያውቅ የቴሌሜትሪ ገጽን ለሪሌይ አሂድ ጊዜ ያስተዋውቃል። የ
ቅብብል አሁን የመጨባበጥ እና የወረዳ ክስተቶችን ወደ ደቂቃ መጠን ባላቸው ባልዲዎች እና
ወደ ውጭ የሚላከው ግትር I18NT0000000X ቆጣሪዎችን ብቻ ነው ፣የግለሰብ ወረዳዎችን ይጠብቃል።
ለኦፕሬተሮች ሊተገበር የሚችል ታይነትን በሚሰጥበት ጊዜ ሊገናኝ የማይችል።

## የአሰባሳቢ አጠቃላይ እይታ

- የአሂድ ጊዜ ትግበራ በ I18NI0000024X ውስጥ ይኖራል
  `PrivacyAggregator`.
- ባልዲዎች በግድግዳ ሰዓት ደቂቃ (`bucket_secs`፣ ነባሪ 60 ሰከንድ) እና ተቆልፈዋል።
  በተከለከለ ቀለበት (`max_completed_buckets`፣ ነባሪ 120) ውስጥ ተከማችቷል። ሰብሳቢ
  ማጋራቶች የየራሳቸውን የታሰረ የኋላ መዝገብ ይይዛሉ (`max_share_lag_buckets`፣ ነባሪ 12)
  ስለዚህ የቆዩ የፕሪዮ መስኮቶች ከመፍሰስ ይልቅ እንደ የታፈኑ ባልዲዎች ይታጠባሉ።
  የማስታወስ ችሎታ ወይም መደበቅ የተጣበቁ ሰብሳቢዎች.
- `RelayConfig::privacy` ካርታዎች በቀጥታ ወደ I18NI0000030X ፣ ማስተካከያን ያጋልጣል
  ማዞሪያዎች (`bucket_secs`፣ `min_handshakes`፣ `flush_delay_buckets`፣
  `force_flush_buckets`፣ `max_completed_buckets`፣ `max_share_lag_buckets`፣
  `expected_shares`)። የምርት አሂድ ጊዜ SNNet-8a እያለ ነባሪዎቹን ይጠብቃል።
  አስተማማኝ የመደመር ገደቦችን ያስተዋውቃል።
- የሩጫ ጊዜ ሞጁሎች ክስተቶችን በተተየቡ ረዳቶች ይመዘግባሉ፡-
  `record_circuit_accepted`፣ `record_circuit_rejected`፣ `record_throttle`፣
  `record_throttle_cooldown`፣ `record_capacity_reject`፣ `record_active_sample`፣
  `record_verified_bytes`፣ እና `record_gar_category`።

## የማስተላለፊያ አስተዳዳሪ የመጨረሻ ነጥብ

ኦፕሬተሮች ለጥሬ ምልከታዎች የአስተዳዳሪውን አድማጭ በ በኩል ሊጠይቁ ይችላሉ።
`GET /privacy/events`. የመጨረሻው ነጥብ በአዲስ መስመር የተወሰነ JSON ይመልሳል
(`application/x-ndjson`) `SoranetPrivacyEventV1` የመስታወት ጭነቶች የያዘ
ከውስጥ `PrivacyEventBuffer`. ቋት አዲሶቹን ክስተቶች ያስቀምጣል።
ወደ `privacy.event_buffer_capacity` ግቤቶች (ነባሪ 4096) እና ፈሰሰ
አንብብ፣ ስለዚህ ቧጨራዎች ክፍተቶችን ለማስወገድ ብዙ ጊዜ በቂ ድምጽ መስጠት አለባቸው። ክስተቶች ይሸፍናሉ
ተመሳሳይ መጨባበጥ፣ ስሮትል፣ የተረጋገጠ የመተላለፊያ ይዘት፣ የነቃ ወረዳ እና የጋር ምልክቶች
የ Prometheus ቆጣሪዎችን የሚያበረታታ፣ የታችኛው ተፋሰስ ሰብሳቢዎች በማህደር እንዲቀመጡ ያስችላቸዋል።
ግላዊነት-አስተማማኝ የዳቦ ፍርፋሪ ወይም ደህንነቱ የተጠበቀ ድምር የስራ ፍሰቶችን ይመግቡ።

## የማስተላለፊያ ውቅር

ኦፕሬተሮች በሪሌይ ውቅር ፋይል ውስጥ የግላዊነት የቴሌሜትሪ ቃላቶችን ያስተካክላሉ
የ `privacy` ክፍል:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

የመስክ ነባሪዎች ከ SNNet-8 ዝርዝር ጋር ይዛመዳሉ እና በሚጫኑበት ጊዜ የተረጋገጡ ናቸው፡

| መስክ | መግለጫ | ነባሪ |
|-------|-------------|--------|
| `bucket_secs` | የእያንዳንዱ የውህደት መስኮት (ሰከንዶች) ስፋት. | `60` |
| `min_handshakes` | አንድ ባልዲ ቆጣሪዎችን ከማውጣቱ በፊት ዝቅተኛው የአስተዋጽዖ አድራጊ ቆጠራ። | `12` |
| `flush_delay_buckets` | ማጠብ ከመሞከርዎ በፊት ለመጠበቅ የተጠናቀቁ ባልዲዎች። | `1` |
| `force_flush_buckets` | የታፈነ ባልዲ ከመልቀቃችን በፊት ከፍተኛው ዕድሜ። | `6` |
| `max_completed_buckets` | የተቀመጠ ባልዲ የኋላ መዝገብ (ያልተገደበ ማህደረ ትውስታን ይከላከላል)። | `120` |
| `max_share_lag_buckets` | ከመጨቆኑ በፊት ለሰብሳቢ አክሲዮኖች የማቆያ መስኮት። | `12` |
| `expected_shares` | ከመቀላቀል በፊት የፕሪዮ ሰብሳቢ ማጋራቶች ያስፈልጋሉ። | `2` |
| `event_buffer_capacity` | ለአስተዳዳሪው ዥረት የNDJSON ክስተት የኋላ መዝገብ። | `4096` |

`force_flush_buckets` ከ I18NI0000069X ዝቅ በማድረግ ማዋቀር
ገደቦች፣ ወይም የማቆያ ጠባቂውን ማሰናከል አሁን ላለማረጋገጥ ማረጋገጫ ወድቋል
በሪሌይ ቴሌሜትሪ የሚያፈስ ማሰማራቶች።

የ`event_buffer_capacity` ገደብ `/admin/privacy/events`ን ይገድባል
ቧጨራዎች ላልተወሰነ ጊዜ ወደ ኋላ ሊወድቁ አይችሉም።

## ፕሪዮ ሰብሳቢ ማጋራቶች

SNNet-8a ሚስጥራዊ የተጋሩ የPrio ባልዲዎችን የሚያመነጩ ድርብ ሰብሳቢዎችን ያሰማራል። የ
ኦርኬስትራ አሁን የ`/privacy/events` NDJSON ዥረት ለሁለቱም ይተነትናል
`SoranetPrivacyEventV1` ግቤቶች እና I18NI0000074X ማጋራቶች፣
ወደ I18NI0000075X በማስተላለፍ ላይ። ባልዲዎች ይለቃሉ
አንዴ `PrivacyBucketConfig::expected_shares` አስተዋጽዖዎች ሲደርሱ፣ በማንጸባረቅ
የማስተላለፊያ ባህሪ. አክሲዮኖች ለባልዲ አሰላለፍ እና ለሂስቶግራም ቅርፅ የተረጋገጡ ናቸው።
ወደ I18NI0000077X ከመዋሃድ በፊት. ከተጣመረ
የመጨባበጥ ብዛት ከ I18NI0000078X በታች ይወርዳል፣ ባልዲው ወደ ውጭ ይላካል እንደ
`suppressed`፣ የውስጠ-ቅብብሎሽ ሰብሳቢውን ባህሪ የሚያንፀባርቅ። ታፍኗል
ዊንዶውስ ኦፕሬተሮች መለየት እንዲችሉ አሁን I18NI0000080X መለያ ይለቃሉ
በ`insufficient_contributors`፣ `collector_suppressed` መካከል፣
`collector_window_elapsed`፣ እና `forced_flush_window_elapsed` ሲናሪዮስ
የቴሌሜትሪ ክፍተቶችን መመርመር. የ `collector_window_elapsed` ምክንያት እንዲሁ ይቃጠላል።
የፕሪዮ አክሲዮኖች ከ `max_share_lag_buckets` ሲዘገዩ ፣የተጣበቁ ሰብሳቢዎችን በማድረግ
በማህደረ ትውስታ ውስጥ የቆዩ ክምችቶችን ሳይለቁ ይታያል.

## Torii ማስገቢያ የመጨረሻ ነጥቦች

Torii አሁን ሁለት የቴሌሜትሪ-ጌድ የኤችቲቲፒ ማለቂያ ነጥቦችን ያጋልጣል ስለዚህ ቅብብሎሽ እና ሰብሳቢዎች
የታሰበ መጓጓዣ ሳይጨምር ምልከታዎችን ማስተላለፍ ይችላል፡-

- `POST /v2/soranet/privacy/event` አንድ ይቀበላል
  `RecordSoranetPrivacyEventDto` ጭነት. የሰውነት መጠቅለያዎች ሀ
  `SoranetPrivacyEventV1` እና አማራጭ I18NI0000090X መለያ። Torii ያረጋግጣል
  ንቁ በሆነው የቴሌሜትሪ ፕሮፋይል ላይ መጠየቅ፣ክስተቱን መዝግቦ ምላሽ ይሰጣል
  በ HTTP `202 Accepted` ከ Norito JSON ኤንቨሎፕ ጋር
  የተሰላ ባልዲ መስኮት (`bucket_start_unix`፣ `bucket_duration_secs`) እና
  የማስተላለፊያ ሁነታ.
- `POST /v2/soranet/privacy/share` I18NI0000095X ይቀበላል
  ጭነት. አካሉ I18NI0000096X እና አማራጭን ይይዛል
  ኦፕሬተሮች ሰብሳቢ ፍሰቶችን ኦዲት ማድረግ እንዲችሉ `forwarded_by` ፍንጭ ይሰጣል። ስኬታማ
  ማስረከቦች HTTP I18NI0000098X ከ Norito JSON ፖስታ ማጠቃለያ ጋር ይመልሳል
  ሰብሳቢው, ባልዲ መስኮት እና የጭቆና ፍንጭ; የማረጋገጫ ውድቀቶች ካርታ ወደ
  የቴሌሜትሪ `Conversion` ምላሽ የመወሰን የስህተት አያያዝን ለመጠበቅ
  በመላ ሰብሳቢዎች. የኦርኬስትራ ክስተት ምልልስ አሁን እነዚህን ማጋራቶች እንደ እሱ ይለቃል
  የምርጫ ቅብብሎሽ፣ የTorii's Prio accumulatorን ከቅብብል ባልዲዎች ጋር በማመሳሰል።

ሁለቱም የመጨረሻ ነጥቦች የቴሌሜትሪ መገለጫን ያከብራሉ፡ `503 አገልግሎትን ይለቃሉ
መለኪያዎች ሲሰናከሉ አይገኙም። ደንበኞች Norito ሁለትዮሽ መላክ ይችላሉ።
(`application/x.norito`) ወይም Norito JSON (`application/x.norito+json`) አካላት;
አገልጋዩ በራስ ሰር ቅርጸቱን በመደበኛ Torii በኩል ይደራደራል።
ኤክስትራክተሮች.

## Prometheus መለኪያዎች

እያንዳንዱ ወደ ውጭ የተላከ ባልዲ `mode` (`entry`፣ `middle`፣ `exit`) እና ይይዛል።
`bucket_start` መለያዎች። የሚከተሉት ልኬት ቤተሰቦች ይለቃሉ፡-

| መለኪያ | መግለጫ |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | የመጨባበጥ ታክሶኖሚ በ`kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`። |
| `soranet_privacy_throttles_total{scope}` | ስሮትል ቆጣሪዎች ከ `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ጋር። |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | የተዋሃዱ የማቀዝቀዝ ቆይታዎች በተጨናነቀ የእጅ መጨባበጥ አስተዋጽዖ አድርገዋል። |
| `soranet_privacy_verified_bytes_total` | ከዓይነ ስውራን የመለኪያ ማረጋገጫዎች የተረጋገጠ የመተላለፊያ ይዘት። |
| `soranet_privacy_active_circuits_{avg,max}` | አማካኝ እና ከፍተኛ ንቁ ወረዳዎች በባልዲ። |
| `soranet_privacy_rtt_millis{percentile}` | RTT ፐርሰንታይል ግምቶች (`p50`፣ `p90`፣ `p99`)። |
| `soranet_privacy_gar_reports_total{category_hash}` | Hashed Governance Action Report ቆጣሪዎች በምድብ ዳይስት ተከፍተዋል። |
| `soranet_privacy_bucket_suppressed` | የአስተዋጽዖ አድራጊው ገደብ ስላልተሟላ ባልዲዎች ታግደዋል። |
| `soranet_privacy_pending_collectors{mode}` | የሰብሳቢ መጋራት ክምችት በመጠባበቅ ላይ ያለ ጥምር፣ በሪሌይ ሁነታ ተመድቦ። |
| `soranet_privacy_suppression_total{reason}` | ዳሽቦርዶች የግላዊነት ክፍተቶችን እንዲለዩ ከ`reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` ጋር የታፈኑ ባልዲ ቆጣሪዎች። |
| `soranet_privacy_snapshot_suppression_ratio` | የመጨረሻው የውሃ ፍሳሽ የተጨቆነ/የፈሰሰው ጥምርታ (0-1)፣ ለማንቂያ በጀቶች ጠቃሚ። |
| `soranet_privacy_last_poll_unixtime` | የ UNIX የጊዜ ማህተም በጣም የቅርብ ጊዜ የተሳካ የህዝብ አስተያየት ( ሰብሳቢ-ስራ ፈት ማንቂያን ያንቀሳቅሳል)። |
| `soranet_privacy_collector_enabled` | ግላዊነት ሰብሳቢው ሲሰናከል ወይም መጀመር ሲያቅተው ወደ `0` የሚገለባበጥ መለኪያ (የሰብሳቢው ተሰናክሏል ማንቂያውን ያንቀሳቅሳል)። |
| `soranet_privacy_poll_errors_total{provider}` | የድምጽ መስጫ ውድቀቶች በሬሌይ ተለዋጭ ስም (ስህተቶችን የመፍታታት ጭማሪ፣ የኤችቲቲፒ ውድቀቶች ወይም ያልተጠበቁ የሁኔታ ኮዶች)። |

ትዝብት የሌላቸው ባልዲዎች ጸጥ ይላሉ፣ ዳሽቦርዶችን ያለ ንጽህና መጠበቅ
በዜሮ የተሞሉ መስኮቶችን ማምረት.

## የተግባር መመሪያ

1. ** ዳሽቦርዶች *** - ከላይ ያሉትን መለኪያዎች በ `mode` እና I18NI0000129X ተመድበው ያቅርቡ።
   የጎደሉ መስኮቶችን ወደ ላይ ላዩን ሰብሳቢ ወይም አስተላላፊ ጉዳዮች ያድምቁ። ተጠቀም
   አስተዋጽዖ አበርካች ለመለየት `soranet_privacy_suppression_total{reason}`
   ክፍተቶችን በሚለዩበት ጊዜ ሰብሳቢ-ተኮር ማፈን ጉድለቶች። Grafana
   ንብረት አሁን የተወሰነ **“የማፈኛ ምክንያቶች (5ሜ)”** ፓነል በእነዚያ ይመገባል።
   ቆጣሪዎች ሲደመር **"የታፈነ ባልዲ %"** ስሌት
   `sum(soranet_privacy_bucket_suppressed) / count(...)` በአንድ ምርጫ እንዲሁ
   ኦፕሬተሮች የበጀት ጥሰቶችን በጨረፍታ ማየት ይችላሉ። የ ** ሰብሳቢ አጋራ
   የኋላ ታሪክ** ተከታታይ (`soranet_privacy_pending_collectors`) እና ** ቅጽበታዊ እይታ
   የማፈኛ ሬሾ *** ስታቲስቲክስ የተጣበቁ ሰብሳቢዎችን እና የበጀት መንሸራተትን ያደምቃል
   አውቶማቲክ ሩጫዎች.
2. ** ማንቂያ *** - ማንቂያዎችን ከግላዊነት-አስተማማኝ ቆጣሪዎች ያሽከርክሩ፡ PoW ፍንጮችን ውድቅ ያደርጋል፣
   የማቀዝቀዝ ድግግሞሽ፣ የአርቲቲ ተንሳፋፊ እና አቅም ውድቅ ያደርጋል። ምክንያቱም ቆጣሪዎች ናቸው
   በእያንዳንዱ ባልዲ ውስጥ ሞኖቶኒክ ፣ ቀጥተኛ ተመን ላይ የተመሰረቱ ህጎች በደንብ ይሰራሉ።
3. **የአደጋ ምላሽ** - በመጀመሪያ በተዋሃደ መረጃ ላይ መታመን። ጥልቅ ማረም ሲደረግ
   አስፈላጊ ነው፣ የባልዲ ቅጽበተ-ፎቶዎችን እንደገና ለማጫወት ወይም ዓይነ ስውራንን ለመመርመር ሪሌይቶችን ይጠይቁ
   ጥሬ የትራፊክ ምዝግብ ማስታወሻዎችን ከመሰብሰብ ይልቅ የመለኪያ ማረጋገጫዎች.
4. **ማቆየት** - ከመጠን በላይ እንዳይሆን ብዙ ጊዜ መቧጨር
   `max_completed_buckets`. ላኪዎች የ I18NT0000003X ውፅዓትን እንደ
   ቀኖናዊ ምንጭ እና መጣል የአካባቢ ባልዲዎች አንዴ ከተላለፉ።

## የማፈን ትንታኔ እና አውቶሜትድ ሩጫዎችየSNNet-8 ተቀባይነት አውቶማቲክ ሰብሳቢዎች እንደሚቆዩ በማሳየት ላይ ያተኩራል።
ጤናማ እና ያ ማፈኛ በፖሊሲ ወሰኖች ውስጥ ይቆያል (≤10% ባልዲዎች በአንድ
በማንኛውም የ 30 ደቂቃ መስኮት ላይ ማስተላለፍ). አሁን ያንን በር ለማርካት የሚያስፈልገው መሳሪያ
ከዛፉ ጋር መርከቦች; ኦፕሬተሮች በየሳምንቱ የአምልኮ ሥርዓታቸው ውስጥ ማስገባት አለባቸው. አዲሱ
Grafana የማፈኛ ፓነሎች ከዚህ በታች ያሉትን የPromQL ቅንጥቦች ያንፀባርቃሉ፣ ለጥሪም ይሰጣሉ።
ወደ ማንዋል መጠይቆች ተመልሰው መውደቅ ከመፈለጋቸው በፊት ቡድኖች ታይነት ይኖራሉ።

### PromQL የምግብ አዘገጃጀት መመሪያዎች ለማፈን ግምገማ

ኦፕሬተሮች የሚከተሉትን የPromQL ረዳቶች በእጅ መያዝ አለባቸው። ሁለቱም ተጠቃሾች ናቸው።
በተጋራው Grafana ዳሽቦርድ (`dashboards/grafana/soranet_privacy_metrics.json`)
እና የማስጠንቀቂያ አስተዳዳሪ ደንቦች፡-

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

**"የታፈነ ባልዲ %"** ስታቲስቲክስ ከታች እንዳለ ለማረጋገጥ የሬሾውን ውጤት ይጠቀሙ
የፖሊሲው በጀት; ለፈጣን ግብረመልስ የሾል ማወቂያውን ወደ Alertmanager ያሽከርክሩ
አስተዋጽዖ አበርካች ሲቆጠር ሳይታሰብ ይንከሩ።

### ከመስመር ውጭ ባልዲ ሪፖርት CLI

የስራ ቦታው `cargo xtask soranet-privacy-report` ለአንድ ጊዜ NDJSON ያጋልጣል
ይይዛል። አንድ ወይም ከዚያ በላይ የአስተዳዳሪ ወደ ውጭ መላክ ላይ ያመልክቱ፦

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

ረዳቱ ቀረጻውን በ`SoranetSecureAggregator` በኩል ያሰራጫል፣ ያትማል ሀ
የማፈን ማጠቃለያ ለ stdout፣ እና እንደ አማራጭ የተዋቀረ የJSON ሪፖርት ይጽፋል
በ `--json-out <path|->` በኩል. ልክ እንደ ቀጥታ ሰብሳቢው ተመሳሳይ ጉብታዎችን ያከብራል።
(`--bucket-secs`፣ `--min-contributors`፣ `--expected-shares`፣ ወዘተ)፣ መፍቀድ
ኦፕሬተሮች በሚለዩበት ጊዜ ታሪካዊ ቀረጻዎችን በተለያዩ ደረጃዎች ይጫወታሉ
ጉዳይ ። JSON ን ከGrafana ቅጽበታዊ ገጽ እይታዎች ጋር ያያይዙ ስለዚህ SNNet-8
የማፈን ትንታኔ በር ለኦዲት መደረጉ ይቆያል።

### የመጀመሪያው አውቶማቲክ አሂድ ማረጋገጫ ዝርዝር

አስተዳደር አሁንም የመጀመሪያው አውቶሜሽን ሩጫውን ማሟላቱን ማረጋገጥን ይጠይቃል
የማፈን በጀት. ረዳቱ አሁን `--max-suppression-ratio <0-1>` እንዲሁ ይቀበላል
የታፈኑ ባልዲዎች ከተፈቀደው በላይ በሆነ ቁጥር CI ወይም ኦፕሬተሮች በፍጥነት ሊወድቁ ይችላሉ።
መስኮት (ነባሪ 10%) ወይም ገና ምንም ባልዲዎች በማይገኙበት ጊዜ። የሚመከር ፍሰት፡

1. NDJSONን ከሪሌይ አስተዳዳሪ የመጨረሻ ነጥብ(ዎች) እና ኦርኬስትራውን ወደ ውጭ ላክ
   `/v2/soranet/privacy/event|share` ዥረት ወደ ውስጥ
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. ረዳቱን በፖሊሲ በጀት ያሂዱ፡-

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   ትዕዛዙ የተመለከተውን ጥምርታ ያትማል እና በጀቱ ሲሆን ከዜሮ ውጭ ይወጣል
   ቴሌሜትሪ እንደሌለው የሚጠቁም ባልዲዎች ካልተዘጋጁ ** ወይም *** አልፏል
   ገና ለሩጫ ተዘጋጅቷል. የቀጥታ መለኪያዎች መታየት አለባቸው
   `soranet_privacy_pending_collectors` ወደ ዜሮ እየፈሰሰ ነው።
   `soranet_privacy_snapshot_suppression_ratio` በተመሳሳይ በጀት መቆየት
   ሩጫው ሲሰራ.
3. ከዚህ በፊት የJSON ውፅዓት እና የ CLI ምዝግብ ማስታወሻን ከSNNet-8 የማስረጃ ጥቅል ጋር በማህደር ያስቀምጡ
   ገምጋሚዎች ትክክለኛ ቅርሶችን እንደገና ማጫወት እንዲችሉ የትራንስፖርት ነባሪውን በመገልበጥ።

## ቀጣይ ደረጃዎች (SNNet-8a)

- ድርብ ፕሪዮ ሰብሳቢዎችን ያዋህዱ ፣ የእነሱን ድርሻ ወደ ውስጥ በማጣመር
  Runtime ስለዚህ ቅብብሎሽ እና ሰብሳቢዎች ወጥነት ያለው `SoranetPrivacyBucketMetricsV1` ያመነጫሉ።
  ሸክሞች. * (ተከናውኗል - `ingest_privacy_payload` ውስጥ ይመልከቱ
  `crates/sorafs_orchestrator/src/lib.rs` እና ተጓዳኝ ሙከራዎች።)*
- የተጋራውን I18NT0000004X ዳሽቦርድ JSON ያትሙ እና የሚሸፍኑ የማስጠንቀቂያ ደንቦችን ያትሙ
  የማፈን ክፍተቶች፣ የሰብሳቢ ጤና እና ማንነትን መደበቅ ቡኒዎች። * (ተከናውኗል - ተመልከት
  `dashboards/grafana/soranet_privacy_metrics.json`፣
  `dashboards/alerts/soranet_privacy_rules.yml`፣
  `dashboards/alerts/soranet_policy_rules.yml` እና የማረጋገጫ ዕቃዎች።)*
- በ ውስጥ የተገለጹትን የልዩነት-የግላዊነት መለኪያ ቅርሶችን ያመርቱ
  `privacy_metrics_dp.md`፣ ሊባዙ የሚችሉ ደብተሮችን እና አስተዳደርን ጨምሮ
  መፈጨት. * (ተከናውኗል - ማስታወሻ ደብተር + የመነጩ ቅርሶች
  `scripts/telemetry/run_privacy_dp.py`; CI መጠቅለያ
  `scripts/telemetry/run_privacy_dp_notebook.sh` የማስታወሻ ደብተሩን በ
  `.github/workflows/release-pipeline.yml` የስራ ፍሰት; የአስተዳደር መግለጫ ገብቷል።
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

የአሁኑ ልቀት የ SNNet-8 መሠረት ያቀርባል፡ ቆራጥ፣
ወደ ነባር Prometheus ጥራጊዎች በቀጥታ የሚያስገባ የግላዊነት-አስተማማኝ ቴሌሜትሪ
እና ዳሽቦርዶች. የልዩነት የግላዊነት ልኬት ቅርሶች በቦታቸው ይገኛሉ፣ የ
የቧንቧ መስመር ዝርጋታ የማስታወሻ ደብተሩን ትኩስ እና የቀረውን ያቆያል
ስራው የመጀመሪያውን አውቶሜትድ አሂድ እና ጭቆናን በማራዘም ላይ ያተኩራል።
የማንቂያ ትንተና.