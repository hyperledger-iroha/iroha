---
id: observability-plan
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

## მიზნები
- განსაზღვრეთ მეტრიკა და სტრუქტურირებული მოვლენები კარიბჭეებისთვის, კვანძებისთვის და მრავალ წყაროს ორკესტრისთვის.
- მიაწოდეთ Grafana დაფები, გაფრთხილების ზღურბლები და ვალიდაციის კაკვები.
- ჩამოაყალიბეთ SLO მიზნები შეცდომების საბიუჯეტო და ქაოსური სავარჯიშო პოლიტიკის გვერდით.

## მეტრული კატალოგი

### კარიბჭის ზედაპირები

| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `sorafs_gateway_active` | ლიანდაგი (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | ემიტირებული `SorafsGatewayOtel` მეშვეობით; თვალყურს ადევნებს ფრენის დროს HTTP ოპერაციებს საბოლოო წერტილის/მეთოდის კომბინაციის მიხედვით. |
| `sorafs_gateway_responses_total` | მრიცხველი | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI0000000040X, I18NI0000000040X | ყველა დასრულებული კარიბჭის მოთხოვნა იზრდება ერთხელ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | ჰისტოგრამა | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI0000000050X, I18NI0000000050X | დროიდან პირველ ბაიტამდე შეყოვნება კარიბჭის პასუხებისთვის; ექსპორტირებული როგორც Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | მრიცხველი | `profile_version`, `result`, `error_code` | დადასტურების დადასტურების შედეგები აღბეჭდილია მოთხოვნის დროს (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | ჰისტოგრამა | `profile_version`, `result`, `error_code` | ვერიფიკაციის შეყოვნების განაწილება PoR ქვითრებისთვის. |
| `telemetry::sorafs.gateway.request` | სტრუქტურირებული ღონისძიება | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | სტრუქტურირებული ჟურნალი გამოქვეყნებულია Loki/Tempo კორელაციის ყოველი მოთხოვნის დასრულებისას. |

`telemetry::sorafs.gateway.request` მოვლენები ასახავს OTEL-ის მრიცხველებს სტრუქტურირებული დატვირთვით, ზედაპირული `endpoint`, `method`, `variant`, I18NI000000080X, I1801800000, I18NI0000000079X, I18NI000000080X, I1801800000, I18NI00000000 ლოკი/ტემპის კორელაცია მაშინ, როცა დაფები მოიხმარენ OTLP სერიებს SLO თვალთვალისათვის.

### ჯანმრთელობის დამადასტურებელი ტელემეტრია

| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | მრიცხველი | `provider_id`, `trigger`, `penalty` | იზრდება ყოველ ჯერზე, როცა `RecordCapacityTelemetry` გამოსცემს `SorafsProofHealthAlert`-ს. `trigger` განასხვავებს PDP/PoTR/ორივე წარუმატებლობას, ხოლო `penalty` ასახავს, ​​იყო თუ არა გირაოს რეალურად შემცირება ან ჩახშობა გაგრილებით. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | ლიანდაგი | `provider_id` | უახლესი PDP/PoTR თვლები მოხსენებულია შეურაცხმყოფელი ტელემეტრიის ფანჯარაში, რათა გუნდებმა შეძლონ რაოდენობრივად განსაზღვრონ პროვაიდერების პოლიტიკა. |
| `torii_sorafs_proof_health_penalty_nano` | ლიანდაგი | `provider_id` | Nano-XOR რაოდენობა შემცირდა ბოლო გაფრთხილებისას (ნულოვანია, როდესაც გაგრილებამ თრგუნა აღსრულება). |
| `torii_sorafs_proof_health_cooldown` | ლიანდაგი | `provider_id` | ლოგიკური ლიანდაგი (`1` = გაფრთხილება ჩახშობილია გაგრილებით) ზედაპირზე, როდესაც შემდგომი გაფრთხილებები დროებით დადუმებულია. |
| `torii_sorafs_proof_health_window_end_epoch` | ლიანდაგი | `provider_id` | ეპოქა ჩაწერილია ტელემეტრიის ფანჯრისთვის, რომელიც დაკავშირებულია გაფრთხილებასთან, რათა ოპერატორებმა შეძლონ კორელაცია Norito არტეფაქტებთან. |

ეს არხები ახლა აძლიერებს Taikai-ის მაყურებლის დაფის ჯანმრთელობის მტკიცებულების რიგს
(`dashboards/grafana/taikai_viewer.json`), რაც CDN ოპერატორებს აძლევს პირდაპირ ხილვადობას
გაფრთხილების მოცულობაში, PDP/PoTR ტრიგერების მიქსში, ჯარიმებსა და გაგრილების მდგომარეობაზე
პროვაიდერი.

იგივე მეტრიკა ახლა მხარს უჭერს Taikai მაყურებლის გაფრთხილების ორ წესს:
`SorafsProofHealthPenalty` ისვრის ნებისმიერ დროს
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` იზრდება
ბოლო 15 წუთი, ხოლო `SorafsProofHealthCooldown` აჩენს გაფრთხილებას, თუ
პროვაიდერი რჩება გაგრილებაში ხუთი წუთის განმავლობაში. ორივე გაფრთხილება პირდაპირ ეთერშია
`dashboards/alerts/taikai_viewer_rules.yml` ასე რომ, SRE-ები მიიღებენ უშუალო კონტექსტს
როდესაც PoR/PoTR აღსრულება იზრდება.

### ორკესტრის ზედაპირები

| მეტრიკა / მოვლენა | ტიპი | ეტიკეტები | პროდიუსერი | შენიშვნები |
|----------------|------|-------|----------|------|
| `sorafs_orchestrator_active_fetches` | ლიანდაგი | `manifest_id`, `region` | `FetchMetricsCtx` | სესიები ამჟამად ფრენის დროს. |
| `sorafs_orchestrator_fetch_duration_ms` | ჰისტოგრამა | `manifest_id`, `region` | `FetchMetricsCtx` | ხანგრძლივობის ჰისტოგრამა მილიწამებში; 1ms→30s თაიგულები. |
| `sorafs_orchestrator_fetch_failures_total` | მრიცხველი | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | მიზეზები: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | მრიცხველი | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | განასხვავებს განმეორებით მიზეზებს (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | მრიცხველი | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | იჭერს სესიის დონის გამორთვის / წარუმატებლობის ანგარიშებს. |
| `sorafs_orchestrator_chunk_latency_ms` | ჰისტოგრამა | `manifest_id`, `provider_id` | `FetchMetricsCtx` | თითო ნაწილზე მოტანის შეყოვნების განაწილება (მმ) გამტარუნარიანობა/SLO ანალიზისთვის. |
| `sorafs_orchestrator_bytes_total` | მრიცხველი | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ბაიტები მიწოდებული თითო მანიფესტზე/პროვაიდერზე; მიიღება გამტარუნარიანობა `rate()`-ის მეშვეობით PromQL-ში. |
| `sorafs_orchestrator_stalls_total` | მრიცხველი | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ითვლის ნაწილებს `ScoreboardConfig::latency_cap_ms`-ზე მეტი. |
| `telemetry::sorafs.fetch.lifecycle` | სტრუქტურირებული ღონისძიება | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI000001860X, I18NI000001860X `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | ასახავს სამუშაოს სიცოცხლის ციკლს (დაწყება/დასრულება) Norito JSON დატვირთვით. |
| `telemetry::sorafs.fetch.retry` | სტრუქტურირებული ღონისძიება | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | ემიტირებული თითო პროვაიდერის ხელახალი ცდის სტრიქონზე; `attempts` ითვლის დამატებით განმეორებით ცდებს (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | სტრუქტურირებული ღონისძიება | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | ჩნდება მაშინ, როდესაც პროვაიდერი გადალახავს წარუმატებლობის ზღვარს. |
| `telemetry::sorafs.fetch.error` | სტრუქტურირებული ღონისძიება | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | ტერმინალის უკმარისობის ჩანაწერი, მეგობრული Loki/Splunk-ის გადაყლაპვისთვის. |
| `telemetry::sorafs.fetch.stall` | სტრუქტურირებული ღონისძიება | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | მატულობს, როდესაც ბლოკის შეყოვნება არღვევს კონფიგურირებულ თავსახურს (სარკეები დგას სალაროების მრიცხველებს). |

### კვანძის / რეპლიკაციის ზედაპირები

| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | ჰისტოგრამა | `provider_id` | საცავის გამოყენების პროცენტის OTEL ჰისტოგრამა (ექსპორტირებული როგორც `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | მრიცხველი | `provider_id` | მონოტონური მრიცხველი წარმატებული PoR ნიმუშებისთვის, მიღებული გრაფიკის კადრებიდან. |
| `sorafs_node_por_failure_total` | მრიცხველი | `provider_id` | მონოტონური მრიცხველი წარუმატებელი PoR ნიმუშებისთვის. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | ლიანდაგი | `provider` | არსებული Prometheus ლიანდაგები გამოყენებული ბაიტებისთვის, რიგის სიღრმე, PoR ფრენის რაოდენობა. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | ლიანდაგი | `provider` | პროვაიდერის სიმძლავრის/ტემპის წარმატების მონაცემები გამოჩნდა სიმძლავრის საინფორმაციო დაფაზე. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | ლიანდაგი | `provider`, `manifest` | ბექლოგის სიღრმე პლუს კუმულაციური წარუმატებლობის მრიცხველები ექსპორტირებულია `/v1/sorafs/por/ingestion/{manifest}`-ის გამოკითხვისას, რაც კვებავს „PoR Stalls“ პანელს/გაფრთხილებას. |

### შეკეთება და SLA

| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | მრიცხველი | `status` | OTEL მრიცხველი სარემონტო დავალების გადასვლებისთვის. |
| `sorafs_repair_latency_minutes` | ჰისტოგრამა | `outcome` | OTEL ჰისტოგრამა შეკეთების სასიცოცხლო ციკლის შეყოვნებისთვის. |
| `sorafs_repair_queue_depth` | ჰისტოგრამა | `provider` | OTEL რიგი ამოცანების ჰისტოგრამა თითო პროვაიდერზე (სნეპშოტის სტილი). |
| `sorafs_repair_backlog_oldest_age_seconds` | ჰისტოგრამა | — | OTEL-ის ჰისტოგრამა ყველაზე ძველი დავალების რიგებში (წამები). |
| `sorafs_repair_lease_expired_total` | მრიცხველი | `outcome` | OTEL-ის მთვლელი იჯარის ვადის გასვლისთვის (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | მრიცხველი | `outcome` | OTEL მთვლელი წინადადების გადასასვლელად. |
| `torii_sorafs_repair_tasks_total` | მრიცხველი | `status` | Prometheus მრიცხველი ამოცანების გადასვლებისთვის. |
| `torii_sorafs_repair_latency_minutes_bucket` | ჰისტოგრამა | `outcome` | Prometheus ჰისტოგრამა სასიცოცხლო ციკლის შეფერხებისთვის. |
| `torii_sorafs_repair_queue_depth` | ლიანდაგი | `provider` | Prometheus ლიანდაგი რიგზე მდგომი ამოცანების თითო პროვაიდერზე. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | ლიანდაგი | — | Prometheus ლიანდაგი ყველაზე ძველი რიგის დავალების ასაკისთვის (წამი). |
| `torii_sorafs_repair_lease_expired_total` | მრიცხველი | `outcome` | Prometheus მრიცხველი იჯარის ვადის გასვლისთვის. |
| `torii_sorafs_slash_proposals_total` | მრიცხველი | `outcome` | Prometheus მრიცხველი ხაზის წინადადების გადასვლებისთვის. |

მართვის აუდიტი JSON მეტამონაცემები ასახავს სარემონტო ტელემეტრიის ეტიკეტებს (`status`, `ticket_id`, `manifest`, `provider` სარემონტო მოვლენებზე; I18NI000000246X-ზე; დეტერმინისტულად.

### შეკავება და GC| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | მრიცხველი | `result` | OTEL-ის მრიცხველი GC სვიპებისთვის, რომელიც გამოსხივებულია ჩაშენებული კვანძის მიერ. |
| `sorafs_gc_evictions_total` | მრიცხველი | `reason` | OTEL-ის მრიცხველი გამოსახლებული მანიფესტებისთვის დაჯგუფებულია მიზეზის მიხედვით. |
| `sorafs_gc_bytes_freed_total` | მრიცხველი | `reason` | OTEL მრიცხველი გამოთავისუფლებული ბაიტებისთვის დაჯგუფებული მიზეზის მიხედვით. |
| `sorafs_gc_blocked_total` | მრიცხველი | `reason` | OTEL-ის მრიცხველი გამოსახლებისთვის დაბლოკილია აქტიური რემონტით ან პოლიტიკით. |
| `torii_sorafs_gc_runs_total` | მრიცხველი | `result` | Prometheus მრიცხველი GC სვიპებისთვის (წარმატება/შეცდომა). |
| `torii_sorafs_gc_evictions_total` | მრიცხველი | `reason` | Prometheus მთვლელი გამოსახლებული მანიფესტებისთვის დაჯგუფებული მიზეზით. |
| `torii_sorafs_gc_bytes_freed_total` | მრიცხველი | `reason` | Prometheus მრიცხველი გამოთავისუფლებული ბაიტებისთვის დაჯგუფებული მიზეზის მიხედვით. |
| `torii_sorafs_gc_blocked_total` | მრიცხველი | `reason` | Prometheus მრიცხველი დაბლოკილი გამოსახლებისთვის დაჯგუფებულია მიზეზის მიხედვით. |
| `torii_sorafs_gc_expired_manifests` | ლიანდაგი | — | ვადაგასული მანიფესტების ამჟამინდელი რაოდენობა დაფიქსირდა GC სვიპებით. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | ლიანდაგი | — | ასაკი ყველაზე ძველი ვადაგასული მანიფესტის წამებში (შეკავების შემდეგ). |

### შერიგება

| მეტრული | ტიპი | ეტიკეტები | შენიშვნები |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | მრიცხველი | `result` | OTEL მრიცხველი შერიგების კადრებისთვის. |
| `sorafs.reconciliation.divergence_total` | მრიცხველი | — | OTEL-ის დივერგენციის მრიცხველი თითო გაშვებაზე. |
| `torii_sorafs_reconciliation_runs_total` | მრიცხველი | `result` | Prometheus მრიცხველი შერიგებისთვის. |
| `torii_sorafs_reconciliation_divergence_count` | ლიანდაგი | — | უახლესი განსხვავება დაფიქსირდა შერიგების ანგარიშში. |

### დროული მოძიების დამადასტურებელი (PoTR) და ცალი SLA

| მეტრული | ტიპი | ეტიკეტები | პროდიუსერი | შენიშვნები |
|--------|------|-------|----------|-------|
| `sorafs_potr_deadline_ms` | ჰისტოგრამა | `tier`, `provider` | PoTR კოორდინატორი | ბოლო ვადის შემცირება მილიწამებში (დადებითი = დაკმაყოფილებულია). |
| `sorafs_potr_failures_total` | მრიცხველი | `tier`, `provider`, `reason` | PoTR კოორდინატორი | მიზეზები: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | მრიცხველი | `provider`, `manifest_id`, `reason` | SLA მონიტორი | გააქტიურებულია, როდესაც ნაწილის მიწოდებას აკლია SLO (დაყოვნება, წარმატების მაჩვენებელი). |
| `sorafs_chunk_sla_violation_active` | ლიანდაგი | `provider`, `manifest_id` | SLA მონიტორი | ლოგიკური ლიანდაგი (0/1) ჩართულია აქტიური დარღვევის ფანჯრის დროს. |

## SLO მიზნები

- კარიბჭის საიმედო ხელმისაწვდომობა: **99.9%** (HTTP 2xx/304 პასუხი).
- დაუჯერებელი TTFB P95: ცხელი იარუსი ≤120 ms, თბილი იარუსი ≤300 ms.
- დადასტურების წარმატების მაჩვენებელი: ≥99.5% დღეში.
- ორკესტრის წარმატება (ნაწილის დასრულება): ≥99%.

## დაფები და გაფრთხილება

1. **კარიბჭის დაკვირვება** (`dashboards/grafana/sorafs_gateway_observability.json`) — თვალყურს ადევნებს საიმედო ხელმისაწვდომობას, TTFB P95, უარის ავარიას და PoR/PoTR წარუმატებლობას OTEL მეტრიკის საშუალებით.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — მოიცავს მრავალ წყაროს დატვირთვას, განმეორებით მცდელობებს, პროვაიდერის წარუმატებლობას და აურზაურს.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — ანონიმური სარელეო თაიგულების, ჩახშობის ფანჯრებისა და კოლექტორის სიჯანსაღის დიაგრამები `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` და I18NI0000029 მეშვეობით.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) - თვალს ადევნებს პროვაიდერის სათავეს, პლუს სარემონტო SLA ესკალაციები, სარემონტო რიგის სიღრმე პროვაიდერის მიერ და GC სვიპები/გამოსახლებები/ბაიტები გათავისუფლებული/დაბლოკილი მიზეზები/ვადაგადაცილებული მანიფესტირებული ასაკობრივი და შერიგების დარღვევის განსხვავება.

გაფრთხილების პაკეტები:

- `dashboards/alerts/sorafs_gateway_rules.yml` — კარიბჭის ხელმისაწვდომობა, TTFB, წარუმატებლობის დამადასტურებელი მწვერვალები.
- `dashboards/alerts/sorafs_fetch_rules.yml` — ორკესტრის წარუმატებლობები/განმეორებითი ცდები/ჩერდება; დადასტურებულია `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` და `dashboards/alerts/tests/soranet_policy_rules.test.yml` მეშვეობით.
- `dashboards/alerts/sorafs_capacity_rules.yml` — სიმძლავრის წნევა პლუს შეკეთება SLA/ჩამორჩენილი/იჯარის ვადის გასვლის გაფრთხილებები და GC შეჩერების/დაბლოკვის/შეცდომის გაფრთხილებები შეკავების გაწმენდისთვის.
- `dashboards/alerts/soranet_privacy_rules.yml` — კონფიდენციალურობის შემცირების მწვერვალები, ჩახშობის სიგნალიზაცია, კოლექციონერის უმოქმედობის გამოვლენა და გამორთული კოლექციონერის გაფრთხილებები (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — ანონიმურობის სიგნალიზაცია ჩართულია `sorafs_orchestrator_brownouts_total`-ზე.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai მაყურებლის დრიფტის/შეღწევის/CEK შეფერხების სიგნალიზაცია პლუს ახალი SoraFS ჯანმრთელობის სასჯელის/გაგრილების დამადასტურებელი გაფრთხილებები, რომლებიც უზრუნველყოფილია `torii_sorafs_proof_health_*`-ით.

## მიკვლევის სტრატეგია

- მიიღეთ OpenTelemetry ბოლომდე:
  - კარიბჭეები ასხივებენ OTLP სპანებს (HTTP), რომლებიც ანოტირდება მოთხოვნის ID-ებით, მანიფესტების დაიჯესტებით და ტოკენის ჰეშებით.
  - ორკესტრატორი იყენებს `tracing` + `opentelemetry` სპექტაკლების ექსპორტისთვის, რათა მიიღონ მცდელობა.
  - ჩაშენებული SoraFS კვანძების ექსპორტი პორ გამოწვევებისთვის და შენახვის ოპერაციებისთვის. ყველა კომპონენტი იზიარებს `x-sorafs-trace`-ის მეშვეობით გავრცელებულ საერთო კვალი ID-ს.
- `SorafsFetchOtel` აკავშირებს ორკესტრატორის მეტრიკას OTLP ჰისტოგრამებში, ხოლო `telemetry::sorafs.fetch.*` ღონისძიებები უზრუნველყოფს მსუბუქ JSON დატვირთვას ლოგინზე ორიენტირებული ბექენდებისთვის.
- კოლექციონერები: მართეთ OTEL კოლექციონერები Prometheus/Loki/Tempo-სთან ერთად (სასურველია ტემპი). Jaeger API ექსპორტიორები არჩევითია.
- მაღალი კარდინალურობის ოპერაციების შერჩევა უნდა მოხდეს (10% წარმატების ბილიკებისთვის, 100% წარუმატებლობისთვის).

## TLS ტელემეტრიის კოორდინაცია (SF-5b)

- მეტრიკული გასწორება:
  - TLS ავტომატიზაცია იგზავნება `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` და `sorafs_gateway_tls_ech_enabled`.
  - ჩართეთ ეს მრიცხველები Gateway Overview-ის დაფაში TLS/სერთიფიკატების პანელის ქვეშ.
- გაფრთხილების კავშირი:
  - როდესაც TLS ვადის გასვლის სიგნალები გასროლის (≤14 დღე დარჩენილია) კორელაციაშია დაუსაბუთებელი ხელმისაწვდომობის SLO.
  - ECH გამორთვა ასხივებს მეორად გაფრთხილებას, რომელიც ეხება როგორც TLS, ასევე ხელმისაწვდომობის პანელებს.
- მილსადენი: TLS ავტომატიზაციის სამუშაო ექსპორტი ხდება იმავე Prometheus დასტაზე, როგორც კარიბჭის მეტრიკა; SF-5b-თან კოორდინაცია უზრუნველყოფს ინსტრუმენტაციის დუბლირებას.

## მეტრიკული დასახელებისა და ეტიკეტების კონვენციები

- მეტრული სახელები მიჰყვება არსებულ `torii_sorafs_*` ან `sorafs_*` პრეფიქსებს, რომლებიც გამოიყენება Torii და კარიბჭის მიერ.
- ეტიკეტების ნაკრები სტანდარტიზებულია:
  - `result` → HTTP შედეგი (`success`, `refused`, `failed`).
  - `reason` → უარის/შეცდომის კოდი (`unsupported_chunker`, `timeout` და ა.შ.).
  - `status` → სარემონტო დავალების მდგომარეობა (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → სარემონტო იჯარა ან ლატენტური შედეგი (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → ექვსკუთხედი კოდირებული პროვაიდერის იდენტიფიკატორი.
  - `manifest` → კანონიკური მანიფესტი დაიჯესტი (მოჭრილი მაღალი კარდინალურობისას).
  - `tier` → დეკლარაციული დონის ეტიკეტები (`hot`, `warm`, `archive`).
- ტელემეტრიის ემისიის წერტილები:
  - კარიბჭის მეტრიკა მოქმედებს `torii_sorafs_*`-ის ქვეშ და ხელახლა გამოიყენება `crates/iroha_core/src/telemetry.rs`-ის კონვენციები.
  - ორკესტრატორი გამოსცემს `sorafs_orchestrator_*` მეტრიკას და `telemetry::sorafs.fetch.*` მოვლენებს (სასიცოცხლო ციკლი, ხელახალი ცდა, პროვაიდერის წარუმატებლობა, შეცდომა, შეფერხება), მონიშნული მანიფესტის შეჯამებით, სამუშაოს ID, რეგიონისა და პროვაიდერის იდენტიფიკატორებით.
  - კვანძების ზედაპირი `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` და `torii_sorafs_por_*`.
- კოორდინაცია Observability-თან, რათა დაარეგისტრიროთ მეტრიკული კატალოგი Prometheus დასახელების საერთო დოკუმენტში, ლეიბლის კარდინალურობის მოლოდინების ჩათვლით (პროვაიდერი/გამოხატავს ზედა საზღვრებს).

## მონაცემთა მილსადენი

- კოლექციონერები განლაგებულია თითოეულ კომპონენტთან ერთად, ექსპორტზე OTLP-ზე Prometheus (მეტრიკა) და Loki/Tempo (ლოგიები/კვალი).
- სურვილისამებრ eBPF (ტეტრაგონი) ამდიდრებს დაბალი დონის მიკვლევას კარიბჭეებისთვის/კვანძებისთვის.
- გამოიყენეთ `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` Torii და ჩაშენებული კვანძებისთვის; ორკესტრი აგრძელებს `install_sorafs_fetch_otlp_exporter` დარეკვას.

## ვალიდაციის კაკვები

- გაუშვით `scripts/telemetry/test_sorafs_fetch_alerts.sh` CI-ის დროს, რათა დარწმუნდეთ, რომ Prometheus გაფრთხილების წესები დარჩება ჩაკეტილი მეტრიკისა და კონფიდენციალურობის აღკვეთის შემოწმებით.
- შეინახეთ Grafana დაფები ვერსიის კონტროლის ქვეშ (`dashboards/grafana/`) და განაახლეთ ეკრანის ანაბეჭდები/ბმულები, როდესაც პანელები იცვლება.
- ქაოსის წვრთნების შესვლა შედეგები `scripts/telemetry/log_sorafs_drill.sh`-ის საშუალებით; ვალიდაცია იყენებს `scripts/telemetry/validate_drill_log.sh` (იხილეთ [ოპერაციების წიგნი](operations-playbook.md)).