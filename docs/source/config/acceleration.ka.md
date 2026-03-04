---
lang: ka
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## აჩქარება და Norito ევრისტიკის მითითება

`[accel]` ბლოკი `iroha_config`-ში გადადის
`crates/irohad/src/main.rs:1895` შევიდა `ivm::set_acceleration_config`. ყოველი მასპინძელი
იყენებს იგივე ღილაკებს VM-ის ინსტალაციამდე, ასე რომ ოპერატორებს შეუძლიათ განსაზღვრონ
აირჩიე რომელი GPU backends არის დაშვებული სკალარული/SIMD სარეზერვო საშუალებების ხელმისაწვდომობის დროს.
Swift-ის, Android-ისა და Python-ის საკინძები იტვირთება იგივე მანიფესტს ხიდის ფენის მეშვეობით, ასე რომ
ამ ნაგულისხმევის დოკუმენტირება განბლოკავს WP6-C-ს ტექნიკის აჩქარების ჩამორჩენაში.

### `accel` (ტექნიკის აჩქარება)

ქვემოთ მოყვანილი ცხრილი ასახავს `docs/source/references/peer.template.toml` და
`iroha_config::parameters::user::Acceleration` განმარტება, გარემოს გამოვლენა
ცვლადი, რომელიც აჭარბებს თითოეულ კლავიშს.

| გასაღები | Env var | ნაგულისხმევი | აღწერა |
|-----|---------|--------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | რთავს SIMD/NEON/AVX შესრულებას. როდესაც `false`, VM აიძულებს ვექტორული ოპერაციების და Merkle-ის ჰეშინგს სკალარულ ბექენდებს დეტერმინისტული პარიტეტის აღების გასაადვილებლად. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | რთავს CUDA backend-ს, როდესაც ის კომპილირებულია და გაშვების დრო გადის ყველა ოქროს ვექტორის შემოწმებას. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | ჩართავს Metal backend-ს macOS კონსტრუქციებზე. მაშინაც კი, როცა სიმართლეა, Metal-ის თვითტესტს მაინც შეუძლია გამორთოს backend გაშვების დროს, თუ პარიტეტის შეუსაბამობა მოხდება. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (ავტო) | იკავებს რამდენი ფიზიკური GPU-ს ინიციალიზაციას გაშვების დრო. `0` ნიშნავს „შესატყვისი ტექნიკის ვენტილაციას“ და დამაგრებულია `GpuManager`-ით. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | მინიმალური ფოთლები საჭიროა Merkle-ის ფოთლის ჰეშინგამდე GPU-ში ჩატვირთვამდე. ამ ზღურბლზე დაბალი მნიშვნელობები განაგრძობს ჰეშირებას CPU-ზე, რათა თავიდან აიცილოს PCIe ზედნადები (`crates/ivm/src/byte_merkle_tree.rs:49`). |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (მემკვიდრეობით გლობალური) | ლითონის სპეციფიკური გადაფარვა GPU ზღვრისთვის. როდესაც `0`, მეტალი მემკვიდრეობით იღებს `merkle_min_leaves_gpu`. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (მემკვიდრეობით გლობალური) | CUDA-ს სპეციფიკური გადაფარვა GPU ზღვრისთვის. როდესაც `0`, CUDA მემკვიდრეობით იღებს `merkle_min_leaves_gpu`. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32768 შიდა) | იკავებს ხის ზომას, სადაც ARMv8 SHA-2 ინსტრუქციებმა უნდა გაიმარჯვოს GPU ჰეშინგზე. `0` ინახავს `32_768` ფურცლების (`crates/ivm/src/byte_merkle_tree.rs:59`) შედგენილ ნაგულისხმევ ნაგულისხმევს. |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (32768 შიდა) | იგივე, რაც ზემოთ, მაგრამ x86/x86_64 ჰოსტებისთვის SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) გამოყენებით. |

`enable_simd` ასევე აკონტროლებს RS16 წაშლის კოდირებას (Torii DA ingest + tooling). გამორთეთ იგი
აიძულეთ სკალარული პარიტეტის გენერირება, ხოლო გამოსავლები დეტერმინისტული შეინარჩუნეთ აპარატურაში.

კონფიგურაციის მაგალითი:

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

