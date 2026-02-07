---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Gateway & DNS Kickoff Runbook

ეს პორტალი ასახავს კანონიკურ წიგნს
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
ის იჭერს ოპერაციულ დამცავ სარდაფებს დეცენტრალიზებული DNS & Gateway-ისთვის
სამუშაო ნაკადი, რათა ქსელებმა, ოპერაციებმა და დოკუმენტაციამ შეძლოს რეპეტიცია
ავტომატიზაციის დასტა 2025-03 წლის დაწყებამდე.

## სფერო და მიწოდება

- შეაერთეთ DNS (SF‑4) და კარიბჭე (SF‑5) ეტაპები დეტერმინისტული რეპეტიციით
  ჰოსტის დერივაცია, გადამწყვეტი დირექტორია გამოშვებები, TLS/GAR ავტომატიზაცია და მტკიცებულება
  ხელში ჩაგდება.
- შეინახეთ დაწყების მონაცემები (დღის წესრიგი, მოწვევა, დასწრების ტრეკერი, GAR ტელემეტრია
  Snapshot) სინქრონიზებულია მფლობელის უახლეს დავალებებთან.
- შექმენით აუდიტორული არტეფაქტის ნაკრები მმართველობის მიმომხილველებისთვის: გადამწყვეტი
  დირექტორია გამოშვების შენიშვნები, კარიბჭის ზონდის ჟურნალები, შესაბამისობის აღკაზმულობის გამომავალი და
  Docs/DevRel-ის შეჯამება.

## როლები და პასუხისმგებლობები

| სამუშაო ნაკადი | პასუხისმგებლობა | საჭირო არტეფაქტები |
|------------|-----------------|-------------------|
| ქსელის TL (DNS დასტა) | შეინარჩუნეთ განმსაზღვრელი ჰოსტის გეგმა, გაუშვით RAD კატალოგის გამოშვებები, გამოაქვეყნეთ გადამწყვეტი ტელემეტრიის შეყვანები. | `artifacts/soradns_directory/<ts>/`, განსხვავდება `docs/source/soradns/deterministic_hosts.md`, RAD მეტამონაცემებისთვის. |
| Ops Automation Lead (კარიბჭე) | შეასრულეთ TLS/ECH/GAR ავტომატიზაციის წვრთნები, გაუშვით `sorafs-gateway-probe`, განაახლეთ PagerDuty კაკვები. | `artifacts/sorafs_gateway_probe/<ts>/`, ზონდი JSON, `ops/drill-log.md` ჩანაწერები. |
| QA Guild & Tooling WG | გაუშვით `ci/check_sorafs_gateway_conformance.sh`, მოაწესრიგეთ მოწყობილობები, დაარქივეთ Norito თვითდამოწმების პაკეტები. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | აღბეჭდეთ ოქმები, განაახლეთ დიზაინის წინასწარ წაკითხული + დანართები და გამოაქვეყნეთ მტკიცებულებების შეჯამება ამ პორტალზე. | განახლებულია `docs/source/sorafs_gateway_dns_design_*.md` ფაილები და გაშვების შენიშვნები. |

## შეყვანა და წინაპირობები

- განმსაზღვრელი ჰოსტის სპეციფიკა (`docs/source/soradns/deterministic_hosts.md`) და
  გადამწყვეტი ატესტაციის ხარაჩო (`docs/source/soradns/resolver_attestation_directory.md`).
- კარიბჭის არტეფაქტები: ოპერატორის სახელმძღვანელო, TLS/ECH ავტომატიზაციის დამხმარეები,
  პირდაპირი რეჟიმის მითითება და სამუშაო პროცესის თვითდამოწმება `docs/source/sorafs_gateway_*`-ში.
- ხელსაწყოები: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` და CI დამხმარეები
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- საიდუმლოებები: GAR გამოშვების გასაღები, DNS/TLS ACME სერთიფიკატები, PagerDuty მარშრუტიზაციის გასაღები,
  Torii ავტორიზაციის ჟეტონი გადამწყვეტი ამოღებისთვის.

## ფრენის წინ საკონტროლო სია

1. დაადასტურეთ დამსწრეები და დღის წესრიგი განახლებით
   `docs/source/sorafs_gateway_dns_design_attendance.md` და მიმოქცევაში
   მიმდინარე დღის წესრიგი (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. სასცენო არტეფაქტის ფესვები, როგორიცაა
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` და
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. განაახლეთ მოწყობილობები (GAR მანიფესტები, RAD მტკიცებულებები, კარიბჭის შესაბამისობის პაკეტები) და
   დარწმუნდით, რომ `git submodule` მდგომარეობა ემთხვევა რეპეტიციის უახლეს ტეგს.
4. შეამოწმეთ საიდუმლოებები (Ed25519 გამოშვების გასაღები, ACME ანგარიშის ფაილი, PagerDuty ჟეტონი) არის
   აწმყო და ემთხვევა სარდაფის საკონტროლო ჯამებს.
5. კვამლის ტესტის ტელემეტრიული სამიზნეები (Pushgateway ბოლო წერტილი, GAR Grafana დაფა) მანამდე
   ბურღამდე.

