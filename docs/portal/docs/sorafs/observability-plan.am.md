---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab50f11b7d9ed6af7d0b1d14f756921ed8cb0798e32a7ea0b109b564c859bdba
source_last_modified: "2026-01-21T19:17:13.231946+00:00"
translation_last_reviewed: 2026-02-07
id: observability-plan
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# አላማዎች
- ለበረኛ መንገዶች፣ ኖዶች እና የባለብዙ ምንጭ ኦርኬስትራ መለኪያዎችን እና የተዋቀሩ ክስተቶችን ይግለጹ።
- Grafana ዳሽቦርዶችን፣ የማንቂያ ጣራዎችን እና የማረጋገጫ መንጠቆዎችን ያቅርቡ።
- ከስህተት-በጀት እና ትርምስ-ቁፋሮ ፖሊሲዎች ጎን ለጎን የ SLO ኢላማዎችን ማቋቋም።

## ሜትሪክ ካታሎግ

### የጌትዌይ ወለሎች

| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `sorafs_gateway_active` | መለኪያ (UpdownCounter) | `endpoint`፣ `method`፣ `variant`፣ `chunker`፣ `profile` | በ `SorafsGatewayOtel` በኩል የወጣ; በበረራ ላይ የኤችቲቲፒ ስራዎችን በየመጨረሻ ነጥብ/ዘዴ ጥምር ይከታተላል። |
| `sorafs_gateway_responses_total` | ቆጣሪ | `endpoint`፣ `method`፣ `variant`፣ `chunker`፣ `profile`፣ `result`፣ Prometheus እያንዳንዱ የተጠናቀቀ መግቢያ ጥያቄ አንድ ጊዜ ይጨምራል። `result` ∈ {`success`፣`error`፣`dropped`}። |
| `sorafs_gateway_ttfb_ms_bucket` | ሂስቶግራም | `endpoint`፣ `method`፣ `variant`፣ `chunker`፣ `profile`፣ `result`፣ Prometheus ለመግቢያ ምላሾች ከጊዜ-ወደ-መጀመሪያ-ባይት መዘግየት; እንደ Prometheus `_bucket/_sum/_count` ተልኳል። |
| `sorafs_gateway_proof_verifications_total` | ቆጣሪ | `profile_version`፣ `result`፣ `error_code` | በጥያቄ ጊዜ (`result` ∈ {`success`፣`failure`}) የተያዙ የማረጋገጫ ውጤቶች። |
| `sorafs_gateway_proof_duration_ms_bucket` | ሂስቶግራም | `profile_version`፣ `result`፣ `error_code` | ለPoR ደረሰኞች የቆይታ ጊዜ ስርጭትን ማረጋገጥ። |
| `telemetry::sorafs.gateway.request` | የተዋቀረ ክስተት | `endpoint`፣ `method`፣ `variant`፣ `result`፣ `status`፣ `error_code`፣ `duration_ms` | በእያንዳንዱ የሎኪ/የጊዜ ማዛመድ ጥያቄ ላይ የተዋቀረ መዝገብ ይወጣል። |

`telemetry::sorafs.gateway.request` ክስተቶች የ OTEL ቆጣሪዎችን በተዋቀሩ የክፍያ ጭነቶች ያንጸባርቃሉ፣ `endpoint`፣ `method`፣ `variant`፣ `status`፣ Prometheus ዳሽቦርዶች የ OTLP ተከታታዮችን ለSLO ክትትል ሲጠቀሙ የሎኪ/የጊዜ ግንኙነት።

