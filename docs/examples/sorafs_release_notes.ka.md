---
lang: ka
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI & SDK — გამოშვების შენიშვნები (v0.1.0)

## მაჩვენებლები
- `sorafs_cli` ახლა ახვევს მთელ შეფუთვის მილსადენს (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) ასე რომ, CI მორბენალი გამოიძახებს
  ერთი ორობითი ნაცვლად შეკვეთილი დამხმარე. ახალი ღილაკების ხელმოწერის ნაკადი ნაგულისხმევია
  `SIGSTORE_ID_TOKEN`, ესმის GitHub Actions OIDC პროვაიდერები და ასხივებს დეტერმინისტულ
  შეჯამება JSON ხელმოწერის პაკეტთან ერთად.
- მრავალ წყაროს მოპოვება *სკორბორტი* იგზავნება როგორც `sorafs_car`: ის ნორმალიზდება
  პროვაიდერის ტელემეტრია, ახორციელებს შესაძლებლობების ჯარიმებს, გრძელდება JSON/Norito ანგარიშებს და
  კვებავს ორკესტრის სიმულატორს (`sorafs_fetch`) საერთო რეესტრის სახელურის მეშვეობით.
  `fixtures/sorafs_manifest/ci_sample/`-ის ქვეშ მყოფი მოწყობილობები აჩვენებენ დეტერმინისტიკას
  შეყვანები და გამოსავლები, რომლებზეც მოსალოდნელია CI/CD განსხვავებული.
- გამოშვების ავტომატიზაცია კოდიფიცირებულია `ci/check_sorafs_cli_release.sh`-ში და
  `scripts/release_sorafs_cli.sh`. ახლა ყოველი გამოშვება არქივს მანიფესტთა პაკეტს,
  ხელმოწერა, `manifest.sign/verify` რეზიუმეები და ქულების დაფის სნეპშოტი.
  მიმომხილველებს შეუძლიათ არტეფაქტების მიკვლევა მილსადენის ხელახალი გაშვების გარეშე.

## განახლების ნაბიჯები
1. განაახლეთ გასწორებული ყუთები თქვენს სამუშაო სივრცეში:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. ხელახლა გაუშვით გამოშვების კარი ადგილობრივად (ან CI-ში), რათა დაადასტუროთ fmt/clippy/ტესტი დაფარვა:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. ხელმოწერილი არტეფაქტების და შეჯამებების რეგენერაცია კურირებული კონფიგურაციით:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   დააკოპირეთ განახლებული პაკეტები/მტკიცებულებები `fixtures/sorafs_manifest/ci_sample/`-ში, თუ
   გამოუშვით კანონიკური მოწყობილობების განახლებები.

## დადასტურება
- გამოშვების კარიბჭე: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` კარიბჭის წარმატების შემდეგ დაუყოვნებლივ).
- `ci/check_sorafs_cli_release.sh` გამომავალი: დაარქივებულია
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (მიმაგრებულია გამოშვების პაკეტზე).
- მანიფესტის პაკეტის დაიჯესტი: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- მტკიცებულების შემაჯამებელი დაიჯესტი: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- მანიფესტი დაიჯესტი (ქვემო დინების ატესტაციის ჯვარედინი შემოწმებისთვის):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json`-დან).

## შენიშვნები ოპერატორებისთვის
- Torii კარიბჭე ახლა ახორციელებს `X-Sora-Chunk-Range` შესაძლებლობების სათაურს. განახლება
  ნებადართული სიები, რათა კლიენტები, რომლებიც წარმოადგენენ ახალი ნაკადის ტოკენის ფარგლებს, დაიშვებიან; ძველი ნიშნები
  დიაპაზონის გარეშე პრეტენზია ჩამოიჭრება.
- `scripts/sorafs_gateway_self_cert.sh` აერთიანებს მანიფესტის გადამოწმებას. სირბილისას
  თვითდამოწმების აღკაზმულობა, მიაწოდეთ ახლად წარმოქმნილი მანიფესტის შეკვრა ისე, რომ შეფუთვა შეძლოს
  სწრაფად მარცხი ხელმოწერის დრიფტზე.
- ტელემეტრიის საინფორმაციო დაფებმა უნდა შეიტანონ ახალი შედეგების დაფის ექსპორტი (`scoreboard.json`)
  შეადარეთ პროვაიდერის უფლებამოსილება, წონის დავალებები და უარის მიზეზები.
- დაარქივეთ ოთხი კანონიკური შეჯამება ყოველი გაშვებით:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. მმართველობის ბილეთები მიუთითებენ ზუსტად ამ ფაილებზე
  დამტკიცება.

## მადლიერება
- Storage Team — ბოლომდე CLI კონსოლიდაცია, ბლოკ-გეგმის რენდერი და ანგარიშის დაფა
  ტელემეტრიული სანტექნიკა.
- Tooling WG — გამოშვების მილსადენი (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) და დეტერმინისტული მოწყობილობების ნაკრები.
- კარიბჭის ოპერაციები — შესაძლებლობების კარიბჭე, ნაკადის ნიშნის პოლიტიკის მიმოხილვა და განახლება
  თვითსერტიფიკაციის სათამაშო წიგნები.