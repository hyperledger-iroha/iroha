---
lang: ka
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 სკამიანი ლუქსი

Iroha 3 სკამიანი კომპლექტი აორმაგებს ცხელ ბილიკებს, რომლებსაც ჩვენ ვეყრდნობით ფსონების დროს, საფასური
დატენვის, მტკიცებულების დადასტურების, დაგეგმვისა და მტკიცებულების საბოლოო წერტილები. ის მუშაობს როგორც
`xtask` ბრძანება დეტერმინისტული მოწყობილობებით (ფიქსირებული თესლი, ფიქსირებული გასაღები მასალა,
და სტაბილური მოთხოვნის დატვირთვა) ასე რომ, შედეგები რეპროდუცირებადია ჰოსტებში.

## ლუქსის გაშვება

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

დროშები:

- `--iterations` აკონტროლებს გამეორებას თითო სცენარის ნიმუშზე (ნაგულისხმევი: 64).
- `--sample-count` იმეორებს თითოეულ სცენარს მედიანას გამოსათვლელად (ნაგულისხმევი: 5).
- `--json-out|--csv-out|--markdown-out` აირჩიეთ გამომავალი არტეფაქტები (ყველა სურვილისამებრ).
- `--threshold` ადარებს მედიანებს საბაზისო საზღვრებთან (კომპლექტი `--no-threshold`
  გამოტოვება).
- `--flamegraph-hint` ანოტირებს Markdown ანგარიშს `cargo flamegraph`-ით
  სცენარის პროფილის ბრძანება.

CI წებო ცხოვრობს `ci/i3_bench_suite.sh`-ში და ნაგულისხმევია ზემოთ მოცემულ ბილიკებზე; კომპლექტი
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` ღამის გასაშვებად.

## სცენარები

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — გადამხდელი და სპონსორის დებეტი
  და დეფიციტის უარყოფა.
- `staking_bond` / `staking_slash` — ობლიგაციების/გაუქმების რიგში და მის გარეშე
  ჭრის.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  ხელმოწერის გადამოწმება ვალდებულების სერტიფიკატების, JDG ატესტაციებისა და ხიდის შესახებ
  მტკიცებულების ტვირთამწეობა.
- `commit_cert_assembly` — დაიჯესტის ასამბლეა ვალდებულების სერთიფიკატებისთვის.
- `access_scheduler` — კონფლიქტის გაცნობიერებული წვდომის კომპლექტის დაგეგმვა.
- `torii_proof_endpoint` — Axum proof ბოლო წერტილის ანალიზი + გადამოწმების ორმხრივი მოგზაურობა.

ყველა სცენარი აღრიცხავს მედიანურ ნანოწამებს თითო გამეორებაზე, გამტარუნარიანობაზე და ა
განმსაზღვრელი განაწილების მრიცხველი სწრაფი რეგრესიებისთვის. ზღურბლები ცხოვრობენ
`benchmarks/i3/thresholds.json`; bump საზღვრები იქ, როდესაც ტექნიკა იცვლება და
შეიტანეთ ახალი არტეფაქტი მოხსენებასთან ერთად.

## პრობლემების მოგვარება

- დაამაგრეთ CPU სიხშირე/გუბერნატორი მტკიცებულებების შეგროვებისას, რათა თავიდან აიცილოთ ხმაურიანი რეგრესი.
- გამოიყენეთ `--no-threshold` საძიებო სირბილებისთვის, შემდეგ ხელახლა ჩართეთ, როგორც კი საბაზისო იქნება
  განახლებული.
- ერთი სცენარის პროფილისთვის დააყენეთ `--iterations 1` და ხელახლა გაუშვით
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.