### የጤና ማረጋገጫ ቴሌሜትሪ| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `torii_sorafs_proof_health_alerts_total` | ቆጣሪ | `provider_id`፣ `trigger`፣ `penalty` | `RecordCapacityTelemetry` `SorafsProofHealthAlert` በሚያወጣ ቁጥር ይጨምራል። `trigger` PDP/PoTR/ሁለቱንም ውድቀቶች የሚለይ ሲሆን `penalty` ዋስትና በመቀዝቀዝ የተቀነሰ ወይም የታፈነ መሆኑን ያሳያል። |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | መለኪያ | `provider_id` | የቅርብ ጊዜ የPDP/PoTR ቆጠራዎች በአጥቂው የቴሌሜትሪ መስኮት ውስጥ ሪፖርት የተደረጉ ሲሆን ቡድኖቹ አቅራቢዎች ፖሊሲውን እስከምን ድረስ እንደደረሱ ለመለካት። |
| `torii_sorafs_proof_health_penalty_nano` | መለኪያ | `provider_id` | በመጨረሻው ማንቂያ ላይ የናኖ-ኤክስኦር መጠን ቀንሷል (የማቀዝቀዝ ሂደት ሲታፈን ዜሮ)። |
| `torii_sorafs_proof_health_cooldown` | መለኪያ | `provider_id` | የቦሊያን መለኪያ (`1` = በማቀዝቀዝ የታፈነ ማንቂያ) የክትትል ማንቂያዎች ለጊዜው ድምጸ-ከል ሲደረግ። |
| `torii_sorafs_proof_health_window_end_epoch` | መለኪያ | `provider_id` | ኦፕሬተሮች ከNorito ቅርሶች ጋር ማዛመድ እንዲችሉ ኢፖክ ከማንቂያው ጋር ታስሮ ለቴሌሜትሪ መስኮት ተመዝግቧል። |

እነዚህ ምግቦች አሁን የታይካይ መመልከቻ ዳሽቦርድ የማረጋገጫ-ጤና ረድፍን ያጎላሉ
(`dashboards/grafana/taikai_viewer.json`)፣ ለCDN ኦፕሬተሮች የቀጥታ ታይነትን መስጠት
ወደ ማንቂያ ጥራዞች፣ PDP/PoTR ቀስቅሴ ቅይጥ፣ ቅጣቶች እና የማቀዝቀዝ ሁኔታ በ
አቅራቢ.

