---
lang: hy
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Արագացում և Norito Էվրիստիկայի հղում

`[accel]` բլոկը `iroha_config`-ում անցնում է
`crates/irohad/src/main.rs:1895` մեջ `ivm::set_acceleration_config`: Յուրաքանչյուր հյուրընկալող
կիրառում է նույն կոճակները՝ նախքան VM-ի ակնարկավորումը, այնպես որ օպերատորները կարող են որոշիչ կերպով
ընտրեք, թե որ GPU-ի հետնամասերը թույլատրվում են՝ միաժամանակ մատչելի պահելով սկալար/SIMD հետադարձ կապերը:
Swift-ի, Android-ի և Python-ի կապերը բեռնում են նույն մանիֆեստը կամրջի շերտի միջոցով, ուստի
Այս լռելյայն փաստաթղթավորումն ապաշրջափակում է WP6-C-ն ապարատային-արագացման հետաձգման մեջ:

### `accel` (ապարատային արագացում)

Ստորև բերված աղյուսակը արտացոլում է `docs/source/references/peer.template.toml` և
`iroha_config::parameters::user::Acceleration` սահմանում, շրջակա միջավայրի բացահայտում
փոփոխական, որը գերազանցում է յուրաքանչյուր ստեղնը:

| Բանալի | Env var | Կանխադրված | Նկարագրություն |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | Միացնում է SIMD/NEON/AVX կատարումը: Երբ `false`, VM-ն ստիպում է վեկտորային օպերացիաների և Merkle-ի հեշինգի սկալյար գծերը՝ հեշտացնելու դետերմինիստական ​​հավասարության ֆիքսումը: |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | Միացնում է CUDA backend-ը, երբ այն կազմվում է և գործարկման ժամանակը անցնում է ոսկե վեկտորի բոլոր ստուգումները: |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | Միացնում է Metal backend-ը macOS-ի կառուցվածքներում: Նույնիսկ եթե ճշմարիտ է, Metal-ի ինքնաստուգումը դեռ կարող է անջատել հետին մասը գործարկման ժամանակ, եթե հավասարության անհամապատասխանություններ առաջանան: |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (ավտոմատ) | Գլխարկներ, թե քանի ֆիզիկական GPU է սկզբնավորվում գործարկման ժամանակը: `0` նշանակում է «համապատասխանող սարքաշարի օդափոխիչ» և սեղմված է `GpuManager`-ով: |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Նախքան Merkle-ի տերևների հեշինգը GPU-ին բեռնաթափելը, պահանջվում է նվազագույն տերևներ: Այս շեմից ցածր արժեքները շարունակում են հաշվել պրոցեսորի վրա՝ PCIe-ի գերավճարներից խուսափելու համար (`crates/ivm/src/byte_merkle_tree.rs:49`): |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (ժառանգել գլոբալ) | GPU-ի շեմի համար հատուկ մետաղի վերացում: Երբ `0`, մետաղը ժառանգում է `merkle_min_leaves_gpu`: |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (ժառանգել գլոբալ) | CUDA-ի հատուկ անտեսում GPU-ի շեմի համար: Երբ `0`, CUDA-ն ժառանգում է `merkle_min_leaves_gpu`: |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32768 ներքին) | Գլխարկներով ծառի չափը, որտեղ ARMv8 SHA-2 հրահանգները պետք է հաղթեն GPU-ի հեշինգին: `0`-ը պահպանում է `32_768` տերևների (`crates/ivm/src/byte_merkle_tree.rs:59`) կազմված լռելյայնությունը: |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (32768 ներքին) | Նույնը, ինչ վերևում, բայց x86/x86_64 հոսթինգների համար՝ օգտագործելով SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`): |

`enable_simd`-ը նաև վերահսկում է RS16 ջնջման կոդավորումը (Torii DA ingest + գործիքավորում): Անջատեք այն
ստիպել սկալյար հավասարության գեներացում՝ միաժամանակ ելքերը դետերմինիստական պահելով սարքաշարում:

Օրինակ կազմաձևում.

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

Վերջին հինգ ստեղների համար զրոյական արժեքները նշանակում են «պահել կազմված լռելյայն»: Տանտերերը չպետք է
սահմանել հակասական անտեսումներ (օրինակ՝ անջատել CUDA-ն՝ միաժամանակ պարտադրելով միայն CUDA-ի շեմերը),
հակառակ դեպքում խնդրանքը անտեսվում է, և backend-ը շարունակում է հետևել համաշխարհային քաղաքականությանը:

### Գործարկման ժամանակի վիճակի ստուգումԳործարկեք `cargo xtask acceleration-state [--format table|json]`՝ կիրառվածը լուսանկարելու համար
կոնֆիգուրացիա Metal/CUDA գործարկման ժամանակի առողջության բիթերի կողքին: Հրամանը ձգում է
ընթացիկ `ivm::acceleration_config`, հավասարության կարգավիճակ և կպչուն սխալի տողեր (եթե ա
backend-ը անջատված էր), այնպես որ գործողությունները կարող են ուղղակիորեն ներդնել արդյունքը հավասարության մեջ
վահանակներ կամ միջադեպերի ակնարկներ:

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

Օգտագործեք `--format json`, երբ լուսանկարը պետք է ընդունվի ավտոմատացման միջոցով (JSON
պարունակում է նույն դաշտերը, որոնք ներկայացված են աղյուսակում):

`acceleration_runtime_errors()`-ն այժմ պարզում է, թե ինչու SIMD-ն վերադարձավ սկալարի.
`disabled by config`, `forced scalar override`, `simd unsupported on hardware` կամ
`simd unavailable at runtime`, երբ հայտնաբերումը հաջողվում է, բայց կատարումը դեռ աշխատում է
առանց վեկտորների. Չեղարկումը ջնջելը կամ կանոնը նորից միացնելը հաղորդագրությունը կթողնի
SIMD-ին աջակցող հոսթերների վրա:

### Պարիտետի ստուգումներ

Շրջեք `AccelerationConfig`-ը միայն պրոցեսորի և արագացման միջև՝ դետերմինիստական արդյունքներն ապացուցելու համար:
`poseidon_instructions_match_across_acceleration_configs` ռեգրեսիան աշխատում է
Poseidon2/6 օպերացիոն կոդերը երկու անգամ՝ սկզբում `enable_cuda`/`enable_metal`-ով սահմանված է `false`, այնուհետև
երկուսն էլ միացված են, և հաստատում է նույնական ելքերը, գումարած CUDA հավասարությունը, երբ առկա են GPU:【crates/ivm/tests/crypto.rs:100】
Նկարագրեք `acceleration_runtime_status()`-ը վազքի կողքին՝ ձայնագրելու, թե արդյոք հետնամասերը
կազմաձևված/հասանելի են լաբորատոր մատյաններում:

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

### GPU կանխադրված և էվրիստիկա

`MerkleTree` GPU-ի բեռնաթափումը սկսվում է լռելյայնորեն `8192`-ից, իսկ CPU SHA-2-ը
Նախապատվության շեմերը մնում են `32_768` տերևների վրա՝ ըստ ճարտարապետության: Երբ ոչ CUDA-ն, ոչ էլ
Մետաղը հասանելի է կամ անջատվել է առողջական ստուգումների արդյունքում, VM-ն ավտոմատ կերպով ընկնում է
վերադառնալ SIMD/scalar hashing-ին և վերը նշված թվերը չեն ազդում դետերմինիզմի վրա:

`max_gpus`-ը սեղմում է լողավազանի չափը, որը սնվում է `GpuManager`-ում: Միացված է `max_gpus = 1`
Multi-GPU հոսթերը պարզեցնում է հեռաչափությունը՝ միաժամանակ թույլ տալով արագացում: Օպերատորները կարող են
օգտագործեք այս անջատիչը FASTPQ կամ CUDA Poseidon աշխատանքների համար մնացած սարքերը վերապահելու համար:

### Հաջորդ արագացման թիրախները և բյուջեները

Վերջին FastPQ մետաղական հետքը (`fastpq_metal_bench_20k_latest.json`, 32K տող × 16
սյունակներ, 5 հատ) ցույց է տալիս Poseidon սյունակի հեշինգը, որը գերակշռում է ZK աշխատանքային ծանրաբեռնվածությանը.

- `poseidon_hash_columns`. CPU միջին **3.64s** ընդդեմ GPU միջին **3.55s** (1.03×):
- `lde`. CPU միջին **1,75s** ընդդեմ GPU միջին **1,57s** (1,12×):

IVM/Crypto-ն թիրախավորելու է այս երկու միջուկները հաջորդ արագացման մաքրման ժամանակ: Ելակետային բյուջեներ.

- Պահպանեք սկալար/SIMD հավասարությունը CPU-ի վերևում կամ դրանից ցածր և գրավեք
  `acceleration_runtime_status()` յուրաքանչյուր վազքի կողքին, այնպես որ Metal/CUDA հասանելիությունը լինի
  գրանցված բյուջեի համարներով:
- Թիրախային ≥1,3× արագացում `poseidon_hash_columns`-ի և ≥1,2× `lde`-ի համար՝ մեկ անգամ լարված մետաղի համար
  և CUDA միջուկները վայրէջք են կատարում՝ առանց ելքերի կամ հեռաչափական պիտակների փոփոխության:

Կցեք JSON հետքը և `cargo xtask acceleration-state --format json` նկարը
Ապագա լաբորատորիան աշխատում է այնպես, որ CI/regressions-ը կարողանա հաստատել ինչպես բյուջեն, այնպես էլ հետին պլանի առողջությունը
համեմատելով միայն պրոցեսորով և արագացման գործարկումներով:

### Norito էվրիստիկա (կազմման ժամանակի լռելյայն)Norito-ի դասավորությունը և սեղմման էվրիստիկաները գործում են `crates/norito/src/core/heuristics.rs`-ում
և կազմվում են յուրաքանչյուր երկուականի մեջ: Դրանք կարգավորելի չեն գործարկման ժամանակ, այլ բացահայտվում են
մուտքերն օգնում են SDK-ին և օպերատորների թիմերին կանխատեսել, թե ինչպես կվարվի Norito-ը GPU-ից հետո
սեղմման միջուկները միացված են:
Աշխատանքային տարածքն այժմ կառուցում է Norito՝ լռելյայն միացված `gpu-compression` հատկանիշով,
այնպես որ GPU zstd backend-ները կազմվում են. գործարկման ժամանակի առկայությունը դեռևս կախված է սարքաշարից,
օգնական գրադարանը (`libgpuzstd_*`/`gpuzstd_cuda.dll`) և `allow_gpu_compression`
կազմաձևման դրոշ: Կառուցեք մետաղական օգնականը `cargo build -p gpuzstd_metal --release`-ով և
տեղադրեք `libgpuzstd_metal.dylib` բեռնիչի ուղու վրա: Ներկայիս Metal helper-ն աշխատում է GPU-ով
համընկնումների որոնում/հաջորդականության ստեղծում և օգտագործում է ներկառուցված դետերմինիստական zstd շրջանակը
կոդավորիչ (Huffman/FSE + շրջանակի հավաքում) հյուրընկալողի վրա; ապակոդավորումը օգտագործում է ներդիր շրջանակը
ապակոդավորիչ՝ CPU zstd հետադարձ կապով չաջակցվող շրջանակների համար, մինչև GPU-ի բլոկի ապակոդավորումը միացված լինի:

| Դաշտային | Կանխադրված | Նպատակը |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` բայթ | Սրանից ներքև, օգտակար բեռներն ամբողջությամբ բաց են թողնում zstd-ը՝ գերավճարներից խուսափելու համար: |
| `min_compress_bytes_gpu` | `1_048_576` բայթ (1ՄիԲ) | Այս սահմանաչափով կամ ավելի բարձր բեռները անցնում են GPU zstd-ի, երբ `norito::core::hw::has_gpu_compression()` ճիշտ է: |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | CPU-ի սեղմման մակարդակները համապատասխանաբար <32KiB և ≥32KiB օգտակար բեռների համար: |
| `zstd_level_gpu` | `1` | Պահպանողական GPU մակարդակ՝ հրամանների հերթերը լրացնելիս հետաձգումը հետևողական պահելու համար: |
| `large_threshold` | `32_768` բայթ | Չափի սահմանը «փոքր» և «մեծ» CPU zstd մակարդակների միջև: |
| `aos_ncb_small_n` | `64` տողեր | Այս տողերի քանակի ներքևում հարմարվողական կոդավորիչներն ուսումնասիրում են ինչպես AoS, այնպես էլ NCB դասավորությունները՝ ընտրելու ամենափոքր բեռնվածությունը: |
| `combo_no_delta_small_n_if_empty` | `2` տողեր | Կանխում է u32/id դելտա կոդավորումների միացումը, երբ 1–2 տողերը պարունակում են դատարկ բջիջներ: |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Դելտաները ներխուժում են միայն մեկ անգամ, որտեղ կա առնվազն երկու շարք: |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Բոլոր դելտա փոխակերպումները լռելյայն միացված են լավ վարքագծի մուտքագրման համար: |
| `combo_enable_name_dict` | `true` | Թույլ է տալիս մեկ սյունակի բառարաններ, երբ հարվածների հարաբերակցությունը հիմնավորում է հիշողության գերբեռնվածությունը: |
| `combo_dict_ratio_max` | `0.40` | Անջատել բառարանները, երբ տողերի 40%-ից ավելին տարբեր են: |
| `combo_dict_avg_len_min` | `8.0` | Պահանջել միջին տողի երկարությունը ≥8 նախքան բառարաններ կառուցելը (կարճ մականունները մնում են ներդիրում): |
| `combo_dict_max_entries` | `1024` | Հիշողության սահմանափակ օգտագործումը երաշխավորելու համար բառարանի գրառումների կոշտ գլխարկ: |

