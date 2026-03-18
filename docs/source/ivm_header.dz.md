---
lang: dz
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Bytecode མགོ་ཡིག་ IVM


བྱེ༌ཤོ༌
- 4 བཱའིཊི་: ཨེ་ཨེསི་སི་ཨའི་ `IVM\0` ཨོ་ཨོཕ་སེཊི་ ༠ ལུ།

སྒྲིག་བཀོད་ (ད་ལྟོ)
- བཤག་བཅོས་དང་ཚད་གཞི། (བཱའིཊི་༡༧ བསྡོམས་):
  - ༠..༤: མགུ་སྐོར་ `IVM\0`
  - ༤: `version_major: u8`
  - ༥: `version_minor: u8`
  - ༦: `mode: u8` (ཁྱད་རྣམ་བིཊི་ཚུ་; འོག་ལུ་བལྟ།)
  - ༧: `vector_length: u8`
  - 8..16: `max_cycles: u64` (ཆུང་ཀུའི ‑enentian).
  - ༡༦: `abi_version: u8`

ཐབས་ལམ་བིཊི་ཚུ།
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (བཀག་ཆ་/ཁྱད་ཆོས>

ས་སྒོ་ཚུ་ (དོན་དག་)
- `abi_version`: སི་ཀཱལ་ཐིག་ཁྲམ་དང་ དཔག་བྱེད་ཀྱི་ཐིག་ཁྲམ་/ཨེ་བི་ཨའི་ ལས་འཆར་གྱི་ཐོན་རིམ་།
- `mode`: ZK འཚོལ་ཞིབ་ཀྱི་དོན་ལུ་ ཁྱད་རྣམ་བིཊི་/VECTOR/HTM.
- `vector_length`: ཝེག་ཊར་ཨོཔ་ཚུ་གི་དོན་ལུ་ གཏན་ཚིག་ཅན་གྱི་བེག་ཊར་རིང་ཚད་ (༠ → གཞི་སྒྲིག་མ་འབད་མི)།
- `max_cycles`: ZK ཐབས་ལམ་དང་ འཛུལ་ཞུགས་ནང་ ལག་ལེན་འཐབ་ཡོད་པའི་ ལག་ལེན་གྱི་ མཐའ་མཚམས་ བསྡམས།

དྲན་ཐོ་ཚུ།
- ཨེན་ཌི་ཡཱན་དང་ བཀོད་སྒྲིག་འདི་ ལག་ལེན་འཐབ་ཐོག་ལས་ ངེས་ཚིག་བརྗོད་དེ་ `version` ལུ་ མཐུད་ཡོདཔ་ཨིན། གོང་ལུ་ལན་གུ་ཡོད་པའི་སྒྲིག་བཀོད་འདི་གིས་ ད་ལྟོའི་ལག་ལེན་འཐབ་ཐངས་འདི་ `crates/ivm_abi/src/metadata.rs` ནང་ལུ་ གསལ་སྟོན་འབདཝ་ཨིན།
- ཉུང་མཐའ་ལྷག་མི་འདི་གིས་ ད་ལྟོའི་ཅ་རྙིང་ཚུ་གི་དོན་ལུ་ བཀོད་རིས་འདི་ལུ་ བློ་གཏད་ཚུགས་ནི་དང་ མ་འོངས་པའི་འགྱུར་བ་ཚུ་ `version` གཱེཊི་བརྒྱུད་དེ་ འཛིན་སྐྱོང་འཐབ་དགོ།
- མཐུན་རྐྱེན་མགྱོགས་ཚད་ (SIMD/Metal/CUDA) འདི་ ཧོསིཊི་རེ་ལུ་ སེལ་འཐུ་འབད་ཡོདཔ་ཨིན། རན་ཊའིམ་འདི་གིས་ `iroha_config`: `enable_simd` རྫུན་མ་ད་ལུ་ scalar farlobks འདི་ རྫུན་མ་ཨིན་ དེ་ལས་ IVM དང་ `enable_cuda` གིས་ ག་བསྡུར་རྐྱབ་པའི་སྐབས་ལུ་ཡང་ འདི་ཚུ་གི་རྒྱབ་རྒྱབ་ཚུ་ མཐུད་དེ་ཡོདཔ་ཨིན། ཝི་ཨེམ་གསར་བསྐྲུན་མ་འབད་བའི་ཧེ་མ་ `ivm::set_acceleration_config` ཨིན།
- འགྲུལ་འཕྲིན་ཨེསི་ཌི་ཀེ་ཚུ་ (ཨེན་ཌོརཌི་/སུའིཕཊ) གིས་ མཐུད་མཚམས་གཅིག་པ་སྦེ་; `IrohaSwift.AccelerationSettings`
  `connect_norito_set_acceleration_config` ཟེར་སླབ་ཨིན།
  NEOO
- བཀོལ་སྤྱོད་པ་ཚུ་གིས་ `IVM_DISABLE_METAL=1` ཡང་ན་ `IVM_DISABLE_CUDA=1` ཕྱིར་འདྲེན་འབད་དེ་ ནད་བརྟག་གི་དོན་ལུ་ དམིགས་བསལ་གྱི་རྒྱབ་ཐག་ཚུ་ བཀག་ཆ་འབད་ཚུགས། མཐའ་འཁོར་འདི་ཚུ་གིས་ རིམ་སྒྲིག་ལས་ གཙོ་བོར་བཏོན་ཞིནམ་ལས་ ཝི་ཨེམ་འདི་ གཏན་འབེབས་སི་པི་ཡུ་འགྲུལ་ལམ་གུ་བཞག།

ཐུབ་ཚད་ལྡན་པའི་གནས་སྟངས་རོགས་སྐྱོར་དང་ཨེ་བི་ཨའི་གི་ངོས་འཛིན།
- ཐུབ་ཚད་ཅན་གྱི་གནས་སྟངས་གྲོགས་རམ་གྱི་སི་ཀཱལ་ཚུ་ (༠x༥༠–༠x༥A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* དང་ JSON/SCHEMA encode/decode) ཚུ་ V1 ABI གི་ཆ་ཤས་ཅིག་ཨིནམ་དང་ `abi_hash` གློག་ཀླད་ནང་ཚུདཔ་ཨིན།
- CoreHost wiers STATE_{GET,SET,DEL} dev/test hosts ཚུ་གིས་ བཀབ་བཙུགས་ཡང་ན་ ཉེ་གནས་བརྟན་ཏོག་ཏོ་ཚུ་ལག་ལེན་འཐབ་འོང་ དེ་འབདཝ་ད་ བལྟ་བརྟོག་འབད་བཏུབ་པའི་སྤྱོད་ལམ་གཅིགཔོ་འདི་ ཉམས་སྲུང་འབད་དགོཔ་ཨིན།

བདེན་དཔང་།
- མཐུད་མཚམས་འཛུལ་ཞུགས་འདི་གིས་ `version_major = 1` དང་ `version_minor = 0` རྐྱངམ་ཅིག་ངོས་ལེན་འབདཝ་ཨིན།
- `mode` ཤེས་རྟོགས་ཡོད་པའི་བིཊི་ཚུ་རྐྱངམ་ཅིག་འོང་དགོཔ་ཨིན་: `ZK`, `VECTOR`, `HTM` (མ་ཤེས་པའི་བིཊི་ཚུ་བཀག་ཆ་འབད་ཡོདཔ་)།
- `vector_length` འདི་ བསླབ་བྱའི་ཨིནམ་དང་ `VECTOR` བིཊི་འདི་གཞི་སྒྲིག་མ་འབད་རུང་ ཀླད་ཀོར་མེན་རུང་ ཀླད་ཀོར་མེནམ་འོང་། འཛུལ་ཞུགས་བཀག་སྡོམ་འདི་ གོང་མའི་མཐའ་མཚམས་རྐྱངམ་ཅིག་ཨིན།
- རྒྱབ་སྐྱོར་ `abi_version` གནས་གོང་ཚུ་: འགོ་དང་པ་གསར་བཏོན་འདི་གིས་ ```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
``` (V1) རྐྱངམ་ཅིག་ངོས་ལེན་འབདཝ་ཨིན། འཛུལ་ཞུགས་ནང་ གནས་གོང་གཞན་ཚུ་ ངོས་ལེན་མ་འབད་བས།

### སྲིད་བྱུས།
འོག་གི་སྲིད་བྱུས་བཅུད་དོན་འདི་ ལག་ལེན་འཐབ་ཐངས་ལས་བཟོ་བཏོན་འབད་ཡོདཔ་ལས་ ལག་ཐོག་ལས་ཞུན་དག་འབད་མི་བཏུབ།<!-- BEGIN GENERATED HEADER POLICY -->
| ཕིལཌ་ | སྲིད་བྱུས།
|---|------|
| ཐོན་རིམ་_མི | 1 |
| ཐོན་རིམ་_མི་ནར་ | 0 |
| ཐབས་ལམ་ (ཤེས་རྟོགས་ཡོད་པའི་བིཊི་) | 0x07 (ZK=0x01, ཝིཀ་ཊོར་=༠x༠༢, ཨེཆ་ཊི་ཨེམ་=༠x༤) |
| abi_version | 1 |
| vector_length | ༠ ཡང་ན་ ༡..=༦༤ (བསླབ་བྱ་; VECTOR བིཊི་ལས་རང་དབང་ཅན་) |
<!-- END GENERATED HEADER POLICY -->

### ABI Hashes (བཟོ་བཀོད།)
འོག་གི་ཐིག་ཁྲམ་འདི་ ལག་ལེན་ལས་བཟོ་བཏོན་འབད་ཡོདཔ་དང་ རྒྱབ་སྐྱོར་འབད་ཡོད་པའི་སྲིད་བྱུས་ཚུ་གི་དོན་ལུ་ ཚད་ལྡན་ `abi_hash` གནས་གོང་ཚུ་ཐོ་ཡིག་བཀོདཔ་ཨིན།

<!-- BEGIN GENERATED ABI HASHES -->
| སྲིད་བྱུས། ཨ་བི་_ཧཤ་ (ཧེགས་) |
|---|------|
| ABI v1 | b1786031c3d0cdbd607debdae1ccc611a0807bf9cf49ed349a0063285724969f |
<!-- END GENERATED ABI HASHES -->

- དུས་མཐུན་ཆུང་བ་ཚུ་གིས་ ```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
``` དང་ བཀག་བཞག་ཡོད་པའི་ opcode གི་ས་སྟོང་རྒྱབ་ཁར་ བཀོད་རྒྱ་ཁ་སྐོང་འབད་ཚུགས། དུས་མཐུན་བཟོ་མི་ཚུ་གིས་ ཨིན་ཀོ་ཌིང་ཚུ་བསྒྱུར་བཅོས་འབད་ནི་ཡང་ན་ མཐུན་སྒྲིག་ཡར་འཕེལ་དང་གཅིག་ཁར་ མཉམ་གཅིག་སྦེ་ བཏོན་གཏང་ནི་/ བསྐྱར་ལོག་འབད་འོང་།
- སི་ཀཱལ་ཁྱབ་ཚད་ཚུ་ བརྟན་ཏོག་ཏོ་སྦེ་ཡོདཔ་ཨིན། མ་ཤེས་པའི་ `abi_version` གིས་ `E_SCALL_UNKNOWN` ཐོན་སྐྱེད་འབདཝ་ཨིན།
- རླངས་རྫས་ཀྱི་དུས་ཚོད་འདི་ <!-- BEGIN GENERATED HEADER POLICY --> ལུ་མཐུད་དེ་ཡོདཔ་ལས་ བསྒྱུར་བཅོས་འབད་བའི་སྐབས་ གསེར་གྱི་ནད་འབུཔ་དགོཔ་ཨིན།

ཅ་རྙིང་ གཟུང་བྱེད་པ།
- མགོ་ཡིག་ས་སྒོ་ཚུ་གི་མཐོང་སྣང་གཏན་ཏོག་ཏོ་ཅིག་གི་དོན་ལུ་ IVM ལག་ལེན་འཐབ།
- དཔེ་ཚུ་/ བཟོ་བསྐྲུན་འབད་མི་ ཅ་རྙིང་ཚུ་གུ་བརྟག་དཔྱད་འབད་མི་ མེཀ་ཕའིལ་དམིགས་གཏད་ ```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
``` ཆུང་ཀུ་ཅིག་ཚུདཔ་ཨིན།

དཔེ་ (Rust): ཉུང་མཐའ་མགུ་སྐོར་ + ཚད་བརྟག་དཔྱད།

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

དྲན་འཛིན་: མགུ་སྐོར་ལས་ལྷག་སྟེ་ མགོ་ཡིག་སྒྲིག་བཀོད་ངོ་མ་འདི་ ཐོན་རིམ་བཟོ་སྟེ་ ལག་ལེན་འཐབ་ཡོདཔ་ཨིན། བརྟན་བརྟན་གྱི་ས་སྒོའི་མིང་དང་གནས་གོང་ཚུ་གི་དོན་ལུ་ <!-- BEGIN GENERATED HEADER POLICY --> ལུ་དགའ།