ბოლო ხუთი კლავიშის ნულოვანი მნიშვნელობები ნიშნავს „შეინარჩუნე შედგენილი ნაგულისხმევი“. მასპინძლები არ უნდა
დააყენეთ კონფლიქტური უგულებელყოფა (მაგ. CUDA-ს გამორთვა მხოლოდ CUDA-ს ზღვრების იძულებით),
წინააღმდეგ შემთხვევაში მოთხოვნა იგნორირებულია და backend აგრძელებს გლობალურ პოლიტიკას.

### გაშვების მდგომარეობის შემოწმებაგაუშვით `cargo xtask acceleration-state [--format table|json]`, რომ გადაიღოთ გამოყენებული
კონფიგურაცია Metal/CUDA გაშვების ჯანმრთელობის ბიტებთან ერთად. ბრძანება გაიყვანს
მიმდინარე `ivm::acceleration_config`, პარიტეტის სტატუსი და წებოვანი შეცდომის სტრიქონები (თუ
backend იყო გამორთული), ასე რომ, ოპერაციებმა შეიძლება შეასრულოს შედეგი პირდაპირ პარიტეტში
დაფები ან ინციდენტების მიმოხილვა.

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

გამოიყენეთ `--format json`, როდესაც სნეპშოტი უნდა იქნას მიღებული ავტომატიზაციის საშუალებით (JSON
შეიცავს იმავე ველებს, რომლებიც ნაჩვენებია ცხრილში).

`acceleration_runtime_errors()` ახლა ამტკიცებს, რატომ დაბრუნდა SIMD სკალარზე:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, ან
`simd unavailable at runtime`, როდესაც აღმოჩენა წარმატებულია, მაგრამ შესრულება კვლავ მუშაობს
ვექტორების გარეშე. უგულებელყოფის გასუფთავება ან პოლიტიკის ხელახლა ჩართვა შეტყობინებას ტოვებს
ჰოსტებზე, რომლებიც მხარს უჭერენ SIMD-ს.

### პარიტეტული ჩეკები

გადაატრიალეთ `AccelerationConfig` მხოლოდ CPU-სა და Accel-ს შორის, რათა დაამტკიცოთ დეტერმინისტული შედეგები.
`poseidon_instructions_match_across_acceleration_configs` რეგრესია მუშაობს
Poseidon2/6 ორჯერ ახდენს ოპკოდებს — ჯერ `enable_cuda`/`enable_metal` დაყენებულია `false`-ზე, შემდეგ
ორივე ჩართულია და ამტკიცებს იდენტურ გამომავალს, პლუს CUDA პარიტეტს GPU-ების არსებობისას.【crates/ivm/tests/crypto.rs:100】
გადაიღეთ `acceleration_runtime_status()` გაშვების პარალელურად, რათა ჩაწეროთ არის თუ არა backends
იყო კონფიგურირებული/ხელმისაწვდომი ლაბორატორიის ჟურნალებში.

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU ნაგულისხმევი და ევრისტიკა

`MerkleTree` GPU გადმოტვირთვა იწყება `8192`-ზე ნაგულისხმევად და CPU SHA-2
უპირატესობის ზღურბლები რჩება `32_768` ფურცლებზე თითო არქიტექტურაზე. როცა არც CUDA და არც
ლითონი ხელმისაწვდომია ან გამორთულია ჯანმრთელობის შემოწმების შედეგად, VM ავტომატურად ეცემა
დაუბრუნდით SIMD/სკალარ ჰეშინგს და ზემოაღნიშნული რიცხვები არ იმოქმედებს დეტერმინიზმზე.

`max_gpus` ამაგრებს აუზის ზომას, რომელიც მიეწოდება `GpuManager`. `max_gpus = 1` ჩართულია
მრავალ GPU მასპინძლები ინარჩუნებს ტელემეტრიას მარტივს, ხოლო აჩქარების საშუალებას იძლევა. ოპერატორებს შეუძლიათ
გამოიყენეთ ეს გადამრთველი, რათა დაჯავშნოთ დარჩენილი მოწყობილობები FASTPQ ან CUDA Poseidon სამუშაოებისთვის.

### შემდეგი აჩქარების მიზნები და ბიუჯეტი

უახლესი FastPQ ლითონის კვალი (`fastpq_metal_bench_20k_latest.json`, 32K მწკრივი × 16
სვეტები, 5 იტერი) აჩვენებს Poseidon-ის სვეტის ჰეშინგს, რომელიც დომინირებს ZK დატვირთვაზე:

