---
lang: ka
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# გადაუდებელი Canon და TTL პოლიტიკა (MINFO-6a)

საგზაო რუქის მითითება: **MINFO-6a — გადაუდებელი კანონი და TTL პოლიტიკა **.

ეს დოკუმენტი განსაზღვრავს უარმყოფელი სიის დონის წესებს, TTL აღსრულებას და მმართველობის ვალდებულებებს, რომლებიც ახლა იგზავნება Torii-ში და CLI-ში. ოპერატორებმა უნდა დაიცვან ეს წესები ახალი ჩანაწერების გამოქვეყნებამდე ან საგანგებო კანონების გამოძახებამდე.

## დონის განმარტებები

| იარუსი | ნაგულისხმევი TTL | განხილვის ფანჯარა | მოთხოვნები |
|------|-------------|--------------|-------------|
| სტანდარტული | 180 დღე (`torii.sorafs_gateway.denylist.standard_ttl`) | n/a | მოწოდებული უნდა იყოს `issued_at`. `expires_at` შეიძლება გამოტოვდეს; Torii ნაგულისხმევად არის `issued_at + standard_ttl` და უარყოფს უფრო ხანგრძლივ ფანჯრებს. |
| გადაუდებელი | 30 დღე (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 დღე (`torii.sorafs_gateway.denylist.emergency_review_window`) | მოითხოვს არაცარიელ `emergency_canon` იარლიყს, რომელიც მიუთითებს წინასწარ დამტკიცებულ კანონზე (მაგ., `csam-hotline`). `issued_at` + `expires_at` უნდა დაეშვას 30-დღიან ფანჯარაში და განხილვის მტკიცებულებამ უნდა მიუთითოს ავტოგენერირებული ვადა (`issued_at + review_window`). |
| მუდმივი | ვადის გასვლის გარეშე | n/a | დაცულია უმრავლესობის მმართველობის გადაწყვეტილებებისთვის. ჩანაწერებში უნდა იყოს ციტირებული არა ცარიელი `governance_reference` (ხმის ID, მანიფესტის ჰეში და ა.შ.). `expires_at` უარყოფილია. |

ნაგულისხმევი კონფიგურაცია რჩება `torii.sorafs_gateway.denylist.*`-ის მეშვეობით და `iroha_cli` ასახავს საზღვრებს არასწორი ჩანაწერების დასაჭერად Torii ფაილის ხელახლა ჩატვირთვამდე.

## სამუშაო პროცესი

1. **მოამზადეთ მეტამონაცემები:** მოიცავს `policy_tier`, `issued_at`, `expires_at` (როდესაც ეს შესაძლებელია) და `emergency_canon`/`governance_reference`10000000036X10000000036X10000000036X1.
2. **ადგილობრივად ვალიდაცია:** გაუშვით `iroha app sorafs gateway lint-denylist --path <denylist.json>`, რათა CLI განახორციელოს დონის სპეციფიკური TTL-ები და საჭირო ველები, სანამ ფაილის ჩადება ან ჩამაგრება მოხდება.
3. **გამოაქვეყნეთ მტკიცებულება:** დაურთეთ კანონის ID ან მმართველობის მითითება, რომელიც ციტირებულია ჩანაწერში GAR საქმის პაკეტში (დღის წესრიგის პაკეტი, რეფერენდუმის ოქმები და ა.შ.), რათა აუდიტორებმა შეძლონ გადაწყვეტილების მიკვლევა.
4. **გადახედეთ გადაუდებელ ჩანაწერებს:** გადაუდებელი წესების ავტომატური ვადა იწურება 30 დღის განმავლობაში. ოპერატორებმა უნდა დაასრულონ პოსტ-ფაქტო მიმოხილვა 7-დღიან ფანჯარაში და ჩაწერონ შედეგი სამინისტროს ტრეკერში/SoraFS მტკიცებულებათა მაღაზიაში.
5. **გადატვირთეთ Torii:** დადასტურების შემდეგ, განათავსეთ Denylist გზა `torii.sorafs_gateway.denylist.path`-ის საშუალებით და გადატვირთეთ/გადატვირთეთ Torii; გაშვების დრო ახორციელებს იგივე ლიმიტებს, სანამ ჩანაწერების დაშვებას აპირებს.

## ინსტრუმენტები და ცნობები

- Runtime Policy Enforcement მოქმედებს `sorafs::gateway::denylist`-ში (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) და ჩამტვირთველი ახლა იყენებს დონის მეტამონაცემებს `torii.sorafs_gateway.denylist.*` შენატანების ანალიზისას.
- CLI ვალიდაცია ასახავს მუშაობის დროის სემანტიკას `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) შიგნით. ლინტერი იშლება, როდესაც TTL-ები აღემატება კონფიგურირებულ ფანჯარას ან როდესაც არ არის სავალდებულო კანონის/მმართველობის მითითებები.
- კონფიგურაციის ღილაკები განსაზღვრულია `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) ქვეშ, ასე რომ ოპერატორებს შეუძლიათ შეცვალონ TTL-ები/განიხილონ ვადები, თუ მმართველობა ამტკიცებს სხვადასხვა საზღვრებს.
- საჯარო ნიმუშის უარყოფის სია (`docs/source/sorafs_gateway_denylist_sample.json`) ახლა ასახავს სამივე საფეხურს და უნდა იყოს გამოყენებული, როგორც კანონიკური შაბლონი ახალი ჩანაწერებისთვის.ეს დამცავი მოაჯირები აკმაყოფილებს საგზაო რუქის პუნქტს **MINFO-6a** გადაუდებელი კანონის სიის კოდიფიცირებით, შეუზღუდავი TTL-ების თავიდან აცილებით და მუდმივი ბლოკებისთვის აშკარა მმართველობის მტკიცებულების იძულებით.

