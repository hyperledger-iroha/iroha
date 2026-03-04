---
lang: ka
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

ეს გვერდი ასახავს შიდა ანგარიშსწორების ხშირად დასმულ კითხვებს (`docs/source/nexus_settlement_faq.md`)
ასე რომ, პორტალის მკითხველს შეუძლია განიხილოს იგივე სახელმძღვანელო მითითებების გათხრების გარეშე
მონო-რეპო. იგი განმარტავს, თუ როგორ ამუშავებს ანგარიშსწორების როუტერი გადახდებს, რა მეტრიკას
მონიტორინგისთვის და როგორ უნდა გააერთიანონ SDK-ებმა Norito დატვირთვა.

## მაჩვენებლები

1. **ზოლის რუქა** — თითოეული მონაცემთა სივრცე აცხადებს `settlement_handle`
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, ან
   `xor_dual_fund`). იხილეთ უახლესი ზოლის კატალოგი ქვემოთ
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **დეტერმინისტული კონვერტაცია** — როუტერი აკონვერტებს ყველა დასახლებას XOR-ში მეშვეობით
   მმართველობის მიერ დამტკიცებული ლიკვიდობის წყაროები. კერძო ხაზების წინასწარი დაფინანსება XOR ბუფერები;
   თმის შეჭრა გამოიყენება მხოლოდ მაშინ, როდესაც ბუფერები მოძრაობენ პოლიტიკის გარეთ.
3. **ტელემეტრია** — საათი `nexus_settlement_latency_seconds`, კონვერტაციის მრიცხველები,
   და თმის შეჭრის ლიანდაგები. დაფები ცხოვრობს `dashboards/grafana/nexus_settlement.json`-ში
   და გაფრთხილებები `dashboards/alerts/nexus_audit_rules.yml`-ში.
4. **მტკიცებულება** — არქივის კონფიგურაციები, როუტერის ჟურნალები, ტელემეტრიის ექსპორტი და
   შეჯერების ანგარიშები აუდიტებისთვის.
5. **SDK-ის პასუხისმგებლობა** — თითოეულმა SDK-მა უნდა გამოავლინოს დასახლების დამხმარეები, ზოლის ID,
   და Norito დატვირთვის ენკოდერები როუტერთან პარიტეტის შესანარჩუნებლად.

## ნაკადების მაგალითი

| ზოლის ტიპი | მტკიცებულება ხელში | რას ამტკიცებს |
|-----------|------------------|----------------|
| პირადი `xor_hosted_custody` | როუტერის ჟურნალი + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC ბუფერები დებეტში დეტერმინისტული XOR და თმის შეჭრა რჩება პოლიტიკაში. |
| საჯარო `xor_global` | როუტერის ჟურნალი + DEX/TWAP მითითება + შეყოვნება/კონვერტაციის მეტრიკა | ლიკვიდობის საერთო გზამ გადარიცხვის ფასი შეაფასა გამოქვეყნებულ TWAP-ზე ნულოვანი თმის შეჭრით. |
| ჰიბრიდი `xor_dual_fund` | როუტერის ჟურნალი, რომელიც აჩვენებს საჯარო და დაცულ გაყოფას + ტელემეტრიის მრიცხველებს | დაცული/საზოგადოებრივი ნაზავი პატივს სცემდა მმართველობის კოეფიციენტებს და ჩაიწერა თმის შეჭრა თითოეულ ფეხზე. |

## გჭირდებათ მეტი დეტალი?

- სრული FAQ: `docs/source/nexus_settlement_faq.md`
- დასახლების როუტერის სპეციფიკაცია: `docs/source/settlement_router.md`
- CBDC პოლიტიკის სათამაშო წიგნი: `docs/source/cbdc_lane_playbook.md`
- ოპერაციების წიგნი: [Nexus ოპერაციები] (./nexus-operations)