ተመሳሳይ መለኪያዎች አሁን ሁለት የታይካይ ተመልካች ማንቂያ ደንቦችን ይመለሳሉ፡-
`SorafsProofHealthPenalty` በማንኛውም ጊዜ ይቃጠላል።
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` ይጨምራል
የመጨረሻዎቹ 15 ደቂቃዎች፣ `SorafsProofHealthCooldown` ማስጠንቀቂያ ሲሰጥ ሀ
አቅራቢው በማቀዝቀዣው ውስጥ ለአምስት ደቂቃዎች ይቆያል. ሁለቱም ማንቂያዎች ይኖራሉ
`dashboards/alerts/taikai_viewer_rules.yml` ስለዚህ SREዎች ወዲያውኑ አውድ ይቀበላሉ።
የPoR/PoTR ተፈጻሚነት ሲጨምር።

### የኦርኬስትራ መሬቶች| መለኪያ / ክስተት | አይነት | መለያዎች | አዘጋጅ | ማስታወሻ |
|------------|-------|----------|-------|
| `sorafs_orchestrator_active_fetches` | መለኪያ | `manifest_id`, `region` | `FetchMetricsCtx` | ክፍለ-ጊዜዎች በአሁኑ ጊዜ በበረራ ላይ። |
| `sorafs_orchestrator_fetch_duration_ms` | ሂስቶግራም | `manifest_id`, `region` | `FetchMetricsCtx` | የቆይታ ጊዜ ሂስቶግራም በሚሊሰከንዶች; 1ms→30s ባልዲዎች። |
| `sorafs_orchestrator_fetch_failures_total` | ቆጣሪ | `manifest_id`፣ `region`፣ `reason` | `FetchMetricsCtx` | ምክንያቶች: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | ቆጣሪ | `manifest_id`፣ `provider_id`፣ `reason` | `FetchMetricsCtx` | የድጋሚ መሞከር መንስኤዎችን (`retry`፣ `digest_mismatch`፣ `length_mismatch`፣ `provider_error`) ይለያል። |
| `sorafs_orchestrator_provider_failures_total` | ቆጣሪ | `manifest_id`፣ `provider_id`፣ `reason` | `FetchMetricsCtx` | የክፍለ-ደረጃ የአካል ጉዳተኝነት/የሽንፈት ቁመትን ይይዛል። |
| `sorafs_orchestrator_chunk_latency_ms` | ሂስቶግራም | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ለግዜ/SLO ትንተና በየ ቸንክ ማምጣት መዘግየት ስርጭት (ሚሴ)። |
| `sorafs_orchestrator_bytes_total` | ቆጣሪ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ባይት በአንፀባራቂ/አቅራቢው ይላካል; በPromQL ውስጥ በ`rate()` በኩል የማስተላለፊያ ዘዴን ያግኙ። |
| `sorafs_orchestrator_stalls_total` | ቆጣሪ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ከ`ScoreboardConfig::latency_cap_ms` በላይ የሆኑ ቁርጥራጮችን ይቆጥራል። |
| `telemetry::sorafs.fetch.lifecycle` | የተዋቀረ ክስተት | `manifest`፣ `region`፣ `job_id`፣ `event`፣ `status`፣ `chunk_count`፣ Prometheus፣ `total_bytes`100 `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | መስተዋቶች የስራ የህይወት ኡደት (ጅምር/ሙሉ) ከ Norito JSON ክፍያ ጋር። |
| `telemetry::sorafs.fetch.retry` | የተዋቀረ ክስተት | `manifest`፣ `region`፣ `job_id`፣ `provider`፣ `reason`፣ `attempts` | `FetchTelemetryCtx` | በእያንዳንዱ አቅራቢ የወጣ ድጋሚ ሙከራ `attempts` ተጨማሪ ሙከራዎችን ይቆጥራል (≥1)። |
| `telemetry::sorafs.fetch.provider_failure` | የተዋቀረ ክስተት | `manifest`፣ `region`፣ `job_id`፣ `provider`፣ `reason`፣ `failures` | `FetchTelemetryCtx` | አቅራቢው የውድቀቱን ገደብ ሲያቋርጥ ተሸፍኗል። |
| `telemetry::sorafs.fetch.error` | የተዋቀረ ክስተት | `manifest`፣ `region`፣ `job_id`፣ `reason`፣ `provider?`፣ `provider_reason?`፣ `duration_ms` | `FetchTelemetryCtx` | የተርሚናል አለመሳካት መዝገብ፣ ለሎኪ/ስፕሉክ ማስመጣት ተስማሚ። |
| `telemetry::sorafs.fetch.stall` | የተዋቀረ ክስተት | `manifest`፣ `region`፣ `job_id`፣ `provider`፣ `latency_ms`፣ `bytes` | `FetchTelemetryCtx` | የተቆረጠ መዘግየት የተዋቀረውን ቆብ (የመስታወት መሸጫ ቆጣሪዎች) ሲጥስ ይነሳል። |