## რეესტრის ავტომატიზაცია და მტკიცებულებების ექსპორტი

გადაუდებელი კანონის დამტკიცებამ უნდა შეადგინოს დეტერმინისტული რეესტრის სნეპშოტი და ა
diff bundle სანამ Torii ჩატვირთავს უარყოფის სიას. ხელსაწყოების ქვეშ
`xtask/src/sorafs.rs` პლუს CI აღკაზმულობა `ci/check_sorafs_gateway_denylist.sh`
მოიცავს მთელ სამუშაო პროცესს.

### Canonical Bundle Generation

1. ეტაპობრივი ჩანაწერები (როგორც წესი, ფაილი განიხილება მმართველობის მიერ) სამუშაოში
   დირექტორია.
2. კანონიკური და დალუქული JSON მეშვეობით:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   ბრძანება გამოსცემს ხელმოწერილი `.json` პაკეტს, Norito `.to`
   კონვერტი და Merkle-root ტექსტური ფაილი, რომელსაც ელოდება მმართველობის რეცენზენტები.
   შეინახეთ დირექტორია ქვეშ `artifacts/ministry/denylist_registry/` (ან თქვენი
   არჩეული მტკიცებულების ვედრო) ასე რომ `scripts/ministry/transparency_release.py` შეუძლია
   აიღეთ მოგვიანებით `--artifact denylist_bundle=<path>`-ით.
3. შეინახეთ გენერირებული `checksums.sha256` პაკეტის გვერდით, სანამ დააყენებთ მას
   SoraFS/GAR-მდე. CI-ის `ci/check_sorafs_gateway_denylist.sh` ვარჯიშობს იგივე
   `pack` დამხმარე ნიმუშების უარყოფის წინააღმდეგ, რათა გარანტირებული იყოს ხელსაწყოების მუშაობა
   ყოველი გამოშვება.

### Diff + აუდიტის ნაკრები

1. შეადარეთ ახალი პაკეტი წინა წარმოების სნეპშოტთან
   xtask diff დამხმარე:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON ანგარიშში ჩამოთვლილია ყველა დამატება/წაშლა და ასახავს მტკიცებულებებს
   `MinistryDenylistChangeV1`-ის მიერ მოხმარებული სტრუქტურა (მითითებული
   `docs/source/sorafs_gateway_self_cert.md` და შესაბამისობის გეგმა).
2. მიამაგრეთ `denylist_diff.json` ყველა კანონის მოთხოვნას (ეს ადასტურებს რამდენი
   ჩანაწერები შეეხო, რომელი იარუსი შეიცვალა და რა მტკიცებულება აქვს ჰეშის რუქებს
   კანონიკური შეკვრა).
3. როდესაც განსხვავებები გენერირებულია ავტომატურად (CI ან გამოშვების მილსადენები), ექსპორტი
   `denylist_diff.json` გზა `--artifact denylist_diff=<path>`-ის მეშვეობით, ასე რომ
   გამჭვირვალობის მანიფესტი აღრიცხავს მას გაწმენდილი მეტრიკის გვერდით. იგივე CI
   დამხმარე იღებს `--evidence-out <path>`, რომელიც აწარმოებს CLI შემაჯამებელ ნაბიჯს და
   აკოპირებს მიღებულ JSON-ს მოთხოვნილ ადგილას შემდგომი გამოქვეყნებისთვის.

### პუბლიკაცია და გამჭვირვალობა1. ჩააგდეთ პაკეტი + განსხვავებული არტეფაქტები კვარტალური გამჭვირვალობის დირექტორიაში
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). გამჭვირვალობა
   გათავისუფლების დამხმარე შეიძლება შემდეგ შეიცავდეს მათ:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. კვარტალურ ანგარიშში მიუთითეთ გენერირებული პაკეტი/განსხვავება
   (`docs/source/ministry/reports/<YYYY-Q>.md`) და მიამაგრეთ იგივე ბილიკები
   GAR ხმის მიცემის პაკეტი, რათა აუდიტორებმა შეძლონ მტკიცებულებების ბილიკის ხელახლა დაკვრა წვდომის გარეშე
   შიდა CI. `ci/check_sorafs_gateway_denylist.sh --მტკიცებულებების გატანა \
   artifacts/ministry/denylist_registry//denylist_evidence.json` ახლა
   ასრულებს შეფუთვა/განსხვავება/მტკიცებულება მშრალი გაშვებას (დარეკავს `iroha_cli app sorafs gateway-ს
   მტკიცებულება` თავსაბურავის ქვეშ), რათა ავტომატიზაციამ შეძლოს შეჯამებასთან ერთად
   კანონიკური ჩალიჩები.