## ავტომატიზაციის რეპეტიციის ნაბიჯები

### განმსაზღვრელი მასპინძლის რუკა და RAD კატალოგის გამოშვება

1. გაუშვით დეტერმინისტული მასპინძლის წარმოშობის დამხმარე შემოთავაზებული მანიფესტის წინააღმდეგ
   დააყენეთ და დაადასტურეთ, რომ არ არის დრიფტი
   `docs/source/soradns/deterministic_hosts.md`.
2. შექმენით გადამწყვეტი დირექტორიას ნაკრები:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ჩაწერეთ დაბეჭდილი დირექტორია ID, SHA-256 და გამომავალი ბილიკები შიგნით
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` და დაწყება
   წუთები.

### DNS ტელემეტრიის გადაღება

- კუდის გამხსნელის გამჭვირვალობის ჟურნალი ≥10 წუთის განმავლობაში გამოყენებით
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- გაიტანეთ Pushgateway მეტრიკა და დაარქივეთ NDJSON სნეპშოტები გაშვებასთან ერთად
  ID დირექტორია.

### კარიბჭის ავტომატიზაციის წვრთნები

1. შეასრულეთ TLS/ECH ზონდი:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. გაუშვით შესაბამისობის აღკაზმულობა (`ci/check_sorafs_gateway_conformance.sh`) და
   თვითდამოწმებული დამხმარე (`scripts/sorafs_gateway_self_cert.sh`) განაახლოს
   Norito ატესტაციის ნაკრები.
3. გადაიღეთ PagerDuty/Webhook მოვლენები, რათა დაამტკიცოთ, რომ ავტომატიზაციის გზა ბოლომდე მუშაობს
   დასასრული.

### მტკიცებულებათა შეფუთვა

- განაახლეთ `ops/drill-log.md` დროის შტამპებით, მონაწილეებით და გამოძიების ჰეშებით.
- შეინახეთ არტეფაქტები გაშვებული ID დირექტორიების ქვეშ და გამოაქვეყნეთ აღმასრულებელი რეზიუმე
  Docs/DevRel შეხვედრის წუთებში.
- დააკავშირეთ მტკიცებულებათა ნაკრები მმართველობის ბილეთში დაწყების განხილვამდე.

## სესიის ფასილიტაცია და მტკიცებულებების გადაცემა

- ** მოდერატორის ვადები:**  
  - T‑24h — პროგრამის მენეჯმენტი აქვეყნებს შეხსენებას + დღის წესრიგის/დასწრების სურათს `#nexus-steering`-ში.  
  - T‑2h — ქსელის TL განაახლებს GAR ტელემეტრიის სურათს და ჩაწერს დელტას `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`-ში.  
  - T‑15m — Ops Automation ამოწმებს ზონდის მზადყოფნას და წერს აქტიური გაშვების ID-ს `artifacts/sorafs_gateway_dns/current`-ში.  
  - ზარის დროს — მოდერატორი აზიარებს ამ წიგნს და ანიჭებს ცოცხალ სკრიპტს; Docs/DevRel აფიქსირებს მოქმედების ერთეულებს ხაზში.
- **წუთის შაბლონი:** დააკოპირეთ ჩონჩხი
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ასევე ასახულია პორტალში
  bundle) და ჩაიდინეთ ერთი შევსებული ინსტანცია თითო სესიაზე. ჩართეთ დამსწრე როლი,
  გადაწყვეტილებები, ქმედებები, მტკიცებულებების ჰეშები და გამოჩენილი რისკები.
- **მტკიცებულებების ატვირთვა:** ჩაწერეთ `runbook_bundle/` დირექტორია რეპეტიციიდან,
  დაურთეთ გაცემული ოქმები PDF, ჩაწერეთ SHA-256 ჰეშები წუთებში + დღის წესრიგში,
  შემდეგ დაარეგისტრირეთ მმართველობის მიმომხილველი მეტსახელი, როგორც კი ატვირთავს მიწას
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## მტკიცებულების სნეპშოტი (2025 წლის მარტის დასაწყისი)

უახლესი რეპეტიცია/ცოცხალი არტეფაქტები, რომლებიც მითითებულია საგზაო რუკასა და მმართველობაში
წუთი ცხოვრობს `s3://sora-governance/sorafs/gateway_dns/` თაიგულის ქვეშ. ჰეშები
ქვემოთ ასახავს კანონიკურ მანიფესტს (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **მშრალი გაშვება — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - შეკვრა ტარბოლი: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - წუთები PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- ** ცოცხალი სახელოსნო — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(მოლოდინშია ატვირთვა: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel დაამატებს SHA-256-ს, როგორც კი გადაღებული PDF დაეშვება პაკეტში.)_

## დაკავშირებული მასალა

- [Gateway ოპერაციების სათამაშო წიგნი] (./operations-playbook.md)
- [SoraFS დაკვირვების გეგმა] (./observability-plan.md)
- [დეცენტრალიზებული DNS & Gateway ტრეკერი] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)