### መስቀለኛ መንገድ / የሚባዙ ወለሎች| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `sorafs_node_capacity_utilisation_pct` | ሂስቶግራም | `provider_id` | የ OTEL ሂስቶግራም የማከማቻ አጠቃቀም መቶኛ (እንደ `_bucket/_sum/_count` ወደ ውጭ የተላከ)። |
| `sorafs_node_por_success_total` | ቆጣሪ | `provider_id` | ለስኬታማ የPoR ናሙናዎች ሞኖቶኒክ ቆጣሪ፣ ከመርሐግብር አውጪ ቅጽበተ-ፎቶዎች የተገኘ። |
| `sorafs_node_por_failure_total` | ቆጣሪ | `provider_id` | ላልተሳካ የPoR ናሙናዎች ሞኖቶኒክ ቆጣሪ። |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | መለኪያ | `provider` | ያገለገሉ ባይት የነባር የPrometheus መለኪያዎች፣ የወረፋ ጥልቀት፣ የPoR inflight ብዛት። |
| `torii_sorafs_capacity_*`፣ `torii_sorafs_uptime_bps`፣ `torii_sorafs_por_bps` | መለኪያ | `provider` | በአቅም ዳሽቦርድ ውስጥ የአቅራቢ አቅም/የጊዜ ስኬት ውሂብ ብቅ አለ። |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | መለኪያ | `provider`, `manifest` | የኋላ መዝገብ ጥልቀት እና `/v2/sorafs/por/ingestion/{manifest}` ድምጽ በተሰጠ ቁጥር ወደ ውጭ የሚላኩት ድምር ውድቀት ቆጣሪዎች የ"PoR Stalls" ፓኔል/ማንቂያን ይመገባሉ። |

### ጥገና & SLA

| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `sorafs_repair_tasks_total` | ቆጣሪ | `status` | የጥገና ሥራ ሽግግሮች OTEL ቆጣሪ. |
| `sorafs_repair_latency_minutes` | ሂስቶግራም | `outcome` | OTEL ሂስቶግራም ለጥገና የህይወት ዑደት መዘግየት። |
| `sorafs_repair_queue_depth` | ሂስቶግራም | `provider` | OTEL ሂስቶግራም የተሰለፉ ተግባራት በአንድ አቅራቢ (የቅጽበተ-ቅጥ)። |
| `sorafs_repair_backlog_oldest_age_seconds` | ሂስቶግራም | - | የ OTEL ሂስቶግራም በጣም የቆየ የተሰለፈው የተግባር ዕድሜ (ሰከንዶች)። |
| `sorafs_repair_lease_expired_total` | ቆጣሪ | `outcome` | የ OTEL ቆጣሪ ለሊዝ ጊዜው ያበቃል (`requeued`/`escalated`)። |
| `sorafs_repair_slash_proposals_total` | ቆጣሪ | `outcome` | የ OTEL ቆጣሪ ለስላሽ ፕሮፖዛል ሽግግሮች። |
| `torii_sorafs_repair_tasks_total` | ቆጣሪ | `status` | Prometheus ቆጣሪ ለተግባር ሽግግሮች። |
| `torii_sorafs_repair_latency_minutes_bucket` | ሂስቶግራም | `outcome` | Prometheus ሂስቶግራም ለጥገና የህይወት ዑደት መዘግየት። |
| `torii_sorafs_repair_queue_depth` | መለኪያ | `provider` | Prometheus መለኪያ በአንድ አቅራቢ ለተሰለፉ ተግባራት። |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | መለኪያ | - | Prometheus መለኪያ ለቀደመው ተራ የተግባር ዕድሜ (ሰከንድ)። |
| `torii_sorafs_repair_lease_expired_total` | ቆጣሪ | `outcome` | Prometheus ቆጣሪ ለሊዝ ጊዜው ያበቃል። |
| `torii_sorafs_slash_proposals_total` | ቆጣሪ | `outcome` | Prometheus ቆጣሪ ለስላሽ ፕሮፖዛል ሽግግሮች። |

የአስተዳደር ኦዲት JSON ሜታዳታ የጥገና ቴሌሜትሪ መለያዎችን ያንጸባርቃል (`status`፣ `ticket_id`፣ `manifest`፣ `provider` በጥገና ዝግጅቶች ላይ፣ `outcome` በሥዕል ጥበብ ፕሮፖጋንዳዎች እና በሥዕል ፕሮፖጋንዳዎች ላይ) በቆራጥነት።

