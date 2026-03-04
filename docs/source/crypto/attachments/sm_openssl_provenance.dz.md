---
lang: dz
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tonguo ལས་རིགས་བརྡ་རྟགས་བརྡ་སྟོན།
% བཟོ་སྐྲུན་: ༢༠༢༦-༠༡-༣༠

# མཐའ་འཁོར་བཅུད་བསྡུས།

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: སྙན་ཞུ་ `LibreSSL 3.3.6` (system-provided ཊི་ཨེལ་ཨེསི་ལག་ཆས་མེཀ་ཨོ་ཨེསི་)།
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: འདི་ ངེས་བདེན་ རཱསི་ བརྟེན་པའི་ བང་རིམ་ (`openssl` crate v0.10.74, `openssl-sys` v0.9.x, བཀྲམ་སྤེལ་འབད་ཡོད་པའི་ OpenSSL 3.x གི་ཐོན་ཁུངས་བརྒྱུད་དེ་ འཐོབ་ཚུགསཔ་ཨིན། ཁྱད་རྣམ་ `vendored` འདི་ གཏན་འབེབས་བཟོ་ནིའི་དོན་ལུ་ `crates/iroha_crypto/Cargo.toml` ནང་ལུ་ལྕོགས་ཅན་བཟོ་ཡོདཔ་ཨིན་)།

# མཆན།

- ས་གནས་གོང་འཕེལ་མཐའ་འཁོར་འབྲེལ་བ། ལི་བི་རི་ཨེསི་ཨེསི་ཨེལ་མགོ་ཡིག་/དཔེ་མཛོད་ཚུ། བཟོ་བསྐྲུན་སྔོན་ལྟ་བཟོ་བསྐྲུན་ཚུ་གིས་ OpenSSL >= ༣.༠.༠ ཡང་ན་ ཊོང་སུའོ་ ༨.x ལག་ལེན་འཐབ་དགོ། མཐའ་མཇུག་གི་ ཅ་རྙིང་བཱན་ཌལ་བཟོ་བཏོན་འབད་བའི་སྐབས་ རིམ་ལུགས་ལག་ཆས་ཆས་ཆས་འདི་ཚབ་བཙུགས་ ཡང་ན་ `OPENSSL_DIR`/`PKG_CONFIG_PATH` གཞི་སྒྲིག་འབད།
- གསར་བཏོན་འབད་མི་མཐའ་འཁོར་ནང་ལུ་ པར་རིས་འདི་ བསྐྱར་བཟོ་འབད་ནི་གི་དོན་ལུ་ ངེས་བདེན་ OpenSSL/Tongsoo tarball hash (`cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`, `openssl version -b`, `openssl version -f`) དེ་ལས་ བསྐྱར་བཟོ་འབད་བཏུབ་པའི་ བཟོ་བསྐྲུན་གྱི་ཡིག་གཟུགས་/བརྟག་དཔྱད་འདི་ མཉམ་སྦྲགས་འབད། བཙོང་མི་ཚུ་གི་དོན་ལུ་ `openssl-src` cret ཐོན་རིམ་/commum འདི་ Cargo གིས་ལག་ལེན་འཐབ་མི་ (`target/debug/build/openssl-sys-*/output` ནང་ལུ་མཐོང་ཚུགསཔ་) ཐོ་བཀོད་འབད།
- Apple Silicon གིས་ OpenSSL གི་ཐ་མག་འཐུང་པའི་སྐབས་ `RUSTFLAGS=-Aunsafe-code` དགོཔ་ཨིན། ཡིག་གཟུགས་ `scripts/sm_openssl_smoke.sh` གིས་ CI དང་ ཉེ་གནས་ཚུ་ རིམ་མཐུན་སྦེ་བཞག་ནིའི་དོན་ལུ་ `cargo` འདི་ འབོད་བརྡ་མ་འབད་བའི་ཧེ་མ་ དར་ཚིག་འདི་ཕྱིར་འདྲེན་འབདཝ་ཨིན།
- ཡར་འཕེལ་གྱི་འབྱུང་ཁུངས་འབྱུང་ཁུངས་ (དཔེར་ན་ `openssl-src-<ver>.tar.gz` SHA256) ཐུམ་སྒྲིལ་གྱི་མདོང་ལམ་འདི་བཙུགས་ཚར་བའི་ཤུལ་ལས་ མཉམ་སྦྲགས་འབད། CI གི་ཅ་རྙིང་ཚུ་ནང་ ཧེཤ་གཅིགཔོ་ལག་ལེན་འཐབ།