- `poseidon_hash_columns`: CPU საშუალო **3.64s** GPU საშუალო **3.55s** (1.03×).
- `lde`: CPU საშუალო **1.75s** GPU საშუალო **1.57s** (1.12×).

IVM/Crypto მიზანმიმართული იქნება ამ ორ ბირთვს შემდეგი accel-ის გაწმენდისას. საბაზისო ბიუჯეტები:

- შეინახეთ სკალარული/SIMD პარიტეტი CPU-ის ზემოთ ან მის ქვემოთ და დაჭერით
  `acceleration_runtime_status()` თითოეულ გაშვებასთან ერთად, ამიტომ Metal/CUDA ხელმისაწვდომობაა
  რეგისტრირებულია ბიუჯეტის ნომრებით.
- სამიზნე ≥1.3× სიჩქარე `poseidon_hash_columns`-ისთვის და ≥1.2× `lde` ერთხელ დარეგულირებული ლითონისთვის
  და CUDA ბირთვები მიწაზე, შედეგების ან ტელემეტრიული ეტიკეტების შეცვლის გარეშე.

მიამაგრეთ JSON კვალი და `cargo xtask acceleration-state --format json` სნეპშოტი
მომავალი ლაბორატორია მუშაობს ისე, რომ CI/რეგრესიას შეუძლია დაამტკიცოს როგორც ბიუჯეტები, ასევე სარეზერვო ჯანმრთელობა, ხოლო
შედარება მხოლოდ CPU და accel-on გაშვებები.

### Norito ევრისტიკა (კომპილაციის დროის ნაგულისხმევი ნაგულისხმევი)Norito-ის განლაგება და შეკუმშვის ევრისტიკა მუშაობს `crates/norito/src/core/heuristics.rs`-ში
და შედგენილია ყველა ბინარში. ისინი არ არის კონფიგურირებადი გაშვების დროს, მაგრამ გამოვლენილია
შეყვანები ეხმარება SDK-ს და ოპერატორის გუნდებს წინასწარ განსაზღვრონ, თუ როგორ მოიქცევა Norito GPU-ს ერთხელ
შეკუმშვის ბირთვები ჩართულია.
სამუშაო სივრცე ახლა აშენებს Norito-ს ნაგულისხმევად ჩართული `gpu-compression` ფუნქციით,
ასე რომ, GPU zstd backends შედგენილია; გაშვების ხელმისაწვდომობა კვლავ დამოკიდებულია აპარატურაზე,
დამხმარე ბიბლიოთეკა (`libgpuzstd_*`/`gpuzstd_cuda.dll`) და `allow_gpu_compression`
კონფიგურაციის დროშა. შექმენით ლითონის დამხმარე `cargo build -p gpuzstd_metal --release` და
მოათავსეთ `libgpuzstd_metal.dylib` ჩამტვირთველის გზაზე. ამჟამინდელი Metal Helper მართავს GPU-ს
შესატყვისის პოვნა/მიმდევრობის გენერირება და იყენებს in-crate deterministic zstd ჩარჩოს
ენკოდერი (Huffman/FSE + ჩარჩოს შეკრება) ჰოსტზე; დეკოდი იყენებს უჯრის ჩარჩოს
დეკოდერი CPU zstd სარეზერვო ფრეიმებისთვის, სანამ GPU ბლოკის დეკოდი არ იქნება ჩართული.

