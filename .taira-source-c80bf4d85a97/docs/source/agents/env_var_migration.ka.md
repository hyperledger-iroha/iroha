---
lang: ka
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Config Migration Tracker

ეს ტრეკერი აჯამებს წარმოების წინაშე მყოფი გარემოს ცვლადი გადამრთველებს
`docs/source/agents/env_var_inventory.{json,md}`-ით და განზრახ მიგრაცია
გზა `iroha_config`-ში (ან მკაფიო დეველ/მხოლოდ ტესტის სკოპინგი).


შენიშვნა: `ci/check_env_config_surface.sh` ახლა მარცხდება, როდესაც ახალი **წარმოება** env
შიმები გამოჩნდება `AGENTS_BASE_REF`-თან შედარებით, თუ `ENV_CONFIG_GUARD_ALLOW=1` არ არის
კომპლექტი; დააფიქსირეთ მიზანმიმართული დამატებები აქ, სანამ გამოიყენებდეთ უგულებელყოფას.

## დასრულებული მიგრაცია- **IVM ABI უარის თქმა** — ამოღებულია `IVM_ALLOW_NON_V1_ABI`; შემდგენელი ახლა უარყოფს
  არა-v1 ABI-ები უპირობოდ ერთეულის ტესტით, რომელიც იცავს შეცდომის გზას.
- **IVM გამართვის ბანერი env shim** — გაუქმდა `IVM_SUPPRESS_BANNER` env უარის თქმა;
  ბანერის ჩახშობა ხელმისაწვდომი რჩება პროგრამული სეტერის საშუალებით.
- **IVM ქეში/ზომა** — ხრახნიანი ქეში/პროვერი/GPU-ის ზომის განსაზღვრა
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) და ამოიღეს Runtime env shims. მასპინძლები ახლა დარეკავენ
  `ivm::ivm_cache::configure_limits` და `ivm::zk::set_prover_threads`, ტესტების გამოყენება
  `CacheLimitsGuard` env-ის ნაცვლად უგულებელყოფს.
- ** რიგის ფესვის დაკავშირება ** — დამატებულია `connect.queue.root` (ნაგულისხმევი:
  `~/.iroha/connect`) კლიენტის კონფიგურაციაში და გადაიტანეთ იგი CLI-ში და
  JS დიაგნოსტიკა. JS დამხმარეები წყვეტენ კონფიგურაციას (ან აშკარა `rootDir`) და
  მხოლოდ პატივი სცეს `IROHA_CONNECT_QUEUE_ROOT` დეველ/ტესტში `allowEnvOverride`-ის მეშვეობით;
  შაბლონები აფიქსირებენ ღილაკს, რათა ოპერატორებს აღარ დასჭირდეთ env გადაფარვები.
- **Izanami ქსელის ჩართვა** — დამატებულია აშკარა `allow_net` CLI/კონფიგურაციის დროშა
  Izanami ქაოსის ინსტრუმენტი; ახლა გაშვებას მოითხოვს `allow_net=true`/`--allow-net` და
- **IVM ბანერის სიგნალი** — ჩაანაცვლა `IROHA_BEEP` env shim კონფიგურაციით
  `ivm.banner.{show,beep}` გადართავს (ნაგულისხმევი: true/true). გაშვების ბანერი / სიგნალი
  გაყვანილობა ახლა კითხულობს კონფიგურაციას მხოლოდ წარმოებაში; dev/test builds still honor
  env უგულებელყოფა ხელით გადართვისთვის.
- **DA spool override (მხოლოდ ტესტები)** — `IROHA_DA_SPOOL_DIR` გადაფარვა არის ახლა
  შემოღობილი `cfg(test)` დამხმარეების უკან; წარმოების კოდი ყოველთვის იღებს კოჭას
  გზა კონფიგურაციისგან.
- **კრიპტო ინტრინსიკა** — შეიცვალა `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` კონფიგურაციით
  `crypto.sm_intrinsics` პოლიტიკა (`auto`/`force-enable`/`force-disable`) და
  ამოიღო `IROHA_SM_OPENSSL_PREVIEW` მცველი. მასპინძლები იყენებენ პოლიტიკას მისამართზე
  გაშვება, სკამები/ტესტები შეიძლება აირჩიონ `CRYPTO_SM_INTRINSICS`-ით და OpenSSL-ით
  გადახედვა ახლა პატივს სცემს მხოლოდ კონფიგურაციის დროშას.
  Izanami უკვე მოითხოვს `--allow-net`/გაგრძელებულ კონფიგურაციას და ტესტები ახლა ეყრდნობა
  ეს ღილაკი, ვიდრე ambient env გადართავს.
