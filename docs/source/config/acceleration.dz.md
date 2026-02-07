---
lang: dz
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## མགྱོགས་ཚད་དང་ Norito ཧེ་རི་སི་ཊིག་གཞི་བསྟུན།

`[accel]` བརྒྱུད་དེ་ `iroha_config` ཐགསཔ་ཚུ་ནང་ བརྒྱུད་དེ་
`crates/irohad/src/main.rs:1895` `ivm::set_acceleration_config` ནང་། གཙོ་བདག་
ཝི་ཨེམ་འདི་ བརྡ་བཀོད་མ་འབད་བའི་ཧེ་མ་ ཅོག་འཐདཔ་ཚུ་འཇུག་སྤྱོད་འབདཝ་ལས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ གཏན་འབེབས་བཟོ་ཚུགས།
scalar/SIMD fillacks འཐོབ་ཚུགས་པའི་སྐབས་ GPU རྒྱབ་ཐག་ཚུ་ ག་འདི་ཆོགཔ་ཨིན་ན་ འཐུ་དགོ།
Swift, Android, དང་ Python བཅིངས་པ་ཚུ་ ཟམ་གྱི་བང་རིམ་བརྒྱུད་དེ་ མངོན་གསལ་ཅོག་འཐདཔ་ མངོན་གསལ་འབདཝ་ཨིན།
སྔོན་སྒྲིག་འདི་ཚུ་ཡིག་ཐོག་ལུ་བཀོད་མི་འདི་གིས་ སྲ་ཆས་མགྱོགས་ཚད་རྒྱབ་ལོག་ནང་ལུ་ ཌབ་ལུ་པི་༦-སི་ བཀག་ཆ་འབདཝ་ཨིན།

### `accel` (སྦྲང་རྩི་གི་མགྱོགས་ཚད་)

ཐིག་ཁྲམ་འདི་གིས་ `docs/source/references/peer.template.toml` དང་ དེ་ལས་
`iroha_config::parameters::user::Acceleration` ངེས་ཚིག ཁོར་ཡུག་ཕྱིར་བཏོན་འབད་དོ།
འགྱུར་ལྡོག་ཅན་འདི་གིས་ ལྡེ་མིག་རེ་རེ་ལུ་ ཚབ་བཙུགསཔ་ཨིན།

