---
lang: ka
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% კონტენტის ჰოსტინგის შესახვევი
% Iroha ბირთვი

# კონტენტის ჰოსტინგის ხაზი

შინაარსის ზოლი ინახავს მცირე სტატიკურ პაკეტებს (ტარის არქივებს) ჯაჭვზე და ემსახურება
ინდივიდუალური ფაილები პირდაპირ Torii-დან.

- **გამოქვეყნება**: გაგზავნეთ `PublishContentBundle` tar არქივით, სურვილისამებრ ვადის გასვლის შემდეგ
  სიმაღლე და სურვილისამებრ მანიფესტი. პაკეტის ID არის blake2b ჰეში
  ტარბოლი. Tar ჩანაწერები უნდა იყოს რეგულარული ფაილები; სახელები არის ნორმალიზებული UTF-8 ბილიკები.
  ზომა/გზა/ფაილების დათვლის ქუდები მოდის `content` კონფიგურაციიდან (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  მანიფესტები მოიცავს Norito-ინდექსის ჰეშს, მონაცემთა სივრცეს/ხაზს, ქეშის პოლიტიკას
  (`max_age_seconds`, `immutable`), ავტორიზაციის რეჟიმი (`public` / `role:<role>` /
  `sponsor:<uaid>`), შენარჩუნების პოლიტიკის ჩანაცვლება და MIME უგულებელყოფა.
- **გამორიცხვა**: ტარის ტვირთამწეობა იშლება (ნაგულისხმევი 64 KiB) და ინახება ერთხელ
  ჰეში მითითებების დათვლით; საპენსიო bundle decrements და prunes მოცულობით.
- **სერვისი**: Torii ავლენს `GET /v1/content/{bundle}/{path}`-ს. პასუხების ნაკადი
  პირდაპირ chunk-ის მაღაზიიდან `ETag` = ფაილის ჰეშით, `Accept-Ranges: bytes`,
  დიაპაზონის მხარდაჭერა და Cache-Control მიღებული მანიფესტიდან. კითხულობს პატივისცემას
  მანიფესტის ავტორიზაციის რეჟიმი: როლური და სპონსორებით დახურული პასუხები მოითხოვს კანონიკურს
  მოთხოვნის სათაურები (`X-Iroha-Account`, `X-Iroha-Signature`) ხელმოწერილისთვის
  ანგარიში; დაკარგული/ვადაგასული პაკეტების დაბრუნება 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (ან `--root <dir>`) ახლა
  ავტომატურად ქმნის მანიფესტს, გამოსცემს არასავალდებულო `--manifest-out/--bundle-out` და
  იღებს `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  და `--expires-at-height` უგულებელყოფს. `iroha content pack --root <dir>` აშენებს
  დეტერმინისტული ტარბოლი + მანიფესტი არაფრის წარდგენის გარეშე.
- **კონფიგურაცია**: ქეში/ავტორის სახელურები მუშაობს `content.*` ქვეშ `iroha_config`-ში
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) და ძალაშია გამოქვეყნების დროს.
- **SLO + ლიმიტები**: `content.max_requests_per_second` / `request_burst` და
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` ქუდი წაკითხვის მხარეს
  გამტარუნარიანობა; Torii ახორციელებს როგორც ბაიტების მომსახურებას, ასევე ექსპორტს
  `torii_content_requests_total`, `torii_content_request_duration_seconds` და
  `torii_content_response_bytes_total` მეტრიკა შედეგების ლეიბლებით. შეყოვნება
  სამიზნეები ცხოვრობენ `content.target_p50_latency_ms` ქვეშ /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **ბოროტად გამოყენების კონტროლი**: შეფასების თაიგულები ჩართულია UAID/API ნიშნით/დისტანციური IP-ით და
  სურვილისამებრ PoW მცველი (`content.pow_difficulty_bits`, `content.pow_header`) შეუძლია
  საჭიროა წაკითხვამდე. DA ზოლის განლაგების ნაგულისხმევი პარამეტრები მოდის
  `content.stripe_layout` და ეხმიანება ქვითრებში/მანიფესტის ჰეშებს.
- ** ქვითრები და DA-ს მტკიცებულებები **: წარმატებული პასუხები ერთვის
  `sora-content-receipt` (base64 Norito ჩარჩოში `ContentDaReceipt` ბაიტი) ტარება
  `bundle_id`, `path`, `file_hash`, `served_bytes`, სერვისული ბაიტის დიაპაზონი,
  `chunk_root` / `stripe_layout`, სურვილისამებრ PDP ვალდებულება და დროის ანაბეჭდი
  კლიენტებს შეუძლიათ დაამაგრონ ის, რაც მოიტანეს სხეულის ხელახლა წაკითხვის გარეშე.

ძირითადი მითითებები:- მონაცემთა მოდელი: `crates/iroha_data_model/src/content.rs`
- შესრულება: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii დამმუშავებელი: `crates/iroha_torii/src/content.rs`
- CLI დამხმარე: `crates/iroha_cli/src/content.rs`