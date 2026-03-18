---
lang: ka
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] გადახედეთ ტექნიკის სპეციფიკას: 8+ ბირთვი, 16 გიბაიტი ოპერატიული მეხსიერება, NVMe საცავი ≥ 500 მიბ/წმ.
- [ ] დაადასტურეთ ორი IPv4 + IPv6 მისამართი და ზედა ნაკადის ნებართვები QUIC/UDP 443.
- [ ] უზრუნველყოფა HSM ან გამოყოფილი უსაფრთხო ანკლავი სარელეო იდენტიფიკაციის გასაღებებისთვის.
- [ ] სინქრონიზაცია კანონიკური უარის თქმის კატალოგი (`governance/compliance/soranet_opt_outs.json`).
- [ ] შესაბამისობის ბლოკის შერწყმა ორკესტრის კონფიგურაციაში (იხ. `03-config-example.toml`).
- [ ] მიიღეთ იურისდიქციის/გლობალური შესაბამისობის ატესტაციები და შეავსეთ `attestations` სია.
- [ ] გაუშვით `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (ან თქვენი პირდაპირი სნეპშოტი) და გადახედეთ უღელტეხილზე/ჩავარდნის ანგარიშს.
- [ ] შექმენით სარელეო დაშვების CSR და მიიღეთ მმართველობით ხელმოწერილი კონვერტი.
- [ ] შემოიტანეთ დამცავი აღწერის დათესვა და გადაამოწმეთ გამოქვეყნებული ჰეშის ჯაჭვის წინააღმდეგ.
- [ ] გაუშვით `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] მშრალად გაშვებული ტელემეტრიის ექსპორტი: უზრუნველყოს Prometheus სკრაპი ადგილობრივად.
- [ ] დაგეგმეთ საბურღი საბურღი ფანჯრის და ჩაწერეთ ესკალაციის კონტაქტები.
- [ ] ხელი მოაწერეთ საბურღი მტკიცებულების პაკეტს: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.