Այս էվրիստիկաները պահում են GPU-ով միացված հոստերերը համահունչ միայն պրոցեսորով աշխատող գործընկերների հետ՝ ընտրիչը
երբեք որոշում չի կայացնում, որը կփոխի մետաղալարերի ձևաչափը, և շեմերը ֆիքսված են
մեկ թողարկման համար: Երբ պրոֆիլավորումը բացահայտում է ավելի լավ անկման կետեր, Norito-ը թարմացնում է
կանոնական `Heuristics::canonical` իրականացում և `docs/source/benchmarks.md` plus
`status.md` արձանագրել փոփոխությունը տարբերակված ապացույցների հետ մեկտեղ:GPU zstd օգնականը պարտադրում է նույն `min_compress_bytes_gpu` անջատումը, նույնիսկ երբ
ուղղակիորեն կանչված (օրինակ՝ `norito::core::gpu_zstd::encode_all`-ի միջոցով), այնքան փոքր
օգտակար բեռները միշտ մնում են պրոցեսորի ուղու վրա՝ անկախ GPU-ի առկայությունից:

### Անսարքությունների վերացում և հավասարության ստուգաթերթ

- Snapshot-ի գործարկման ժամանակի վիճակը `cargo xtask acceleration-state --format json`-ով և պահպանեք
  ելքը ցանկացած ձախողված տեղեկամատյանների հետ մեկտեղ; հաշվետվությունը ցույց է տալիս կազմաձևված/մատչելի հետնամասեր
  գումարած պարիտետ/վերջին սխալ տողեր:
- Վերագործարկեք արագացման հավասարության ռեգրեսիան տեղական մակարդակում՝ շեղումը բացառելու համար.
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (աշխատում է միայն պրոցեսորով, այնուհետև արագացնում է): Գրանցեք `acceleration_runtime_status()` վազքի համար:
- Եթե backend-ը ձախողում է ինքնափորձարկումը, հանգույցը պահեք առցանց միայն պրոցեսորի ռեժիմում (`enable_metal =
  false`, `enable_cuda = false`) և բացեք միջադեպ՝ գրավված հավասարության ելքով
  փոխարենը պարտադրելու backend-ը: Արդյունքները պետք է որոշիչ մնան բոլոր ռեժիմներում:
- **CUDA հավասարաչափ ծուխ (լաբորատոր NV սարքավորում):** Գործարկել
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x սարքաշարի վրա, նկարեք `cargo xtask acceleration-state --format json` և կցեք
  կարգավիճակի պատկերը (GPU մոդելը/դրայվերը ներառված է) չափանիշի արտեֆակտներին: