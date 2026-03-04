---
lang: ka
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9df713c3e078ac2ccbd74eb215b91bb80d08306d0ca455dc122fde535601ce8
source_last_modified: "2026-01-18T10:42:52.828202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito სტრიმინგი

Norito Streaming განსაზღვრავს მავთულის ფორმატს, საკონტროლო ჩარჩოებს და საცნობარო კოდეკს
გამოიყენება პირდაპირი მედიის ნაკადებისთვის Torii-სა და SoraNet-ში. კანონიკური სპეციფიკა ცხოვრობს
`norito_streaming.md` სამუშაო სივრცეში root; ეს გვერდი ასუფთავებს ნაწილებს, რომლებიც
ოპერატორებს და SDK ავტორებს სჭირდებათ კონფიგურაციის შეხების წერტილები.

## მავთულის ფორმატი და კონტროლის თვითმფრინავი

- **მანიფესტები და ჩარჩოები.** `ManifestV1` და `PrivacyRoute*` აღწერს სეგმენტს
  ვადები, ნაწილების აღწერები და მარშრუტის მინიშნებები. საკონტროლო ჩარჩოები (`KeyUpdate`,
  `ContentKeyUpdate` და კადენციური გამოხმაურება) ცხოვრობს მანიფესტთან ერთად
  მაყურებელს შეუძლია დაადასტუროს ვალდებულებები დეკოდირებამდე.
- ** საბაზისო კოდეკი.** `BaselineEncoder`/`BaselineDecoder` აძლიერებს მონოტონურობას
  ნაწილის ID, დროის ანაბეჭდის არითმეტიკა და ვალდებულებების დადასტურება. მასპინძლებმა უნდა დარეკონ
  `EncodedSegment::verify_manifest` მაყურებლების ან რელეების მომსახურებამდე.
- **ფუნქციური ბიტები.** მოლაპარაკების შესაძლებლობები რეკლამირებს `streaming.feature_bits`
  (ნაგულისხმევი `0b11` = საბაზისო გამოხმაურება + კონფიდენციალურობის მარშრუტის პროვაიდერი) ასე რომ, რელე და
  კლიენტებს შეუძლიათ უარყონ თანატოლები დეტერმინისტული შესაძლებლობების შესატყვისი გარეშე.

## გასაღებები, ლუქსი და კადენცია

- **იდენტიფიკაციის მოთხოვნები.** სტრიმინგის კონტროლის ჩარჩოები ყოველთვის ხელმოწერილია
  Ed25519. გამოყოფილი გასაღებების მიწოდება შესაძლებელია მეშვეობით
  `streaming.identity_public_key`/`streaming.identity_private_key`; წინააღმდეგ შემთხვევაში
  კვანძის იდენტურობა ხელახლა გამოიყენება.
- **HPKE ლუქსი.** `KeyUpdate` ირჩევს ყველაზე დაბალ საერთო კომპლექტს; ლუქსი #1 არის
  სავალდებულო (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`),
  სურვილისამებრ `Kyber1024` განახლების გზა. ლუქსის არჩევანი ინახება
  სესიაზე და დამოწმებულია ყოველ განახლებაზე.
- **როტაცია.** გამომცემლები ასხივებენ ხელმოწერილი `KeyUpdate` ყოველ 64 MiB ან 5 წუთში.
  `key_counter` მკაცრად უნდა გაიზარდოს; რეგრესია მძიმე შეცდომაა.
  `ContentKeyUpdate` ავრცელებს მოძრავი ჯგუფის კონტენტის გასაღებს, რომელიც შეფუთულია ქვეშ
  შეთანხმებული HPKE კომპლექტი და კარიბჭეების სეგმენტის გაშიფვრა ID + მოქმედებით
  ფანჯარა.
- **Snapshots.** `StreamingSession::snapshot_state` და
  `restore_from_snapshot` მუდმივი `{session_id, key_counter, suite, sts_root,
  cadence state}` under `streaming.session_store_dir` (ნაგულისხმევი
  `./storage/streaming`). სატრანსპორტო გასაღებები ხელახლა მიიღება აღდგენისას, ამიტომ ავარია
  არ გაჟონოთ სესიის საიდუმლოებები.

## გაშვების კონფიგურაცია

- **საკვანძო მასალა.** მიაწოდეთ სპეციალური გასაღებები
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519
  multihash) და სურვილისამებრ Kyber მასალა მეშვეობით
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. ოთხივე უნდა იყოს
  აწმყო ნაგულისხმევი ნაგულისხმევის გადაფარვისას; `streaming.kyber_suite` იღებს
  `mlkem512|mlkem768|mlkem1024` (სხვა სახელი: `kyber512/768/1024`, ნაგულისხმევი
  `mlkem768`).
- **კოდეკის დამცავი მოაჯირები.** CABAC რჩება გამორთული, თუ build არ იძლევა ამის საშუალებას;
  შეფუთული RANS მოითხოვს `ENABLE_RANS_BUNDLES=1`. აღსრულება მეშვეობით
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` და სურვილისამებრ
  `streaming.codec.rans_tables_path` მორგებული მაგიდების მიწოდებისას. შეფუთული
- **SoraNet მარშრუტები.** `streaming.soranet.*` აკონტროლებს ანონიმურ ტრანსპორტს:
  `exit_multiaddr` (ნაგულისხმევი `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (ნაგულისხმევი 25 ms), `access_kind` (`authenticated` vs `read-only`), სურვილისამებრ
  `channel_salt`, `provision_spool_dir` (ნაგულისხმევი
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (ნაგულისხმევი 0,
  შეუზღუდავი), `provision_window_segments` (ნაგულისხმევი 4) და
  `provision_queue_capacity` (ნაგულისხმევი 256).
- **სინქრონიზაციის კარიბჭე.** `streaming.sync` გადართავს დრიფტის აღსრულებას აუდიოვიზუალისთვის
  ნაკადები: `enabled`, `observe_only`, `ewma_threshold_ms` და `hard_cap_ms`
  არეგულირებს, როდესაც სეგმენტები უარყოფილია დროის დრიფტის გამო.

## ვალიდაცია და მოწყობილობები

- კანონიკური ტიპის განსაზღვრებები და დამხმარეები ცხოვრობენ
  `crates/iroha_crypto/src/streaming.rs`.
- ინტეგრაციის გაშუქება ახორციელებს HPKE-ს ხელის ჩამორთმევას, შინაარსის გასაღების განაწილებას,
  და სნეპშოტის სიცოცხლის ციკლი (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  სტრიმინგის დასადასტურებლად გაუშვით `cargo test -p iroha_crypto streaming_handshake`
  ზედაპირზე ადგილობრივად.
- განლაგების, შეცდომების დამუშავებისა და სამომავლო განახლებების ღრმა ჩაძირვისთვის წაიკითხეთ
  `norito_streaming.md` საცავში root.