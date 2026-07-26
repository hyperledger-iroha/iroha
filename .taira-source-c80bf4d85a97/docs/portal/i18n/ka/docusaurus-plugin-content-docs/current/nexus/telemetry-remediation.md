---
id: nexus-telemetry-remediation
lang: ka
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# მიმოხილვა

საგზაო რუკის პუნქტი **B2 — ტელემეტრიული ხარვეზის ფლობა** მოითხოვს გამოქვეყნებულ გეგმის მიბმას
ყველა გამორჩეული Nexus ტელემეტრიული უფსკრული სიგნალთან, გაფრთხილების დამცავი ღერძი, მფლობელი,
ვადა და დადასტურების არტეფაქტი 2026 წლის კვარტალში აუდიტის ფანჯრების დაწყებამდე.
ეს გვერდი ასახავს `docs/source/nexus_telemetry_remediation_plan.md`-ს, ამიტომ გაათავისუფლეთ
ინჟინერიის, ტელემეტრიის ოპერაციების და SDK მფლობელებს შეუძლიათ დაადასტურონ დაფარვა მანამდე
routed-trace და `TRACE-TELEMETRY-BRIDGE` რეპეტიციები.

# უფსკრული მატრიცა

| ხარვეზის ID | სიგნალის და გაფრთხილების დამცავი მოაჯირი | მფლობელი / ესკალაცია | ვადა (UTC) | მტკიცებულება და გადამოწმება |
|--------|-------------------------------------------|----------|---------------------------------|
| `GAP-TELEM-001` | ჰისტოგრამა `torii_lane_admission_latency_seconds{lane_id,endpoint}` გაფრთხილებით **`SoranetLaneAdmissionLatencyDegraded`** სროლისას `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 წუთის განმავლობაში (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (სიგნალი) + `@telemetry-ops` (გაფრთხილება); ესკალაცია Nexus მარშრუტირებული კვალი გამოძახებით. | 2026-02-23 | გაფრთხილების ტესტები `dashboards/alerts/tests/soranet_lane_rules.test.yml`-ში, პლუს `TRACE-LANE-ROUTING` რეპეტიციური გადაღება, რომელიც აჩვენებს გასროლილ/აღდგენილ სიგნალიზაციას და Torii `/metrics` სკრეპი, რომელიც დაარქივებულია [I18NT00000005-ში შენიშვნები](./nexus-transition-notes). |
| `GAP-TELEM-002` | მრიცხველი `nexus_config_diff_total{knob,profile}` დამცავი მოაჯირით `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` აწყობს კარიბჭეს (`docs/source/telemetry.md`). | `@nexus-core` (ინსტრუმენტაცია) → `@telemetry-ops` (გაფრთხილება); მმართველი მოვალეობის ოფიცერი გვერდს უსვამს, როდესაც მრიცხველი მოულოდნელად იზრდება. | 2026-02-26 | მართვის მშრალი გაშვების შედეგები ინახება `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`-ის გვერდით; გამოშვების საკონტროლო სია მოიცავს Prometheus მოთხოვნის სკრინშოტს და ჟურნალის ამონაწერს, რომელიც ადასტურებს `StateTelemetry::record_nexus_config_diff`-ის გამოსხივებას. |
| `GAP-TELEM-003` | მოვლენა `TelemetryEvent::AuditOutcome` (მეტრული `nexus.audit.outcome`) გაფრთხილებით **`NexusAuditOutcomeFailure`** როდესაც წარუმატებლობა ან გამოტოვებული შედეგები გრძელდება >30 წუთის განმავლობაში (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (მილსადენი) იზრდება `@sec-observability`-მდე. | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` არქივს NDJSON იტვირთება და ვერ ხერხდება, როდესაც TRACE ფანჯარას არ აქვს წარმატებული მოვლენა; გაფრთხილების ეკრანის ანაბეჭდები, რომლებიც მიმაგრებულია მარშრუტირებული კვალის ანგარიშზე. |
| `GAP-TELEM-004` | ლიანდაგი `nexus_lane_configured_total` დამცავი მოაჯირით `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, რომელიც კვებავს SRE გამოძახების საკონტროლო სიას. | `@telemetry-ops` (გაზომვა/ექსპორტი) იზრდება `@nexus-core`-მდე, როდესაც კვანძები აცნობებენ კატალოგის არათანმიმდევრულ ზომებს. | 2026-02-28 | გრაფიკის ტელემეტრიული ტესტი `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ადასტურებს ემისიას; ოპერატორები ამაგრებენ Prometheus diff + `StateTelemetry::set_nexus_catalogs` ჟურნალის ამონაწერს TRACE სარეპეტიციო პაკეტს. |

# ოპერატიული სამუშაო პროცესი

1. **ყოველკვირეული ტრიაჟი.** მფლობელები აცნობებენ პროგრესს Nexus მზადყოფნის ზარში;
   ბლოკატორები და გაფრთხილების ტესტის არტეფაქტები შესულია `status.md`-ში.
2. **Alert მშრალი გაშვებები.** ყოველი alert წესი გემების ერთად a
   `dashboards/alerts/tests/*.test.yml` ჩანაწერი, ასე რომ CI ახორციელებს `promtool ტესტს
   წესები, როდესაც საცავი იცვლება.
3. **აუდიტორული მტკიცებულება.** `TRACE-LANE-ROUTING`-ის დროს და
   `TRACE-TELEMETRY-BRIDGE` რეპეტიციებს ახორციელებს გამოძახებისას, აღწერს Prometheus მოთხოვნას
   შედეგები, გაფრთხილების ისტორია და შესაბამისი სკრიპტის შედეგები
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` კორელირებული სიგნალებისთვის) და
   ინახავს მათ მარშრუტულ-კვალი არტეფაქტებთან ერთად.
4. **ესკალაცია.** თუ სარეპეტიციო ფანჯრის მიღმა გაჩნდება დამცავი ღობე, მესაკუთრე
   გუნდი წარადგენს Nexus ინციდენტის ბილეთს ამ გეგმაზე მითითებით, მათ შორის
   მეტრიკული სნეპშოტი და შემარბილებელი ნაბიჯები აუდიტის განახლებამდე.

გამოქვეყნებული ამ მატრიცით — და მითითებულია როგორც `roadmap.md`-დან, ასევე
`status.md` — საგზაო რუკის პუნქტი **B2** ახლა აკმაყოფილებს „პასუხისმგებლობას, ვადას,
გაფრთხილება, გადამოწმება“ მიღების კრიტერიუმები.