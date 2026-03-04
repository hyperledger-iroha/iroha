---
id: nexus-routed-trace-audit-2026q1
lang: ka
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/nexus_routed_trace_audit_report_2026q1.md`-ს. შეინახეთ ორივე ასლი გასწორებულად, სანამ დარჩენილი თარგმანები არ გამოდგება.
:::

# 2026 კვარტალის მარშრუტების კვალიფიკაციის აუდიტის ანგარიში (B1)

საგზაო რუქის პუნქტი **B1 — მარშრუტის კვალი აუდიტი და ტელემეტრიის საბაზისო** მოითხოვს
Nexus მარშრუტირებული კვალი პროგრამის კვარტალური მიმოხილვა. ეს ანგარიში ადასტურებს
Q12026 აუდიტის ფანჯარა (იანვარი-მარტი), რათა მმართველმა საბჭომ ხელი მოაწეროს მას
ტელემეტრიული პოზა Q2-ის გაშვების რეპეტიციებამდე.

## ფარგლები და ვადები

| კვალი ID | ფანჯარა (UTC) | მიზანი |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | გადაამოწმეთ ზოლის დაშვების ჰისტოგრამები, რიგის ჭორები და გაფრთხილების ნაკადი მრავალ ზოლის ჩართვამდე. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | დაადასტურეთ OTLP ხელახალი დაკვრა, განსხვავებული ბოტის პარიტეტი და SDK ტელემეტრიის ჩასმა AND4/AND7 ეტაპებზე წინ. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | დაადასტურეთ მმართველობის მიერ დამტკიცებული `iroha_config` დელტა და უკან დაბრუნების მზადყოფნა RC1 ჭრამდე. |

თითოეული რეპეტიცია წარმოების მსგავსი ტოპოლოგიით მიმდინარეობდა მარშრუტ-კვალით
ჩართულია ინსტრუმენტაცია (`nexus.audit.outcome` ტელემეტრია + Prometheus მრიცხველი),
Alertmanager-ის წესები დატვირთულია და მტკიცებულება ექსპორტირებულია `docs/examples/`-ში.

## მეთოდოლოგია

1. **ტელემეტრიის კოლექცია.** ყველა კვანძი ასხივებდა სტრუქტურირებულს
   `nexus.audit.outcome` მოვლენა და თანმხლები მეტრიკა
   (`nexus_audit_outcome_total*`). დამხმარე
   `scripts/telemetry/check_nexus_audit_outcome.py` მოყვა JSON ჟურნალს,
   დაადასტურა ღონისძიების სტატუსი და დაარქივებული დატვირთვა ქვემოთ
   `docs/examples/nexus_audit_outcomes/`.【scripts/telemetry/check_nexus_audit_outcome.py:1】
2. **გაფრთხილების ვალიდაცია.** `dashboards/alerts/nexus_audit_rules.yml` და მისი ტესტი
   აღკაზმულობა უზრუნველყოფდა გაფრთხილების ხმაურის ზღურბლებს და დატვირთვის შაბლონის შენარჩუნებას
   თანმიმდევრული. CI მუშაობს `dashboards/alerts/tests/nexus_audit_rules.test.yml`-ზე
   ყოველი ცვლილება; იგივე წესები ხორციელდებოდა ხელით თითოეული ფანჯრის დროს.
3. **Dashboard-ის აღება.** ოპერატორებმა მოახდინეს მარშრუტირებული ტრასის პანელების ექსპორტი
   `dashboards/grafana/soranet_sn16_handshake.json` (ხელის ჩამორთმევის ჯანმრთელობა) და
   ტელემეტრიის მიმოხილვის დაფები რიგის სიჯანსაღის აუდიტის შედეგებთან კორელაციისთვის.
4. **მიმომხილველი აღნიშნავს.** მმართველობის მდივანმა ჩაწერა რეფერენტის ინიციალები,
   გადაწყვეტილება და ნებისმიერი შემამსუბუქებელი ბილეთი [Nexus გარდამავალი შენიშვნები] (./nexus-transition-notes)
   და კონფიგურაციის დელტა ტრეკერი (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## დასკვნები

| კვალი ID | შედეგი | მტკიცებულება | შენიშვნები |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | საშვი | გაფრთხილების ცეცხლი/აღდგენის ეკრანის ანაბეჭდები (შიდა ბმული) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` განმეორება; ტელემეტრიული განსხვავებები ჩაწერილია [Nexus გარდამავალ ნოტებში] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | რიგში დაშვება P95 დარჩა 612ms (სამიზნე ≤750ms). შემდგომი დაკვირვება არ არის საჭირო. |
| `TRACE-TELEMETRY-BRIDGE` | საშვი | დაარქივებული შედეგის დატვირთვა `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` პლუს OTLP განმეორებითი ჰეში ჩაწერილი `status.md`-ში. | SDK რედაქციის მარილები ემთხვეოდა Rust-ის საწყისს; diff bot იტყობინება ნულოვანი დელტა. |
| `TRACE-CONFIG-DELTA` | უღელტეხილი (შემარბილებელი დახურული) | მმართველობის ტრეკერის ჩანაწერი (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS პროფილის მანიფესტი (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + ტელემეტრიული პაკეტის მანიფესტი (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2-ის განმეორებით გაშვებამ გააშინა დამტკიცებული TLS პროფილი და დაადასტურა ნულოვანი სტრაგლერები; ტელემეტრიის მანიფესტის ჩანაწერების სლოტის დიაპაზონი 912–936 და სამუშაო დატვირთვის თესლი `NEXUS-REH-2026Q2`. |

ყველა კვალმა წარმოქმნა მინიმუმ ერთი `nexus.audit.outcome` მოვლენა მათში
ფანჯრები, დამაკმაყოფილებელი Alertmanager-ის ფარები (`NexusAuditOutcomeFailure`
დარჩა მწვანე მეოთხედი).

## შემდგომი

- Routed-trace დანართი განახლებულია TLS ჰეშით `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`;
  შემარბილებელი საშუალება `NEXUS-421` დაიხურა გარდამავალ შენიშვნებში.
- განაგრძეთ დაუმუშავებელი OTLP-ის გამეორებების და Torii განსხვავებული არტეფაქტების მიმაგრება არქივში
  გააძლიერეთ თანასწორობის მტკიცებულება Android AND4/AND7 მიმოხილვებისთვის.
- დაადასტურეთ, რომ მომავალი `TRACE-MULTILANE-CANARY` რეპეტიციები იგივეს ხელახლა გამოიყენებს
  ტელემეტრიის დამხმარე, ასე რომ Q2-ის გაფორმება ისარგებლებს დადასტურებული სამუშაო ნაკადით.

## არტეფაქტის ინდექსი

| აქტივი | მდებარეობა |
|-------|----------|
| ტელემეტრიის ვალიდატორი | `scripts/telemetry/check_nexus_audit_outcome.py` |
| გაფრთხილების წესები და ტესტები | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| ნიმუშის შედეგის დატვირთვა | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| დელტა ტრეკერის კონფიგურაცია | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| მარშრუტ-კვალი გრაფიკი & შენიშვნები | [Nexus გარდამავალი შენიშვნები](./nexus-transition-notes) |

ეს ანგარიში, ზემოთ მოყვანილი არტეფაქტები და გაფრთხილების/ტელემეტრიის ექსპორტი უნდა იყოს
თან ერთვის მმართველობის გადაწყვეტილებების ჟურნალს B1 კვარტლის დახურვის მიზნით.