| ველი | ნაგულისხმევი | დანიშნულება |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` ბაიტი | ამის ქვემოთ, ტვირთამწეობა მთლიანად გამოტოვებს zstd-ს ზედნადების თავიდან ასაცილებლად. |
| `min_compress_bytes_gpu` | `1_048_576` ბაიტი (1MiB) | ტვირთამწეობა ამ ლიმიტზე ან ზემოთ გადადის GPU zstd-ზე, როდესაც `norito::core::hw::has_gpu_compression()` არის true. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | CPU-ის შეკუმშვის დონეები <32 KiB და ≥32 KiB დატვირთვისთვის შესაბამისად. |
| `zstd_level_gpu` | `1` | კონსერვატიული GPU დონე ბრძანების რიგების შევსებისას ლატენტურობის შესანარჩუნებლად. |
| `large_threshold` | `32_768` ბაიტი | ზომის საზღვარი "პატარა" და "დიდი" CPU zstd დონეებს შორის. |
| `aos_ncb_small_n` | `64` რიგები | ამ მწკრივის დათვლის ქვემოთ ადაპტური ენკოდერები იკვლევენ როგორც AoS, ასევე NCB განლაგებას, რათა აირჩიონ ყველაზე მცირე დატვირთვა. |
| `combo_no_delta_small_n_if_empty` | `2` რიგები | ხელს უშლის u32/id დელტა კოდირების ჩართვას, როდესაც 1–2 მწკრივი შეიცავს ცარიელ უჯრედებს. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | დელტაები მხოლოდ ერთხელ იჭრება, სულ მცირე, ორი რიგი. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | ყველა დელტა ტრანსფორმაცია ჩართულია ნაგულისხმევად კარგად მოვლილი შეყვანებისთვის. |
| `combo_enable_name_dict` | `true` | ნებას რთავს თითო სვეტის ლექსიკონებს, როდესაც დარტყმის კოეფიციენტები ამართლებს მეხსიერების ზედმეტად. |
| `combo_dict_ratio_max` | `0.40` | გამორთეთ ლექსიკონები, როდესაც მწკრივების 40%-ზე მეტი განსხვავებულია. |
| `combo_dict_avg_len_min` | `8.0` | ლექსიკონების შექმნამდე მოითხოვეთ სტრიქონის საშუალო სიგრძე ≥8 (მოკლე მეტსახელები რჩება ხაზში). |
| `combo_dict_max_entries` | `1024` | ლექსიკონის ჩანაწერებში მყარი ქუდი მეხსიერების შეზღუდული გამოყენების გარანტირებისთვის. |

ეს ევრისტიკა ინარჩუნებს GPU-ზე ჩართული ჰოსტებს მხოლოდ CPU-ის თანატოლებთან: სელექტორი
არასოდეს იღებს გადაწყვეტილებას, რომელიც შეცვლის მავთულის ფორმატს და ზღვრები ფიქსირდება
თითო გამოშვებაზე. როდესაც პროფილირებით გამოვლენილია უკეთესი წყვეტის წერტილები, Norito განაახლებს
კანონიკური `Heuristics::canonical` განხორციელება და `docs/source/benchmarks.md` plus
`status.md` ჩაწერეთ ცვლილება ვერსიულ მტკიცებულებასთან ერთად.GPU zstd დამხმარე ახორციელებს იგივე `min_compress_bytes_gpu` შეწყვეტას მაშინაც კი, როდესაც
პირდაპირ დარეკვა (მაგალითად, `norito::core::gpu_zstd::encode_all`-ის საშუალებით), ასე მცირე
ტვირთამწეობა ყოველთვის რჩება CPU გზაზე, GPU ხელმისაწვდომობის მიუხედავად.

### პრობლემების აღმოფხვრა და პარიტეტის საკონტროლო სია

- Snapshot გაშვების მდგომარეობა `cargo xtask acceleration-state --format json`-ით და შეინახეთ
  გამომავალი ჩავარდნილ ჟურნალებთან ერთად; ანგარიშში ნაჩვენებია კონფიგურირებული/ხელმისაწვდომი ბექენდები
  პლუს პარიტეტი/ბოლო შეცდომის სტრიქონები.
- ხელახლა გაუშვით დაჩქარების პარიტეტის რეგრესია ადგილობრივად, რათა გამორიცხოთ დრიფტი:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (მუშაობს მხოლოდ CPU-ზე და შემდეგ აჩქარებს). ჩაწერეთ `acceleration_runtime_status()` გაშვებისთვის.
- თუ ბექენდი ვერ ახერხებს თვითტესტირებას, შეინახეთ კვანძი ონლაინ რეჟიმში მხოლოდ CPU-ის რეჟიმში (`enable_metal =
  false`, `enable_cuda = false`) და გახსენით ინციდენტი დაფიქსირებული პარიტეტის გამომავალით
  ნაცვლად იძულებით backend on. შედეგები უნდა დარჩეს განმსაზღვრელი ყველა რეჟიმში.
- ** CUDA პარიტეტული კვამლი (ლაბორატორია NV აპარატურა):** გაშვება
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x აპარატურაზე, გადაიღეთ `cargo xtask acceleration-state --format json` და მიამაგრეთ
  სტატუსის სნეპშოტი (GPU მოდელი/დრაივერი მოყვება) საორიენტაციო არტეფაქტებს.