3. გამოქვეყნების შემდეგ, დამაგრეთ მართვის დატვირთვა მეშვეობით
   `cargo xtask ministry-transparency anchor` (გამოძახებულია ავტომატურად
   `transparency_release.py` როდესაც მოცემულია `--governance-dir`) ასე რომ
   denylist რეესტრის დაიჯესტი გამოჩნდება იმავე DAG ხეში, როგორც გამჭვირვალობა
   გათავისუფლება.

ამ პროცესის შემდეგ იხურება "რეესტრის ავტომატიზაცია და მტკიცებულებების ექსპორტი"
უფსკრული გამოიძახა `roadmap.md:450`-ში და უზრუნველყოფს, რომ ყოველი საგანგებო კანონი
გადაწყვეტილება მიიღება რეპროდუცირებადი არტეფაქტებით, JSON განსხვავებებით და გამჭვირვალობის ჟურნალით
ჩანაწერები.

### TTL & Canon Evidence Helper

პაკეტის/განსხვავების წყვილის წარმოების შემდეგ გაუშვით CLI მტკიცებულების დამხმარე დასაჭერად
TTL შეჯამებები და გადაუდებელი განხილვის ვადები, რომლებსაც მმართველობა მოითხოვს:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

ბრძანება ჰეშირებს JSON წყაროს, ამოწმებს ყველა ჩანაწერს და გამოსცემს კომპაქტს
რეზიუმე, რომელიც შეიცავს:

- მთლიანი ჩანაწერები `kind`-ზე და პოლიტიკის საფეხურზე ყველაზე ადრეული/უახლესი
  დაფიქსირდა დროის ანაბეჭდები.
- `emergency_reviews[]` სია, რომელშიც ჩამოთვლილია ყოველი საგანგებო კანონი თავისთან ერთად
  დესკრიპტორი, მოქმედების ვადა, ნებადართული TTL და გამოთვლილი
  `review_due_by` ბოლო ვადა.

მიამაგრეთ `denylist_evidence.json` შეფუთული პაკეტის/განსხვავების გვერდით, რათა აუდიტორებმა შეძლონ
დაადასტურეთ TTL შესაბამისობა CLI-ის ხელახლა გაშვების გარეშე. CI სამუშაოები, რომლებიც უკვე წარმოქმნიან
პაკეტებს შეუძლიათ გამოიძახონ დამხმარე და გამოაქვეყნონ მტკიცებულების არტეფაქტი (მაგ
დარეკვა `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`), უზრუნველყოფა
ყველა კანონიერი მოთხოვნა მიწებია თანმიმდევრული შეჯამებით.

### მერკლის რეესტრის მტკიცებულება

MINFO-6-ში დანერგილი Merkle-ის რეესტრი ოპერატორებს ავალდებულებს გამოაქვეყნონ
root და ყოველი შესვლის მტკიცებულებები TTL შეჯამებასთან ერთად. სირბილისთანავე
მტკიცებულების დამხმარე, დააფიქსირე მერკლის არტეფაქტები:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```Snapshot JSON ჩაწერს BLAKE3 Merkle-ის ფესვს, ფოთლების რაოდენობას და თითოეულს
აღწერის/ჰეშის წყვილი, რათა GAR ხმებმა შეიძლება მიუთითოს ზუსტი ხე, რომელიც ჰეშირებულ იქნა.
`--norito-out`-ის მიწოდება ინახავს `.to` არტეფაქტს JSON-თან ერთად, რაც იძლევა საშუალებას
კარიბჭეები შთანთქავს რეესტრის ჩანაწერებს პირდაპირ Norito-ის საშუალებით, გახეხვის გარეშე
stdout. `merkle proof` ასხივებს მიმართულების ბიტებს და ძმების ჰეშებს ნებისმიერისთვის
ნულზე დაფუძნებული შესვლის ინდექსი, რაც მარტივს ხდის თითოეულისთვის ჩართვის მტკიცებულების მიმაგრებას
საგანგებო კანონი მოხსენიებულია GAR-ის მემორანდუმში - არჩევითი Norito ასლი ინახავს მტკიცებულებას
მზად არის წიგნში განაწილებისთვის. შემდეგ შეინახეთ JSON და Norito არტეფაქტები
TTL-ის შეჯამებისა და განსხვავებების პაკეტში, რათა გამჭვირვალობის გამოშვებები და მმართველობა
წამყვანები მიუთითებენ იმავე ფესვზე.