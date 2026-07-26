---
lang: ka
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ Metal Kernel Suite

Apple Silicon backend-ი აგზავნის ერთ `fastpq.metallib`-ს, რომელიც შეიცავს ყველა
Metal Shading Language (MSL) ბირთვი, რომელსაც ახორციელებს პროვერტი. ეს შენიშვნა განმარტავს
ხელმისაწვდომი შესვლის წერტილები, მათი ჯგუფის საზღვრები და დეტერმინიზმი
გარანტიები, რომლებიც აქცევს GPU-ს გზას ურთიერთშემცვლელს სკალარულ სანაცვლოდ.

კანონიკური განხორციელება ცხოვრობს ქვეშ
`crates/fastpq_prover/metal/kernels/` და შედგენილია მიერ
`crates/fastpq_prover/build.rs` ყოველთვის, როცა `fastpq-gpu` ჩართულია macOS-ზე.
გაშვების მეტამონაცემები (`metal_kernel_descriptors`) ასახავს ქვემოთ მოცემულ ინფორმაციას.
ეტალონები და დიაგნოსტიკა შეიძლება გამოავლინოს იგივე ფაქტები პროგრამულად.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## ბირთვის ინვენტარი| შესვლის წერტილი | ოპერაცია | ძაფთა ჯგუფის ქუდი | კრამიტის სცენის თავსახური | შენიშვნები |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | FFT-ის გადაგზავნა კვალის სვეტებზე | 256 თემა | 32 ეტაპი | იყენებს საზიარო მეხსიერების ფილებს პირველი ეტაპებისთვის და მიმართავს ინვერსიულ სკალირებას, როდესაც დამგეგმავი ითხოვს IFFT რეჟიმს.
| `fastpq_fft_post_tiling` | ასრულებს FFT/IFFT/LDE კრამიტის სიღრმის მიღწევის შემდეგ | 256 თემა | — | აწარმოებს დანარჩენ პეპლებს პირდაპირ მოწყობილობის მეხსიერებიდან და ამუშავებს საბოლოო კოსეტს/შებრუნებულ ფაქტორებს ჰოსტში დაბრუნებამდე.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | დაბალი ხარისხის გაფართოება სვეტების გასწვრივ | 256 თემა | 32 ეტაპი | კოპირებს კოეფიციენტებს შეფასების ბუფერში, ახორციელებს მოპირკეთებულ ეტაპებს კონფიგურირებული კოსეტით და საბოლოო ეტაპებს უტოვებს `fastpq_fft_post_tiling`-ს, როდესაც საჭიროა.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | ჰეშერით სვეტები და გამოთვალეთ სიღრმე-1 მშობელი ერთ გადასასვლელში | 256 თემა | — | აწარმოებს იგივე აბსორბციას/პერმუტაციას, როგორც `poseidon_hash_columns`, ინახავს ფოთლის მონელებას პირდაპირ გამომავალ ბუფერში და დაუყოვნებლივ იკეცება თითოეული `(left,right)` წყვილი `fastpq:v1:trace:node` დომენის ქვეშ, ასე რომ `(⌈columns / 2⌉)` მშობლებს დაეშვება. კენტი სვეტების რაოდენობა ასახავს ბოლო ფურცელს მოწყობილობაზე, რაც გამორიცხავს შემდგომ ბირთვს და CPU-ის სარეზერვო ნაწილს Merkle-ის პირველი ფენისთვის.
| `poseidon_permute` | Poseidon2 პერმუტაცია (STATE_WIDTH = 3) | 256 თემა | — | ძაფთა ჯგუფები ქეშირებენ მრგვალ კონსტანტებს/MDS სტრიქონებს ძაფთა ჯგუფის მეხსიერებაში, MDS სტრიქონების კოპირებას თითო ძაფების რეგისტრებში და ამუშავებენ მდგომარეობებს 4-მდგომარეობის ნაწილებში, ასე რომ, ყოველი მრგვალი მუდმივი ამოღება ხელახლა გამოიყენება მრავალ მდგომარეობაში, სანამ წინსვლამდე მიდის. რაუნდები სრულად გაშლილი რჩება და ყველა ზოლი კვლავ გადის რამდენიმე შტატში, რაც უზრუნველყოფს ≥4096 ლოგიკურ ძაფს თითო გაგზავნაზე. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` ჩამაგრება გაშვების სიგანე და თითო ზოლის პარტია ხელახლა აშენების გარეშე metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

აღწერები ხელმისაწვდომია გაშვების დროს მეშვეობით
`fastpq_prover::metal_kernel_descriptors()` ხელსაწყოებისთვის, რომელსაც სურს ჩვენება
იგივე მეტამონაცემები.

## დეტერმინისტული ოქროს არითმეტიკა- ყველა ბირთვი მუშაობს Goldilocks ველზე მითითებული დამხმარეებით
  `field.metal` (მოდულური დამატება/მულ/ქვე, ინვერსიები, `pow5`).【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE ეტაპები ხელახლა იყენებს იმავე ტრიალ ცხრილებს, რომლებსაც CPU დამგეგმავი აწარმოებს.
  `compute_stage_twiddles` წინასწარ ითვლის ერთ თვიდლს თითო ეტაპზე და ჰოსტზე
  ატვირთავს მასივს ბუფერულ სლოტში 1 ყოველი გაგზავნის წინ, რაც გარანტიას იძლევა
  GPU გზა იყენებს ერთიანობის იდენტურ ფესვებს.【crates/fastpq_prover/src/metal.rs:1527】
- კოსეტის გამრავლება LDE-სთვის შერწყმულია საბოლოო ეტაპზე, ასე რომ GPU არასოდეს
  განსხვავდება CPU კვალის განლაგებიდან; ჰოსტი ნული ავსებს შეფასების ბუფერს
  გაგზავნამდე, ბალიშის ქცევის განმსაზღვრელი შენარჩუნება.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib თაობა

`build.rs` აგროვებს ინდივიდუალურ `.metal` წყაროებს `.air` ობიექტებში და შემდეგ
აკავშირებს მათ `fastpq.metallib`-ში, ახორციელებს ზემოთ ჩამოთვლილ ყველა შესვლის პუნქტის ექსპორტს.
`FASTPQ_METAL_LIB`-ის დაყენება ამ გზაზე (ამას აკეთებს build სკრიპტი
ავტომატურად) საშუალებას აძლევს Runtime-ს, განურჩევლად ბიბლიოთეკის განმსაზღვრელად ჩატვირთოს
სადაც `cargo`-მა მოათავსა კონსტრუქციის არტეფაქტები.【crates/fastpq_prover/build.rs:45】

CI გაშვებებთან თანასწორობისთვის შეგიძლიათ ბიბლიოთეკის ხელით რეგენერაცია:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## ძაფთა ჯგუფის ზომის ევრისტიკა

`metal_config::fft_tuning` აკავშირებს მოწყობილობის შესრულების სიგანეს და მაქსიმალურ ძაფებს თითო
threadgroup შევიდა დამგეგმავი, ასე რომ Runtime დისპეტჩერები პატივს სცემენ ტექნიკის ლიმიტებს.
ნაგულისხმევი დამაგრება 32/64/128/256 ზოლზე, როგორც ლოგის ზომა იზრდება, და
კრამიტის სიღრმე ახლა მიდის ხუთი ეტაპიდან ოთხამდე `log_len ≥ 12`-ზე, შემდეგ ინარჩუნებს
საზიარო მეხსიერების საშვი აქტიურია 12/14/16 ეტაპად, როგორც კი კვალი გადაკვეთს
`log_len ≥ 18/20/22` სამუშაოს ჩაბარებამდე კრამიტის შემდგომ ბირთვში. ოპერატორი
გადაფარავს (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) გადის
`FftArgs::threadgroup_lanes`/`local_stage_limit` და გამოიყენება ბირთვების მიერ
ზემოთ metallib-ის აღდგენის გარეშე.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

გამოიყენეთ `fastpq_metal_bench` გადაწყვეტილი ტუნინგის მნიშვნელობების დასაფიქსირებლად და დაადასტურეთ, რომ
მრავალპასიანი ბირთვები განხორციელდა (`post_tile_dispatches` JSON-ში) ადრე
საორიენტაციო პაკეტის გაგზავნა.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】