---
lang: ka
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GPU ბენჩმარკის გადაღების ისტორია (FASTPQ WP5-B)

ეს ფაილი გენერირებულია `python3 scripts/fastpq/update_benchmark_history.py`-ის მიერ.
ის აკმაყოფილებს FASTPQ 7 ეტაპის WP5-B მიწოდებას ყოველ შეფუთულ GPU-ს თვალყურის დევნით
საორიენტაციო არტეფაქტი, პოსეიდონის მიკროსკოპის მანიფესტი და დამხმარე ასლები
`benchmarks/`. განაახლეთ ძირითადი გადაღებები და ხელახლა გაუშვით სკრიპტი, როდესაც ახალი იქნება
შეფუთული მიწები ან ტელემეტრია საჭიროებს ახალ მტკიცებულებებს.

## სფერო და განახლების პროცესი

- შექმენით ან შეფუთეთ ახალი GPU გადაღებები (`scripts/fastpq/wrap_benchmark.py`-ის საშუალებით),
  მიამაგრეთ ისინი დაჭერის მატრიცას და ხელახლა გაუშვით ეს გენერატორი, რათა განაახლოთ
  მაგიდები.
- როდესაც Poseidon microbench მონაცემები არსებობს, ექსპორტი გააკეთეთ
  `scripts/fastpq/export_poseidon_microbench.py` და აღადგინეთ manifest გამოყენებით
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- ჩაწერეთ Merkle-ის ზღურბლის გადაღებები მათი JSON-ის შედეგების ქვეშ შენახვით
  `benchmarks/merkle_threshold/`; ეს გენერატორი ჩამოთვლის ცნობილ ფაილებს, ამიტომ აუდიტი
  შეუძლია ჯვარედინი მითითება CPU vs GPU ხელმისაწვდომობა.

## FASTPQ ეტაპი 7 GPU ბენჩმარკები

| შეკვრა | Backend | რეჟიმი | GPU backend | GPU ხელმისაწვდომია | მოწყობილობის კლასი | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|------------|-------------|--------- ------|-----|--------------------|--------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | დიახ | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | ლითონის | gpu | არცერთი | დიახ | ვაშლი-მ4 | Apple GPU 40 ბირთვიანი | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | ლითონის | gpu | ლითონის | დიახ | apple-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | ლითონის | gpu | ლითონის | დიახ | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | ლითონის | gpu | ლითონის | დიახ | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | დიახ | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> სვეტები: `Backend` მომდინარეობს პაკეტის სახელიდან; `Mode`/`GPU backend`/`GPU available`
> კოპირებულია შეფუთული `benchmarks` ბლოკიდან, რათა გამოაშკარავდეს CPU-ის გამოტოვებას ან გამოტოვებულ GPU-ს
> აღმოჩენა (მაგალითად, `gpu_backend=none` `Mode=gpu`-ის მიუხედავად). SU = სიჩქარის კოეფიციენტი (CPU/GPU).

## Poseidon Microbench Snapshots

`benchmarks/poseidon/manifest.json` აერთიანებს ნაგულისხმევი პოსეიდონის წინააღმდეგ
microbench ეშვება ექსპორტირებული თითოეული Metal bundle. ქვემოთ მოყვანილი ცხრილი განახლებულია
გენერატორის სკრიპტი, ამიტომ CI და მმართველობის მიმოხილვები შეიძლება განსხვავდებოდეს ისტორიული სიჩქარით
შეფუთული FASTPQ ანგარიშების ამოხსნის გარეშე.

| რეზიუმე | შეკვრა | დროის ანაბეჭდი | ნაგულისხმევი ms | სკალარული ms | აჩქარება |
|---------|--------|-----------|-----------|----------|--------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## მერკლის ზღურბლის გადალახვასაცნობარო ჩანაწერები შეგროვდა მეშვეობით
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
ცხოვრობს `benchmarks/merkle_threshold/` ქვეშ. სიის ჩანაწერები აჩვენებს თუ არა მასპინძელს
გამოაშკარავებული ლითონის მოწყობილობები, როდესაც წმენდა გაიქცა; GPU ჩართული გადაღებები უნდა იყოს მოხსენებული
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon Capture (`takemiyacStudio.lan_25.0.0_arm64`) არის კანონიკური GPU საბაზისო ხაზი, რომელიც გამოიყენება `docs/source/benchmarks.md`-ში; macOS 14 ჩანაწერები რჩება მხოლოდ CPU-ის საბაზისო ხაზებად იმ გარემოებისთვის, რომლებსაც არ შეუძლიათ ლითონის მოწყობილობების გამოვლენა.

## მწკრივის გამოყენების სნეპშოტები

`scripts/fastpq/check_row_usage.py`-ის საშუალებით გადაღებული მოწმის დეკოდები ადასტურებს გადაცემას
გაჯეტის რიგის ეფექტურობა. შეინახეთ JSON არტეფაქტები `artifacts/fastpq_benchmarks/`-ში
და ეს გენერატორი შეაჯამებს აუდიტორებისთვის დაფიქსირებულ გადარიცხვის კოეფიციენტებს.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — პარტიები=2, გადაცემის_ფარდობა საშუალო=0,629 (მინ=0,625, მაქს=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — პარტიები=2, გადაცემის_ფარდობა საშუალო=0,619 (მინ=0,613, მაქს=0,625)