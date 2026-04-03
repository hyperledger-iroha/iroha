<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Cross-Dataspace Localnet Proof

ეს runbook ახორციელებს Nexus ინტეგრაციის მტკიცებულებას, რომ:

- ჩატვირთავს 4-თანხმიან ლოკალურ ქსელს ორი შეზღუდული პირადი მონაცემთა სივრცით (`ds1`, `ds2`),
- მარშრუტებს ანგარიშის ტრაფიკი თითოეულ მონაცემთა სივრცეში,
- ქმნის აქტივს თითოეულ მონაცემთა სივრცეში,
- ახორციელებს ატომური სვოპის დასახლებას მონაცემთა სივრცეში ორივე მიმართულებით,
- ადასტურებს უკან დაბრუნების სემანტიკას არასაკმარისად დაფინანსებული ფეხის წარდგენით და ნაშთების შემოწმებით უცვლელი რჩება.

კანონიკური ტესტია:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## სწრაფი გაშვება

გამოიყენეთ wrapper სკრიპტი საცავიდან root:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

ნაგულისხმევი ქცევა:

- აწარმოებს მხოლოდ მონაცემთა ჯვარედინი სივრცის დადასტურების ტესტს,
- კომპლექტი `NORITO_SKIP_BINDINGS_SYNC=1`,
- კომპლექტი `IROHA_TEST_SKIP_BUILD=1`,
- იყენებს `--test-threads=1`,
- გადის `--nocapture`.

## სასარგებლო პარამეტრები

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` ინახავს დროებით თანატოლთა საქაღალდეებს (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) კრიმინალისტიკისთვის.
- `--all-nexus` გადის `mod nexus::` (სრული Nexus ინტეგრაციის ქვეჯგუფი), არა მხოლოდ მტკიცებულების ტესტს.

## CI კარიბჭე

CI დამხმარე:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

გააკეთე მიზანი:

```bash
make check-nexus-cross-dataspace
```

ეს კარიბჭე ახორციელებს დეტერმინისტული მტკიცებულების შეფუთვას და ვერ ასრულებს სამუშაოს, თუ მონაცემთა ჯვარედინი სივრცე ატომურია
სვოპის სცენარი რეგრესია.

## მექანიკური ეკვივალენტური ბრძანებები

მიზნობრივი მტკიცებულების ტესტი:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

სრული Nexus ქვეჯგუფი:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## მოსალოდნელი მტკიცებულების სიგნალები- გამოცდა გადის.
- ერთი მოსალოდნელი გაფრთხილება ჩნდება განზრახ წარუმატებელი არასაკმარისი ანგარიშსწორების შესახებ:
  `settlement leg requires 10000 but only ... is available`.
- საბოლოო ბალანსის მტკიცება წარმატებულია შემდეგ:
  - წარმატებული წინსვლის გაცვლა,
  - წარმატებული საპირისპირო გაცვლა,
  - წარუმატებელი არასაკმარისი დაფინანსების სვოპ (უკან უცვლელი ნაშთები).

## მიმდინარე გადამოწმების სურათი

**2026 წლის 19 თებერვლისთვის**, ეს სამუშაო პროცესი გავიდა:

- მიზნობრივი ტესტი: `1 passed; 0 failed`,
- სრული Nexus ქვეჯგუფი: `24 passed; 0 failed`.