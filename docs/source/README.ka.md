---
lang: ka
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama Docs ინდექსი

ეს ინდექსი აკავშირებს IVM, Kotodama და IVM-პირველი მილსადენის საპროექტო და საცნობარო დოკუმენტებს. 日本語訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM არქიტექტურა და ენობრივი რუქა: `../../ivm.md`
- IVM syscall ABI: `ivm_syscalls.md`
- გენერირებული syscall მუდმივები: `ivm_syscalls_generated.md` (გაუშვით `make docs-syscalls` განახლებისთვის)
- IVM ბაიტიკოდის სათაური: `ivm_header.md`
- Kotodama გრამატიკა და სემანტიკა: `kotodama_grammar.md`
- Kotodama მაგალითები და syscall რუკები: `kotodama_examples.md`
- ტრანზაქციის მილსადენი (IVM‑პირველი): `../../new_pipeline.md`
- Torii Contracts API (მანიფესტები): `torii_contracts_api.md`
- უნივერსალური ანგარიში/UAID ოპერაციების სახელმძღვანელო: `universal_accounts_guide.md`
- JSON შეკითხვის კონვერტი (CLI / tooling): `query_json.md`
- Norito ნაკადის მოდულის მითითება: `norito_streaming.md`
- Runtime ABI ნიმუშები: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (დანართები, პროვერტი, ხმების რაოდენობა): `zk_app_api.md`
- Torii ZK დანართები/პროვერების ჩანართი: `zk/prover_runbook.md`
- Torii ZK App API ოპერატორის სახელმძღვანელო (დანართები/პროვერტი; კრატის დოკუმენტი): `../../crates/iroha_torii/docs/zk_app_api.md`
- VK/proof სასიცოცხლო ციკლი (რეგისტრი, ვერიფიკაცია, ტელემეტრია): `zk/lifecycle.md`
- Torii ოპერატორის დამხმარე საშუალებები (ხილვადობის ბოლო წერტილები): `references/operator_aids.md`
- Nexus ნაგულისხმევი ხაზის სწრაფი დაწყება: `quickstart/default_lane.md`
- MOCHI ზედამხედველის სწრაფი დაწყება და არქიტექტურა: `mochi/index.md`
- JavaScript SDK სახელმძღვანელო (სწრაფი დაწყება, კონფიგურაცია, გამოქვეყნება): `sdk/js/index.md`
- Swift SDK პარიტეტი/CI დაფები: `references/ios_metrics.md`
- მმართველობა: `../../gov.md`
- დომენის დადასტურებები (კომიტეტები, პოლიტიკა, ვალიდაცია): `domain_endorsements.md`
- JDG ატესტაციები (ხაზგარეშე ვალიდაციის ინსტრუმენტი): `jdg_attestations.md`
- დაზუსტების კოორდინაციის მოთხოვნა: `coordination_llm_prompts.md`
- საგზაო რუკა: `../../roadmap.md`
- Docker შემქმნელის გამოსახულების გამოყენება: `docker_build.md`

გამოყენების რჩევები
- შექმენით და გაუშვით მაგალითები `examples/`-ში გარე ხელსაწყოების გამოყენებით (`koto_compile`, `ivm_run`):
  - `make examples-run` (და `make examples-inspect` თუ `ivm_tool` ხელმისაწვდომია)
- არჩევითი ინტეგრაციის ტესტები (ნაგულისხმევად იგნორირებულია) მაგალითებისთვის და სათაურის შემოწმებები პირდაპირ ეთერშია `integration_tests/tests/`.მილსადენის კონფიგურაცია
- ყველა გაშვების ქცევა კონფიგურირებულია `iroha_config` ფაილების საშუალებით. გარემოს ცვლადები არ გამოიყენება ოპერატორებისთვის.
- გათვალისწინებულია გონივრული ნაგულისხმევი პარამეტრები; განლაგების უმეტესობას ცვლილებები არ დასჭირდება.
- შესაბამისი გასაღებები `[pipeline]`-ში:
  - `dynamic_prepass`: ჩართეთ IVM მხოლოდ წაკითხვადი prepass წვდომის კომპლექტების მისაღებად (ნაგულისხმევი: true).
  - `access_set_cache_enabled`: ქეში მიღებული წვდომის ნაკრები `(code_hash, entrypoint)`-ზე; გამორთეთ მინიშნებების გამართვა (ნაგულისხმევი: true).
  - `parallel_overlay`: პარალელურად გადაფარების აგება; commit რჩება დეტერმინისტული (ნაგულისხმევი: true).
  - `gpu_key_bucket`: არჩევითი კლავიშების ჩაყრა გრაფიკის წინასწარი გავლისთვის სტაბილური რადიქსის გამოყენებით `(key, tx_idx, rw_flag)`-ზე; დეტერმინისტული CPU სარეზერვო ყოველთვის აქტიურია (ნაგულისხმევი: false).
  - `cache_size`: გლობალური IVM წინასწარი გაშიფვრის ქეშის მოცულობა (შენახულია დეკოდირებული ნაკადები). ნაგულისხმევი: 128. გაზრდამ შეიძლება შეამციროს გაშიფვრის დრო განმეორებითი შესრულებისთვის.

დოკუმენტების სინქრონიზაციის შემოწმებები
- Syscall მუდმივები (docs/source/ivm_syscalls_generated.md)
  - რეგენერაცია: `make docs-syscalls`
  - შეამოწმეთ მხოლოდ: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI ცხრილი (crates/ivm/docs/syscalls.md)
  - შეამოწმეთ მხოლოდ: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - განაახლეთ გენერირებული სექცია (და კოდი დოკუმენტების ცხრილი): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer-ABI ცხრილები (crates/ivm/docs/pointer_abi.md და ivm.md)
  - შეამოწმეთ მხოლოდ: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - განაახლეთ სექციები: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM სათაურის პოლიტიკა და ABI ჰეშები (docs/source/ivm_header.md)
  - შეამოწმეთ მხოლოდ: `cargo run -p ivm --bin gen_header_doc -- --check` და `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - განაახლეთ სექციები: `cargo run -p ivm --bin gen_header_doc -- --write` და `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions workflow `.github/workflows/check-docs.yml` აწარმოებს ამ შემოწმებებს ყოველი პუშტის/PR-ის დროს და წარუმატებელი იქნება, თუ გენერირებული დოკუმენტები განხორციელდება.
- [მმართველობის სათამაშო წიგნი] (governance_playbook.md)