### ማቆየት እና ጂሲ| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `sorafs_gc_runs_total` | ቆጣሪ | `result` | OTEL ቆጣሪ ለጂሲ ጠራርጎዎች፣ በተከተተው መስቀለኛ መንገድ የተለቀቀ። |
| `sorafs_gc_evictions_total` | ቆጣሪ | `reason` | OTEL ቆጣሪ ለተባረሩ መገለጫዎች በምክንያት ተቧድኗል። |
| `sorafs_gc_bytes_freed_total` | ቆጣሪ | `reason` | የ OTEL ቆጣሪ ለ ባይት በምክንያት ተቧድኗል። |
| `sorafs_gc_blocked_total` | ቆጣሪ | `reason` | OTEL ለማፈናቀል ቆጣሪ በነቃ ጥገና ወይም ፖሊሲ ታግዷል። |
| `torii_sorafs_gc_runs_total` | ቆጣሪ | `result` | Prometheus ቆጣሪ ለጂሲ ጠረገ (ስኬት/ስህተት)። |
| `torii_sorafs_gc_evictions_total` | ቆጣሪ | `reason` | Prometheus ቆጣሪ በምክንያት ተቧድኖ የተባረሩ መገለጫዎች። |
| `torii_sorafs_gc_bytes_freed_total` | ቆጣሪ | `reason` | Prometheus ቆጣሪ ለ ባይት በምክንያት ተቧድኗል። |
| `torii_sorafs_gc_blocked_total` | ቆጣሪ | `reason` | Prometheus ቆጣሪ በምክንያት ተቧድኖ ለታገዱ መፈናቀሎች። |
| `torii_sorafs_gc_expired_manifests` | መለኪያ | - | የአሁን ጊዜ ያለፈባቸው አንጸባራቂዎች ብዛት በጂሲ ጠረገ። |
| `torii_sorafs_gc_oldest_expired_age_seconds` | መለኪያ | - | እድሜ ካለፈው አንጋፋው አንጸባራቂ በሰከንዶች ውስጥ (ከማቆየት ጸጋ በኋላ)። |

### እርቅ

| መለኪያ | አይነት | መለያዎች | ማስታወሻ |
|--------|-------|----|------|
| `sorafs.reconciliation.runs_total` | ቆጣሪ | `result` | OTEL ቆጣሪ ለዕርቅ ቅጽበታዊ እይታዎች። |
| `sorafs.reconciliation.divergence_total` | ቆጣሪ | - | OTEL የልዩነት ቆጠራ በአንድ ሩጫ። |
| `torii_sorafs_reconciliation_runs_total` | ቆጣሪ | `result` | Prometheus ቆጣሪ ለማስታረቅ ሩጫዎች። |
| `torii_sorafs_reconciliation_divergence_count` | መለኪያ | - | በማስታረቅ ሪፖርት ላይ የቅርብ ጊዜ ልዩነት ቆጠራ ታይቷል። |

### በጊዜው የማግኘት ማረጋገጫ (PoTR) እና ቁራጭ SLA

| መለኪያ | አይነት | መለያዎች | አዘጋጅ | ማስታወሻ |
|--------|-------|-------|
| `sorafs_potr_deadline_ms` | ሂስቶግራም | `tier`, `provider` | PoTR አስተባባሪ | የማብቂያ ጊዜ መዘግየት በሚሊሰከንዶች (አዎንታዊ = ተሟልቷል)። |
| `sorafs_potr_failures_total` | ቆጣሪ | `tier`፣ `provider`፣ `reason` | PoTR አስተባባሪ | ምክንያቶች: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | ቆጣሪ | `provider`፣ `manifest_id`፣ `reason` | SLA ማሳያ | ቁራጭ መላኪያ SLO ሲያመልጥ ተባረረ (የማዘግየት፣ የስኬት መጠን)። |
| `sorafs_chunk_sla_violation_active` | መለኪያ | `provider`, `manifest_id` | SLA ማሳያ | የቦሊያን መለኪያ (0/1) ገባሪ በሆነ የጥሰት መስኮት ወቅት ተቀይሯል። |