| ལྡེ་མིག་ | Env var | སྔོན་སྒྲིག་ | འགྲེལ་བཤད་ |
|--|-|-|--------------------------------------------- |
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX ལག་ལེན་འཐབ་བཏུབ་འབདཝ་ཨིན། `false` འབད་བའི་སྐབས་ VM གིས་ vector ops དང་ Merkle hashing གི་དོན་ལུ་ scalar backend དང་ Merkle hasing གིས་ གཏན་འབེབས་བཟོ་མི་ཚུ་གི་ འཛིན་བཟུང་ཚུ་ འཇམ་ཏོང་ཏོ་བཟོཝ་ཨིན། |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | ནང་ན་བསྡུ་སྒྲིག་འབད་བའི་སྐབས་ CUDA རྒྱབ་རྟེན་འདི་ལྕོགས་ཅན་བཟོཝ་ཨིནམ་དང་ རན་ཊའིམ་འདི་ གསེར་གྱི་ཝེག་ཊར་ཞིབ་དཔྱད་ཆ་མཉམ་འགྱོཝ་ཨིན། |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | macOS གུ་ཡོད་པའི་ལྕགས་ཀྱི་རྒྱབ་རྟེན་འདི་ལྕོགས་ཅན་བཟོཝ་ཨིན། བདེན་པ་ཨིན་རུང་ མེ་ཊལ་རང་གིས་བརྟག་དཔྱད་ཚུ་གིས་ མཉམ་མཐུན་མ་མཐུན་མི་ཚུ་འབྱུང་པ་ཅིན་ རན་ཊའིམ་ལུ་ རྒྱབ་རྟེན་འདི་ ལྕོགས་མིན་བཟོ་ཚུགས། |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (auto) | གཡོག་བཀོལ་བའི་དུས་ཚོད་འགོ་བཙུགས་མི་འདི་ དངོས་གཟུགས་ཇི་པི་ཨུ་ག་དེམ་ཅིག་ཚུ་ མགུ་ཏོག་ཨིན། `0` ཟེར་མི་འདི་ “match མཐུན་རྐྱེན་གྱི་རླུང་འཕྲིན་” ཟེར་མི་འདི་ཨིན། |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | མར་ཀལ་གྱིས་ ཇི་པི་ཡུ་ལུ་ ཕབ་ལེན་ཚུ་ ཕྱིར་འཐེན་མ་འབད་བའི་ཧེ་མ་ ཉུང་མཐའི་འདབ་མ་ཚུ་ དགོཔ་ཨིན། འདི་གི་འོག་ལུ་ཡོད་པའི་གནས་གོང་ཚུ་གིས་ པི་སི་ཨའི་ མགོ་ལུ་ བཀག་ཐབས་ལུ་ སི་པི་ཡུ་གུ་ ཧ་ཤིང་སྦེ་བཞགཔ་ཨིན། (`crates/ivm/src/byte_merkle_tree.rs:49`) |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (འཛམ་གླིང་ཡོངས་ཁྱབ་ཀྱི་) | ལྕགས་རིགས་ཀྱི་ ཇི་པི་ཡུ་ཚད་གཞི་གི་དོན་ལུ་ བཀག་ཆ་འབད་ཡོདཔ། `0` སྐབས་ ལྕགས་རིགས་ཀྱིས་ `merkle_min_leaves_gpu` ཤུལ་འཛིན་འབདཝ་ཨིན། |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (འཛམ་གླིང་ཡོངས་ཁྱབ་ཀྱི་) | CUDA-དམིགས་བསལ་གྱི་བཀག་སྡོམ་འདི་ GPU གི་ཚད་གཞི་གི་དོན་ལུ་ཨིན། `0` འབད་བའི་སྐབས་ CUDA གིས་ `merkle_min_leaves_gpu` ཤུལ་འཛིན་འབདཝ་ཨིན། |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | ```
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
``` (32768 ནང་དུ་) | ARMv8 SHA-2 བཀོད་རྒྱ་ཚུ་གིས་ GPU ཧ་ཤིང་ལས་ རྒྱལ་ཁ་ཐོབ་དགོ་པའི་ ཤིང་གི་སྦོམ་ཆུང་འདི་ ཁ་བསྡམས། `0` གིས་ `32_768` གི་ བསྡུ་སྒྲིག་འབད་ཡོད་པའི་ སྔོན་སྒྲིག་འདི་ (`crates/ivm/src/byte_merkle_tree.rs:59`) བཞགཔ་ཨིན། |
| Norito | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | Norito (༣༢༧༦༨ ནང་ན་) | གོང་ལུ་དང་འདྲ་ དེ་འབདཝ་ད་ ཨེསི་ཨེཆ་ཨེ་-ཨེན་ཨའི་ (`crates/ivm/src/byte_merkle_tree.rs:63`) ལག་ལེན་འཐབ་སྟེ་ x86/x86_64 གི་དོན་ལུ་ཨིན། |

`enable_simd` གིས་ RS16 བཏོན་གཏང་མི་ ཀོ་ཌིང་ (Torii DA ingest + ལག་ཆས་ཚུ་) ཡང་ཚད་འཛིན་འབདཝ་ཨིན། ལུ་ལྕོགས་མིན་བཟོ།
འཕྲུལ་ཆས་ཚུ་ནང་ལུ་ ཐོན་འབྲས་ཚུ་ གཏན་འབེབས་བཟོཝ་ཨིན།

དཔེར་ན་ རིམ་སྒྲིག་:

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