- **FastPQ GPU tuning** — დამატებულია `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  კონფიგურაციის სახელურები (ნაგულისხმევი: `None`/`None`/`false`/`false`/`false`) და გადაიტანეთ ისინი CLI ანალიზში
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` shims ახლა იქცევა როგორც დეველ/ტესტი სარეზერვო და
  იგნორირებულია კონფიგურაციის ჩატვირთვის შემდეგ (მაშინაც კი, როდესაც კონფიგურაცია ტოვებს მათ დაყენებულს); დოკუმენტები/ინვენტარი იყო
  განახლებულია მიგრაციის დროშისთვის.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) ახლა გამართულია გამართვის/ტესტის კონსტრუქციების უკან გაზიარებული ვერსიით
  დამხმარე, ასე რომ, წარმოების ორობითი ფაილები უგულებელყოფენ მათ, ხოლო კლავიშების შენარჩუნებას ადგილობრივი დიაგნოსტიკისთვის. ენვ
  ინვენტარი განახლდა, რათა ასახავდეს მხოლოდ დეველოპმენტის/ტესტის სფეროს.- **FASTPQ მოწყობილობების განახლებები** — `FASTPQ_UPDATE_FIXTURES` ახლა მხოლოდ FASTPQ ინტეგრაციაში ჩანს
  ტესტები; წარმოების წყაროები აღარ კითხულობენ env გადართვას და ინვენტარი ასახავს მხოლოდ ტესტს
  ფარგლები.
- **ინვენტარის განახლება + ფარგლების ამოცნობა** — env ინვენტარის ხელსაწყოები ახლა აწერს `build.rs` ფაილებს, როგორც
  შექმენით ფარგლები და თვალყური ადევნეთ `#[cfg(test)]`/ინტეგრაციის აღკაზმულობის მოდულებს, რათა გადართეთ მხოლოდ ტესტისთვის (მაგ.
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) და CUDA build დროშები გამოჩნდება წარმოების რაოდენობის მიღმა.
  ინვენტარი რეგენერირებულია 2025 წლის 7 დეკემბერს (518 refs / 144 vars), რათა შეინარჩუნოს env-config მცველის განსხვავება მწვანე.
- **P2P ტოპოლოგია env shim გამოშვების მცველი** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` ახლა იწვევს დეტერმინისტულ
  გაშვების შეცდომა გამოშვების ნაგებობებში (მხოლოდ გაფრთხილება გამართვის/ტესტის დროს), ამიტომ წარმოების კვანძები მხოლოდ ეყრდნობიან
  `network.peer_gossip_period_ms`. env ინვენტარი რეგენერირებული იყო მცველისა და ასახვის მიზნით
  განახლებული კლასიფიკატორი ახლა ახორციელებს `cfg!`-დაცვით გადართვას, როგორც გამართვა/ტესტი.

## მაღალი პრიორიტეტული მიგრაცია (წარმოების გზები)

- _ არცერთი (ინვენტარი განახლებულია cfg!/გამართვის გამოვლენით; env-config დამცავი მწვანე P2P შიმის გამკვრივების შემდეგ)._

## Dev/მხოლოდ ტესტის გადართვა ღობეზე

- მიმდინარე წმენდა (2025 წლის 07 დეკემბერი): მხოლოდ CUDA დროშები (`IVM_CUDA_*`) არის `build` და
  აღკაზმულობის გადამრთველები (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) ახლა დარეგისტრირდით როგორც
  `test`/`debug` ინვენტარში (მათ შორის, `cfg!`-დაცული შიმები). დამატებითი ღობე არ არის საჭირო;
  შეინახეთ მომავალი დამატებები `cfg(test)`/მხოლოდ სკამზე დამხმარეები TODO მარკერებით, როცა შიმბები დროებითია.

## აშენების დრო (დატოვეთ როგორც არის)

- ტვირთის/ფუნქციის ველები (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  რჩება `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD` და ა.შ.
  build-script შეშფოთებულია და საზღვრებს გარეთაა გაშვების კონფიგურაციის მიგრაციისთვის.

## შემდეგი მოქმედებები

1) გაუშვით `make check-env-config-surface` კონფიგურაციის ზედაპირის განახლებების შემდეგ, რათა დაიჭიროთ ახალი წარმოების env shims
   ადრეული და მიანიჭეთ ქვესისტემის მფლობელები/ETA-ები.  
2) განაახლეთ ინვენტარი (`make check-env-config-surface`) ყოველი გაწმენდის შემდეგ.
   ტრეკერი რჩება ახალ დამცავ რელსებთან შესაბამისობაში და env-config დამცავი განსხვავება რჩება ხმაურის გარეშე.