## SLO ኢላማዎች

- የጌትዌይ ታማኝነት የሌለው ተገኝነት፡ **99.9%** (ኤችቲቲፒ 2xx/304 ምላሾች)።
- እምነት የለሽ TTFB P95፡ ሙቅ ደረጃ ≤120 ሚሴ፣ ሞቅ ያለ ደረጃ ≤300 ሚሴ
- የስኬት መጠን፡ ≥99.5% በቀን።
- የኦርኬስትራ ስኬት (ጭፍን ማጠናቀቅ): ≥99%.

## ዳሽቦርዶች እና ማንቂያ1. **የጌትዌይ ታዛቢነት** (`dashboards/grafana/sorafs_gateway_observability.json`) - እምነት የለሽ ተገኝነትን፣ TTFB P95ን፣ እምቢተኝነትን እና የPoR/PoTR ውድቀቶችን በOTEL ሜትሪክስ ይከታተላል።
2. ** ኦርኬስትራ ጤና *** (`dashboards/grafana/sorafs_fetch_observability.json`) - የባለብዙ ምንጭ ጭነትን፣ ሙከራዎችን፣ የአቅራቢዎችን ውድቀቶችን እና የድንኳን ፍንዳታዎችን ይሸፍናል።
3. **የሶራኔት የግላዊነት መለኪያዎች** (`dashboards/grafana/soranet_privacy_metrics.json`) — ስማቸው ያልተገለጡ የቅብብሎሽ ባልዲዎች፣ የማፈን መስኮቶች እና ሰብሳቢ ጤና በ`soranet_privacy_last_poll_unixtime`፣ `soranet_privacy_collector_enabled`፣ እና `soranet_privacy_poll_errors_total{provider}` ገበታዎች።
4. ** የአቅም ጤና *** (`dashboards/grafana/sorafs_capacity_health.json`) - ትራክ አቅራቢ headroom እና SLA ጭማሪዎች መጠገን, የጥገና ወረፋ ጥልቀት በአቅራቢው, እና ጂሲ ጠራርጎ / ማስወጣት / ባይት ነጻ / የታገዱ ምክንያቶች / ጊዜው ያለፈበት-የእድሜ እና የእርቅ ልዩነት ቅጽበተ-ፎቶዎች.

ማንቂያ ጥቅሎች፡

- `dashboards/alerts/sorafs_gateway_rules.yml` — መግቢያ በር መገኘት፣ TTFB፣ የማረጋገጫ አለመሳካቶች
- `dashboards/alerts/sorafs_fetch_rules.yml` - የኦርኬስትራ ውድቀቶች / ሙከራዎች / መሸጫዎች; በ `scripts/telemetry/test_sorafs_fetch_alerts.sh`፣ `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`፣ `dashboards/alerts/tests/soranet_privacy_rules.test.yml`፣ እና `dashboards/alerts/tests/soranet_policy_rules.test.yml` የተረጋገጠ።
- `dashboards/alerts/sorafs_capacity_rules.yml` - የአቅም ግፊት እና ጥገና SLA / backlog / የሊዝ-የሚያበቃበት ማንቂያዎች እና ጂሲ ስቶል / የታገዱ / ማቆየት ጠራርጎ የሚሆን ስህተት ማንቂያዎች.
- `dashboards/alerts/soranet_privacy_rules.yml` - የግላዊነት ማሽቆልቆል ካስማዎች፣ የጭቆና ማንቂያዎች፣ ሰብሳቢ-ስራ ፈትነትን ማወቅ እና የአካል ጉዳተኛ ሰብሳቢ ማንቂያዎች (`soranet_privacy_last_poll_unixtime`፣ `soranet_privacy_collector_enabled`)።
- `dashboards/alerts/soranet_policy_rules.yml` - ማንነታቸው የማይታወቅ ቡኒ ማንቂያዎች ወደ `sorafs_orchestrator_brownouts_total` ተሽረዋል።
- `dashboards/alerts/taikai_viewer_rules.yml` — የታይካይ ተመልካች ተንሸራታች/መዋጥ/CEK መዘግየት ማንቂያዎች እና አዲሱ የSoraFS ማረጋገጫ-ጤና ቅጣት/ቀዝቃዛ ማንቂያዎች በ`torii_sorafs_proof_health_*`።