མཇུག་གི་ལྡེ་མིག་ལྔ་གི་དོན་ལུ་ ཀླད་ཀོར་གནས་གོང་ཀླད་ཀོར་འདི་གིས་ “ཕྱོགས་སྒྲིག་འབད་ཡོད་མི་སྔོན་སྒྲིག་” ཟེར་བའི་དོན་དག་ཨིན། གཙོ་འཛིན་ཚུ་ མ་དགོཔ།
འགལ་བ་ཚུ་ འགལ་བ་སྦེ་གཞི་སྒྲིག་འབད།(དཔེར་ན་ CUDA-རྐྱངམ་ཅིག་ཚད་གཞི་ཚུ་ བཀག་ཆ་འབད་བའི་སྐབས་ CUDA ལྕོགས་མིན་བཟོ་ནི།)
དེ་མེན་པ་ཅིན་ ཞུ་བ་འདི་ སྣང་མེད་བཞག་སྟེ་ རྒྱབ་ཐག་འདི་ འཛམ་གླིང་སྲིད་བྱུས་ལུ་ གཞི་བཞག་སྟེ་ འཕྲོ་མཐུད་དེ་རང་ འབད་དོ་ཡོདཔ་ཨིན།

### གཡོག་བཀོལ་བའི་གནས་སྟངས།འཇུག་སྤྱོད་འབད་ནི་གི་དོན་ལུ་ `cargo xtask acceleration-state [--format table|json]` གཡོག་བཀོལ།
རིམ་སྒྲིག་འདི་ ལྕགས་/སི་ཡུ་ཌི་ རན་ཊའིམ་གསོ་བའི་བིཊ་ཚུ་གི་སྦོ་ལོགས་ཁར་ཨིན། བརྡ་བཀོད་ཀྱིས་ འདི་འཐེན་ཨིན།
ད་ལྟའི་`ivm::acceleration_config`, ཆ་སྙོམས་གནས་རིམ་དང་ སྦྱར་བའི་འཛོལ་བ་ཡིག་རྒྱུན་ཚུ་ (གལ་སྲིད་ a གལ་ཏེ་ a ཨིན།
རྒྱབ་ཕྱོགས་འདི་ལྕོགས་མིན་བཟོ་ཡོདཔ་ལས་ བཀོལ་སྤྱོད་འདི་གིས་ གྲུབ་འབྲས་འདི་ ཐད་ཀར་དུ་ འདྲ་མཉམ་ལུ་ འཕྲི་ཚུགསཔ་ཨིན།
dashboard ཡང་ན་ བྱུང་རྐྱེན་བསྐྱར་ཞིབ་ཚུ།

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

པར་ཆས་འདི་རང་བཞིན་གྱིས་བཙུགས་དགོཔ་ད་ `--format json` ལག་ལེན་འཐབ། ( JSON
ཤོག་ཁྲམ་ནང་སྟོན་ཡོད་པའི་ས་སྒོ་ཚུ་ཅོག་འཐདཔ་ཡོདཔ་ཨིན།

`acceleration_runtime_errors()` ད་ལྟ་SIMD ནི་ scalar ལུ་ལོག་ཅི་རང་ ལྷོད་ཅི་ག་ཟེར་ འབོད་བཀུག་འབདཝ་ཨིན།
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, ཡང་ན་
`simd unavailable at runtime` བརྟག་དཔྱད་འདི་མཐར་འཁྱོལ་བྱུང་པའི་སྐབས་ལུ་ཨིན་རུང་ ལག་ལེན་འཐབ་ནི་འདི་ད་ལྟོ་ཡང་འགྱོཝ་ཨིན།
vectors མེད། སྲིད་བྱུས་འདི་ལོག་བཙུགས་ནི་དང་ ཡང་ན་ ལོག་བཙུགས་ནི་འདི་གིས་ འཕྲིན་དོན་འདི་བཀོདཔ་ཨིན།
SIMD ལུ་རྒྱབ་སྐྱོར་འབད་མི་ ཧོསིཊི་ཚུ་ལུ་།

### ཆ་དམན་ཞིབ་དཔྱད་ཚུ།

སི་པི་ཡུ་རྐྱངམ་གཅིག་དང་ མགྱོགས་ཚད་ཀྱི་བར་ན་ `AccelerationConfig` གིས་ གཏན་འབེབས་གྲུབ་འབྲས་ཚུ་ བདེན་ཁུངས་བཟང་ནིའི་དོན་ལུ་ ཕིལིཔ་ཨིན།
`poseidon_instructions_match_across_acceleration_configs` རི་གེ་རེ་སི།
Poseidon2/6 གིས་ ཚར་གཉིས་—དང་པ་ `enable_cuda`/```
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
``` དང་གཅིག་ཁར་ `false` ལུ་གཞི་སྒྲིག་འབདཝ་ཨིན།
གཉིས་ཆ་རང་—དང་ འདྲ་མཚུངས་ཀྱི་ཐོན་འབྲས་ཚུ་ དང་ ཇི་པི་ཡུ་ཚུ་ཡོད་པའི་སྐབས་ལུ་ སི་ཡུ་ཌི་ ཆ་སྙོམས་ཚུ་ བདེན་ཁུངས་བཀལཝ་ཨིན།
རྒྱབ་ཐག་ཚུ་ ཐོ་བཀོད་འབད་ནི་ལུ་ རན་དང་གཅིག་ཁར་ `acceleration_runtime_status()` བཟུང་དགོ།
བརྟག་དཔྱད་ཁང་གི་དྲན་ཐོ་ཚུ་ནང་ རིམ་སྒྲིག་/འཐོབ་ཡོདཔ་ཨིན།

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

### ཇི་པི་ཡུ་སྔོན་སྒྲིག་དང་ཧུ་རིཊིག་སི།

`MerkleTree` ཇི་པི་ཡུ་ སྔོན་སྒྲིག་ཐོག་ལས་ `8192` ནང་ ནང་ཡོད་པའི་ Norito སྔོན་སྒྲིག་གིས་ དང་ སི་པི་ཡུ་ ཨེསི་ཨེཆ་ཨེ་-༢ ཨིན།
དགའ་གདམ་གྱི་ཚད་གཞི་ཚུ་ `32_768` བཟོ་བཀོད་རེ་ལུ་ འདབ་མ་ནང་སྡོད་དོ་ཡོདཔ་ཨིན། ག་ཅིའི་སྐབས་ ཀ་ཌ་མེདཔ།
ལྕགས་འདི་འཐོབ་ཚུགསཔ་ཨིན་ ཡང་ན་ གསོ་བའི་ཞིབ་དཔྱད་ཚུ་གིས་ ལྕོགས་མིན་བཟོ་ཡོདཔ་ཨིན་ ཝི་ཨེམ་འདི་ རང་བཞིན་གྱིས་ མར་ཕབ་འགྱོཝ་ཨིན།
ལོག་སྟེ་ SIMD/scalar hashing དང་ གོང་འཁོད་ཨང་གྲངས་ཚུ་གིས་ གཏན་འབེབས་བཟོ་ནིའི་ནང་ གནོད་པ་མེདཔ་ཨིན།

`max_gpus` གིས་ ཆུ་རྫིང་གི་ཚད་འདི་ `GpuManager` ནང་ལུ་ འཕྱོག་ཡོདཔ་ཨིན། IVM གཞི་སྒྲིག་འབད་དོ།
སྣ་མང་ཇི་པི་ཡུ་ ཧོསིཊི་ཚུ་གིས་ བརྒྱུད་འཕྲིན་འདི་ འཇམ་ཏོང་ཏོ་སྦེ་བཞག་དོ་ཡོདཔ་ད་ དེ་བསྒང་ མགྱོགས་ཚད་འདི་ འབད་བཅུག་དོ་ཡོདཔ་ཨིན། བཀོལ་སྤྱོད་པ་ཚུ་འབད་ཚུགས།
FASTPQ ཡང་ན་ CUDA པོ་སི་ཌོན་ལཱ་གཡོག་གི་དོན་ལུ་ ལྷག་ལུས་ཐབས་འཕྲུལ་ཚུ་ གསོག་འཇོག་འབད་ནི་ལུ་ སོར་བསྒྱུར་འདི་ལག་ལེན་འཐབ།

### ཤུལ་མམ་གྱི་མགྱོགས་ཚད་དམིགས་ཚད་དང་འཆར་དངུལ་།

མགྱོགས་མྱུར་གྱི་ལྕགས་རིགས་ཀྱི་རྗེས་ཤུལ་ (`fastpq_metal_bench_20k_latest.json`, 32K གྱལ་རིམ་ × 16
ཀེར་ཐིག་ཚུ་, 5 hires) གིས་ ZK ལཱ་གི་འབོར་ཚད་ཚུ་ དབང་ཤུགས་ཆེཝ་སྦེ་ Poseidon ཀེར་ཐིག་ཧ་ཤིང་སྟོནམ་ཨིན།

- `poseidon_hash_columns`: སི་པི་ཡུ་ སྤྱིར་སྙོམས་ **3.64s** vs. GPU mea **3.55s** (1.03×).
- `lde`: སི་པི་ཡུ་ སྤྱིར་སྙོམས་ **1.75s** vs. GPU སྤྱིར་སྙོམས་ **1.57s** (1.12×).

IVM/Cryto གིས་ ཀར་ནེལ་གཉིས་འདི་ ཤུལ་མའི་མགྱོགས་ཚད་ཕྱགས་བརྡར་ནང་ དམིགས་གཏད་བསྐྱེད་འོང་། གཞི་རྩའི་སྔོན་འགྲོ།

- གོང་འཁོད་ཀྱི་ སི་པི་ཡུ་ལུ་ སི་ཀེ་ལར་/ཨེསི་ཨའི་ཨེམ་ཌི་ འདྲ་མཉམ་བཞག་ནི་དང་ འཛིན་བཟུང་འབད།
  `acceleration_runtime_status()` འདི་ གཡོག་བཀོལ་མི་རེ་རེ་དང་གཅིག་ཁར་ ལྕགས་/CUDA འཐོབ་ཚུགསཔ་ར།
  འཆར་དངུལ་ཨང་གྲངས་ཚུ་དང་གཅིག་ཁར་ ནང་བསྐྱོད་འབད་ཡོདཔ།
- `poseidon_hash_columns` དང་ `lde` གི་དོན་ལུ་ ≥1.2× གི་མགྱོགས་ཚད་འདི་ ལྕགས་རིགས་ལུ་ཚར་གཅིག་ བསྒྱུར་བཅོས་འབད་ཡོདཔ་ཨིན།
  དང་ CUDA ཀར་ནེལ་ས་ཆ་ཚུ་ ཐོན་འབྲས་དང་ ཡང་ན་ བརྡ་འཕྲིན་གྱི་ ཁ་ཡིག་ཚུ་ འགྱུར་བཅོས་མ་འབད་བར་ཡོདཔ་ཨིན།

JSON གི་རྗེས་འདེད་དང་ `cargo xtask acceleration-state --format json` པར་ལེན་ ལུ་མཉམ་སྦྲགས་འབད།
མ་འོངས་པའི་བརྟག་དཔྱད་ཁང་གིས་ CI/regressions གིས་ འཆར་དངུལ་དང་ རྒྱབ་རྟེན་གསོ་བའི་གཉིས་ཆ་ར་ བདེན་ཁུངས་བཀལ་ཚུགས།
སི་པི་ཡུ་-རྐྱངམ་གཅིག་ དང་ མགྱོགས་ཚད་-རན་ཚུ་ག་བསྡུར་འབད་དོ།

### Norito ཧུ་རིསི་ཊིཀསི (བསྡུ་སྒྲིག་འབད་བའི་དུས་ཚོད་སྔོན་སྒྲིག་)།Norito གི་སྒྲིག་བཀོད་དང་ བསྡམ་བཞག་གི་ heuristics `crates/norito/src/core/heuristics.rs` ནང་སྡོད་ཡོད།
དེ་ལས་ གཉིས་ལྡན་ག་རའི་ནང་ བསྡུ་སྒྲིག་འབད་ཡོདཔ་ཨིན། དེ་ཚུ་ རཱན་ཊའིམ་ལུ་རིམ་སྒྲིག་འབད་མི་བཏུབ་ དེ་འབདཝ་ད་ གསལ་སྟོན་འབདཝ་ཨིན།
ཨིན་པུཊི་ཚུ་གིས་ ཨེསི་ཌི་ཀེ་ལུ་ ཕན་ཐོགཔ་ཨིནམ་དང་ བཀོལ་སྤྱོད་སྡེ་ཚན་ཚུ་གིས་ Norito གིས་ ཚར་གཅིག་ GPU ག་དེ་སྦེ་འབད་ནི་ཨིན་ན་ སྔོན་དཔག་འབདཝ་ཨིན།
བསྡམ་བཞག་ཀར་ནེལ་ཚུ་ ལྕོགས་ཅན་བཟོ་ཡོདཔ་ཨིན།
ད་ལྟོ་ལཱ་གི་ས་སྒོ་འདི་གིས་ `gpu-compression` འདི་ སྔོན་སྒྲིག་གིས་ ལྕོགས་ཅན་བཟོ་ཡོད་པའི་ `gpu-compression` དང་ཅིག་ཁར་བཟོ་བསྐྲུན་འབདཝ་ཨིན།
དེ་འབདཝ་ལས་ GPU zstd རྒྱབ་ལོག་ཚུ་ བསྡུ་སྒྲིག་འབད་ཡོདཔ་ཨིན། རན་ཊའིམ་ཐོབ་ཚུལ་འདི་ ད་ལྟོ་ཡང་ མཐུན་རྐྱེན་ལུ་རག་ལསཔ་ཨིན།
རོགས་རམ་དཔེ་མཛོད་ (`libgpuzstd_*`/```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```) དང་ `allow_gpu_compression` དང་།
རིམ་སྒྲིག་རྒྱལ་དར་། ལྕགས་རིགས་རོགས་རམ་པ་ `cargo build -p gpuzstd_metal --release` དང་།
log loading `libgpuzstd_metal.dylib` མངོན་གསལ་བྱེད་པའི་འགྲུལ་ལམ་སྟེང་། ད་ལྟོའི་མེ་ཊལ་ རོགས་རམ་པ་གིས་ ཇི་པི་ཡུ་གཡོག་བཀོལཝ་ཨིན།
match-finding/sequence བཟོ་ནི་དང་ ཀེར་ཐིག་ནང་ གཏན་འབེབས་བཟོ་མི་ zstd གཞི་ཁྲམ་ལག་ལེན་འཐབ་ཨིན།
incoder (ཧཕ་མེན་/ཨེཕ་ཨེསི་ཨི་ + གཞི་ཁྲམ་ཚོགས་སྡེ་) ཧོསིཊི་གུ་; decode གིས་ ཀེར་ཐིག་ནང་གཞི་ཁྲམ་ལག་ལེན་འཐབ་ཨིན།
ཇི་པི་ཡུ་སྡེབ་ཚན་ཌི་ཀོཌི་ནང་མ་ལྷོད་ཚུན་ཚོད་ རྒྱབ་སྐྱོར་མ་འབད་བའི་གཞི་ཁྲམ་ཚུ་གི་དོན་ལུ་ སི་པི་ཡུ་ཛཱསི་ཊི་ཌི་ཕོལཀ་བེག་དང་གཅིག་ཁར་ ཌི་ཀོཌར་དང་གཅིག་ཁར་།

| ཕིལཌ་ | སྔོན་སྒྲིག་ | དམིགས་ཡུལ། |
|---------|--------------|------------|
| `min_compress_bytes_cpu` | `256` བཱའིཊི་ | འདི་གི་འོག་ལུ་ མགུ་ཏོག་གུ་ལས་ འཛེགས་ནི་ zstd འདི་ ཡོངས་རྫོགས་སྦེ་ བཀག་ཐབས་འབདཝ་ཨིན། |
| `min_compress_bytes_gpu` | `1_048_576` བཱའིཊི་ (༡ཨེམ་ཨའི་བི་) | `norito::core::hw::has_gpu_compression()` འདི་བདེན་པ་ཡོད་པའི་སྐབས་ ཚད་འཛིན་འདི་ ཇི་པི་ཡུ་ zstd ལུ་སྤྲོད་ལེན་འབདཝ་ཨིན། |
| `zstd_level_small` / `zstd_level_large` | ```
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
``` / `3` | <32KiB དང་ ≥32KiB པེ་ལོཌི་ཚུ་གི་དོན་ལུ་ སི་པི་ཡུ་བསྡམ་བཞག་པའི་གནས་ཚད། |
| Norito `1` | བརྡ་བཀོད་གྲལ་ཐིག་ཚུ་བཀང་པའི་སྐབས་ འཕྲོ་མཐུད་རྒྱུན་རིམ་སྦེ་བཞག་ནི་ལུ་ ཀོན་སར་ཝེ་ཊིབ་ཇི་པི་ཡུ་གནས་རིམ། |
| `large_threshold` | `32_768` བཱའིཊི་ | “ཆུང་ཆུང་” དང་ “ཆེ་བ་” སི་པི་ཡུ་ zstd གནས་རིམ་གྱི་བར་ན་ ཚད་ཀྱི་མཚམས་ཚད། |
| `aos_ncb_small_n` | `64` གྱལ་རིམ་ | འདི་གི་འོག་ལུ་ རྩིས་རྐྱབ་སྟེ་ མཐུན་སྒྲིག་ཅན་གྱི་ཨེན་ཀོ་ཌར་ཚུ་གིས་ AoS དང་ NCB བཀོད་སྒྲིག་གཉིས་ཆ་ར་ ཞིབ་དཔྱད་འབདཝ་ཨིན། |
| `combo_no_delta_small_n_if_empty` | `2` གྱལ་རིམ་ | གྲལ་ཐིག་༡–༢ ནང་ ནང་ཐིག་སྟོངམ་ཡོད་པའི་སྐབས་ u32/id delta encodings ལྕོགས་ཅན་བཟོ་ནི་འདི་ སྔོན་བཀག་འབདཝ་ཨིན། |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | ཌེལ་ཊས་ཀྱིས་ ཉུང་ཤོས་རང་ གྱལ་རིམ་གཉིས་ཡོདཔ་ཨིན། |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | ཌེལ་ཊ་བསྒྱུར་བཅོས་ཚུ་ཆ་མཉམ་རང་ སྤྱོད་ལམ་ལེགས་ཤོམ་ཡོད་པའི་ ཨིན་པུཊི་ཚུ་གི་དོན་ལུ་ སྔོན་སྒྲིག་གིས་ ལྕོགས་ཅན་བཟོ་ཡོདཔ་ཨིན། |
| `combo_enable_name_dict` | `true` | ཧིཊ་ཆ་ཚད་ཚུ་གིས་ དྲན་ཚད་མགུ་ལུ་བདེན་དཔང་འབད་བའི་སྐབས་ ཀེར་ཐིག་རེ་རེ་གི་ཚིག་མཛོད་ཚུ་ འབད་བཅུགཔ་ཨིན། |
| `combo_dict_ratio_max` | `0.40` | གྲལ་ཐིག་ ༤༠ ལས་ལྷག་སྟེ་ ཁྱད་པར་ཡོད་པའི་སྐབས་ ཚིག་མཛོད་ཚུ་ ལྕོགས་མིན་བཟོ། |
| `combo_dict_avg_len_min` | `8.0` | བཟོ་བསྐྲུན་གྱི་ཚིག་མཛོད་ཀྱི་ཧེ་མ་ སྤྱིར་སྙོམས་ཡིག་རྒྱུན་རིང་ཚད་ ≥༨ དགོ (ཐུང་ཀུའི་ནང་ཐིག་ཚུ་ གྱལ་རིམ་ནང་སྡོད།) |
| `combo_dict_max_entries` | `1024` | མཐའ་མཚམས་དྲན་ཚད་ལག་ལེན་གྱི་ ཁས་ལེན་འབད་ནི་ལུ་ ཚིག་མཛོད་ཐོ་བཀོད་ཚུ་གུ་ ཧརཌི་ཀེབ་རྐྱབས། |

འ་ནི་ heuristics གིས་ GPU ལྕོགས་ཅན་བཟོ་མི་ ཧོསིཊི་ཚུ་ སི་པི་ཡུ་རྐྱངམ་ཅིག་གི་ མཉམ་རོགས་: གདམ་འཐུ་འབད་མི་འདི་ བཞགཔ་ཨིན།
ཐགཔ་གི་རྩ་སྒྲིག་འདི་བསྒྱུར་བཅོས་འབད་ནི་གི་ཐག་བཅད་འདི་ ནམ་ཡང་མི་བཟོ།
གློད་གྲོལ། གསལ་སྡུད་འབད་བའི་སྐབས་ བརྡབ་འགྱོ་བའི་སྐུགས་ཚུ་ཡང་ ལེགས་ཤོམ་སྦེ་གསལ་སྟོན་འབད་བའི་སྐབས་ Norito གིས་ དུས་མཐུན་བཟོ་ཡོདཔ་ཨིན།
ཚད་ལྡན་ `Heuristics::canonical` ལག་བསྟར་དང་ Norito དང་།
`status.md` ཐོན་རིམ་འབད་ཡོད་པའི་སྒྲུབ་བྱེད་ཀྱི་མཉམ་དུ་འགྱུར་བ་འདི་ཐོ་བཀོད་འབད།GPU zstd གྲོགས་རམ་པ་གིས་ `min_compress_bytes_gpu` གི་བཏོག་བཏོགཔ་ཨིན།
ཐད་ཀར་ (དཔེར་ན་ `norito::core::gpu_zstd::encode_all` བརྒྱུད་དེ་) དེ་འདྲའི་ཆུང་བ།
པེ་ལོཌི་ཚུ་ ཇི་པི་ཡུ་ཐོབ་ཐངས་ལུ་མ་ལྟོས་པར་ སི་པི་ཡུ་འགྲུལ་ལམ་གུ་ ཨ་རྟག་ར་སྡོད་དོ་ཡོདཔ་ཨིན།

### དཀའ་ངལ་སེལ་ཐབས་དང་ ཆ་འཇོག་དཔྱད་གཞི།

- `cargo xtask acceleration-state --format json` དང་བཅས་པར་ལེན་དུས་ཚོད་གནས་སྟངས་དང་བཞག་དགོ།
  ཐོན་འབྲས་འདི་ འཐུས་ཤོར་བྱུང་བའི་དྲན་ཐོ་གང་རུང་ཅིག་དང་གཅིག་ཁར་; སྙན་ཞུ་འདི་གིས་ རིམ་སྒྲིག་/འཐོབ་ཚུགས་པའི་རྒྱབ་ཐག་ཚུ་སྟོནམ་ཨིན།
  plus sparity/མཇུག་གི་འཛོལ་བ་ཡིག་རྒྱུན་ཚུ།
- འཕྱེལ་འགྱོ་ནི་ལུ་ ས་གནས་ནང་ ས་གནས་ནང་ མགྱོགས་ཚད་ཕྱོགས་ཀྱི་ རི་གེ་རེ་ཤཱན་ ལོག་སྟེ་ གཡོག་བཀོལ།
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (སི་པི་ཡུ་-རྐྱངམ་ཅིག་ མགྱོགས་དྲགས་-རྐྱངམ་ཅིག་) གཡོག་བཀོལཝ་ཨིན། རྒྱུག་པའི་དོན་ལུ་ `acceleration_runtime_status()` ཐོ་བཀོད་འབད།
- རྒྱབ་ཐག་ཅིག་གིས་ རང་དོན་བརྟག་དཔྱད་ཚུ་ འཐུས་ཤོར་བྱུང་པ་ཅིན་ མཐུད་མཚམས་འདི་ སི་པི་ཡུ་-རྐྱངམ་ཅིག་ ཚད་གཞི་ (`enable_metal =
  farme 18NI00000132222222222cuda = flain`) དང་ བཟུང་ཡོད་པའི་ཆ་སྙོམས་ཨའུཊི་པུཊི་དང་གཅིག་ཁར་ བྱུང་རྐྱེན་ཅིག་ཁ་ཕྱེ།
  རྒྱབ་ཐག་འདི་བཀག་བཞག་ནིའི་ཚབ་ལུ། གྲུབ་འབྲས་ཚུ་ ཐབས་ལམ་ཚུ་གི་བར་ན་ ཐག་བཅད་བཞག་དགོ།
- **CUDA ཆ་སྙོམས་དུ་བ་ (lab NV མཐུན་རྐྱེན་):** གཡོག་བཀོལ།
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x མཐུན་རྐྱེན་ལུ་ `cargo xtask acceleration-state --format json` བཟུང་ཞིནམ་ལས་ མཉམ་སྦྲགས་འབད།
  གནས་ཚད་ཀྱི་པར་རིས་ (GPU དཔེ་སྟོན་/འདྲེན་བྱེད་ཚུད་ཡོདཔ་) འདི་ བེན་ཀ་མཱརཀ་ ཅ་རྙིང་ཚུ་ལུ་ཨིན།