## የመከታተያ ስትራቴጂ

- ክፍት ቴሌሜትሪ ከጫፍ እስከ ጫፍ ተቀበል፡
  - የመግቢያ መንገዶች የOTLP spans (HTTP) በጥያቄ መታወቂያዎች፣ የሰነድ መግለጫዎች እና የማስመሰያ ሃሾች የተብራራ ያመነጫሉ።
  - ኦርኬስትራተሩ ወደ ውጭ ለመላክ ሙከራዎች `tracing` + `opentelemetry` ይጠቀማል።
  - የተከተተ SoraFS አንጓዎች ለPoR ተግዳሮቶች እና የማከማቻ ስራዎች ወደ ውጭ የመላክ ጊዜ። ሁሉም ክፍሎች በ`x-sorafs-trace` በኩል የተሰራጨ የጋራ የመከታተያ መታወቂያ ይጋራሉ።
- `SorafsFetchOtel` የኦርኬስትራ መለኪያዎችን ወደ OTLP ሂስቶግራም ሲያገናኝ የ`telemetry::sorafs.fetch.*` ዝግጅቶች ቀላል ክብደት ያላቸውን JSON ለሎግ-አማካይ የኋላ መከለያዎች ይሰጣሉ።
- ሰብሳቢዎች፡ የኦቲኤል ሰብሳቢዎችን ከPrometheus/Loki/Tempo (የቴምፖ ተመራጭ) ጋር ያካሂዱ። Jaeger API ላኪዎች እንደ አማራጭ ይቀራሉ።
- ከፍተኛ-ካርዲኒቲ ኦፕሬሽኖች ናሙና መሆን አለባቸው (10% ለስኬት ጎዳናዎች ፣ 100% ውድቀቶች)።

## የቲኤልኤስ ቴሌሜትሪ ማስተባበሪያ (SF-5b)

- ሜትሪክ አሰላለፍ;
  - TLS አውቶሜሽን `sorafs_gateway_tls_cert_expiry_seconds`፣ `sorafs_gateway_tls_renewal_total{result}` እና `sorafs_gateway_tls_ech_enabled` ይልካል።
  - እነዚህን መለኪያዎች በTLS/የሰርቲፊኬቶች ፓነል ስር በጌትዌይ አጠቃላይ እይታ ዳሽቦርድ ውስጥ ያካትቱ።
- የማንቂያ ትስስር;
  - TLS ጊዜው የሚያበቃበት ማንቂያዎች እሳት (≤14 ቀናት ይቀራሉ) ከታመነው ተገኝነት SLO ጋር ይዛመዳሉ።
  - የ ECH አካል ጉዳተኛ TLS እና የተገኝነት ፓነሎችን የሚያመለክት ሁለተኛ ደረጃ ማንቂያ ያወጣል።
- የቧንቧ መስመር: የ TLS አውቶሜሽን ሥራ ወደ ተመሳሳይ Prometheus ቁልል እንደ ጌትዌይ ሜትሪክስ ወደ ውጭ ይልካል; ከ SF-5b ጋር ማስተባበር የተባዛ መሳሪያዎችን ያረጋግጣል.

## የሜትሪክ ስያሜ እና መለያ ስምምነቶች- የመለኪያ ስሞች ነባሩን `torii_sorafs_*` ወይም `sorafs_*` ቅድመ ቅጥያዎችን በTorii እና በመግቢያው ይከተላሉ።
- የመለያ ስብስቦች ደረጃቸውን የጠበቁ ናቸው፡
  - `result` → HTTP ውጤት (`success`፣ `refused`፣ `failed`)።
  - `reason` → እምቢታ/የስህተት ኮድ (`unsupported_chunker`፣ `timeout`፣ ወዘተ)።
  - `status` → የጥገና ተግባር ሁኔታ (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → የጥገና ኪራይ ውል ወይም የቆይታ ጊዜ ውጤት (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → ሄክስ-ኢንኮድ አቅራቢ መለያ።
  - `manifest` → ቀኖናዊ አንጸባራቂ መፍጨት (ከፍተኛ ካርዲናሊቲ ሲፈጠር የተከረከመ)።
  - `tier` → ገላጭ ደረጃ መለያዎች (`hot`፣ `warm`፣ `archive`)።
- የቴሌሜትሪ ልቀት ነጥቦች;
  - የጌትዌይ ሜትሪክስ በ`torii_sorafs_*` ስር ይኖራሉ እና ከ`crates/iroha_core/src/telemetry.rs` የውል ስምምነቶችን እንደገና ይጠቀሙ።
  - ኦርኬስትራተሩ የ`sorafs_orchestrator_*` ሜትሪክስ እና `telemetry::sorafs.fetch.*` ክስተቶችን (የህይወት ዑደት፣ ድጋሚ ሞክር፣ የአቅራቢ ውድቀት፣ ስህተት፣ ድንኳን) በማንፀባረቅ መፈጨት፣ የስራ መታወቂያ፣ ክልል እና አቅራቢ መለያዎች ይለቃል።
  - አንጓዎች ወለል `torii_sorafs_storage_*`፣ `torii_sorafs_capacity_*`፣ እና `torii_sorafs_por_*`።
- የመለኪያ ካታሎግ በተጋራ Prometheus የስያሜ ሰነድ ውስጥ ለመመዝገብ ከተመልካችነት ጋር ማስተባበር፣ የመለያ ካርዲናሊቲ የሚጠበቁትን ጨምሮ (አቅራቢ/የላይኛውን ድንበር ያሳያል)።

## የውሂብ ቧንቧ

- ሰብሳቢዎች OTLP ወደ Prometheus (ሜትሪክስ) እና Loki/Tempo (ምዝግብ ማስታወሻዎች) በመላክ ከእያንዳንዱ አካል ጋር ያሰማራሉ።
- አማራጭ eBPF (ቴትራጎን) ለበረንዳዎች/አንጓዎች ዝቅተኛ ደረጃ ፍለጋን ያበለጽጋል።
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` ለ Torii እና የተከተቱ ኖዶች ይጠቀሙ; ኦርኬስትራ ወደ `install_sorafs_fetch_otlp_exporter` መደወል ቀጥሏል።

## የማረጋገጫ መንጠቆዎች

- የ Prometheus የማንቂያ ደንቦች ከስቶል ሜትሪክስ እና የግላዊነት ማፈኛ ፍተሻዎች ጋር መቆለፋቸውን ለማረጋገጥ `scripts/telemetry/test_sorafs_fetch_alerts.sh`ን በCI ጊዜ ያሂዱ።
- Grafana ዳሽቦርዶችን በስሪት ቁጥጥር (`dashboards/grafana/`) ያቆዩ እና ፓነሎች ሲቀየሩ ቅጽበታዊ ገጽ እይታዎችን/ማገናኛዎችን ያዘምኑ።
- Chaos ምዝግብ ማስታወሻ ውጤቶችን በ `scripts/telemetry/log_sorafs_drill.sh`; የማረጋገጫ አቅም `scripts/telemetry/validate_drill_log.sh` ([ኦፕሬሽኖች ፕሌይቡክ](operations-playbook.md ይመልከቱ))።