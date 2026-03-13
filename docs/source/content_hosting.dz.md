---
lang: dz
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% ནང་དོན་ ཧོསིཊིང་ལམ།
% Iroha ཀོར།

# ནང་དོན་ཧོསཊི་ ལམ།

ནང་དོན་ལམ་འདི་གིས་ གནས་སྟངས་མེད་པའི་ བཱན་ཌལ་ཆུང་ཀུ་ཚུ་ རིམ་སྒྲིག་འབད་དེ་ ཕྱག་ཞུ་དོ་ཡོདཔ་ཨིན།
ཡིག་སྣོད་ངོ་རྐྱང་ཚུ་ ཐད་ཀར་དུ་ Torii ལས་ཨིན།

- **དཔེ་སྐྲུན་**: `PublishContentBundle` འདི་ ཊར་ཡིག་མཛོད་དང་གཅིག་ཁར་ བཙུགས་དགོ། གདམ་ཁ་ཅན་གྱི་དུས་སྟོན།
  མཐོ་ཚད་དང་ གདམ་ཁའི་མངོན་གསལ་ཅིག། བཱན་ཌལ་ཨའི་ཌི་འདི་ blax2b a of the ཨིན།
  ཊར་བཱོལ་. ཊར་ཐོ་བཀོད་ཚུ་ དུས་རྒྱུན་གྱི་ཡིག་སྣོད་ཚུ་ཨིན་དགོ། མིང་ཚུ་ སྤྱིར་བཏང་བཟོ་ཡོད་པའི་ UTF-8 འགྲུལ་ལམ་ཚུ་ཨིན།
  ཚད་/ལམ་/ཡིག་སྣོད་-གྱངས་ཁ་-རྩིས་རྟགས་ཚུ་ `content` རིམ་སྒྲིག་ (`max_bundle_bytes`, ལས་འོངམ་ཨིན།
  `max_files`, `max_path_len`, `max_retention_blocks`, Torii).
  མངོན་གསལ་ཚུ་ནང་ Norito-index ཧེཤ་ གནད་སྡུད་ས་སྟོང་/ལམ་ཐིག་ འདྲ་མཛོད་སྲིད་བྱུས་ཚུ་ཚུདཔ་ཨིན།
  (Torii, `immutable`), བདེན་བཤད་ཐབས་ལམ་ (`public` / `role:<role>` /
  `sponsor:<uaid>` དང་ བཀག་འཛིན་སྲིད་བྱུས་ས་གནས་འཛིན་མི་དང་ MIME གིས་ བཀག་ཆ་འབད་ཡོདཔ།
- **རྩིས་ཞིབ་**: ཏར་པེ་ལོཌི་ཚུ་ ཆ་ཤས་ (སྔོན་སྒྲིག་ ༦༤ཀིབི་) དང་ གཅིག་རེ་ གསོག་འཇོག་འབདཝ་ཨིན།
  གཞི་བསྟུན་རྩིས་ཐོ་ཚུ་དང་གཅིག་ཁར་ hash; དགོངས་ཞུ་འབད་དེ་ མར་ཕབ་དང་ གཞུ་དབྱིབས་ཀྱི་ཆ་ཤས་ཚུ་ དགོངས་ཞུ་འབད་ཡོདཔ།
- **ཞབས་ཏོག་**: Torii `GET /v2/content/{bundle}/{path}` ཕྱིར་འདོན་འབདཝ་ཨིན། ལན་འདེབས་རྒྱུན་ལམ།
  ཐད་ཀར་གྱི་ཆན་ཀ་ལས་ `ETag` = ཡིག་སྣོད་ཧེཤ་, `Accept-Ranges: bytes`,
  ཁྱབ་ཚད་རྒྱབ་སྐྱོར་དང་ གསལ་སྟོན་ལས་བྱུང་བའི་ འདྲ་མཛོད་ཚད་འཛིན་འབད། ལྷག་བསམ་རྣམ་དག་གིས་གུས་ཞབས་ནི།
  གསལ་བསྒྲགས། འགན་འཁུར་དང་རྒྱབ་སྐྱོར་འབད་མི་ལན་འདེབས་ཚུ་ལུ་ ཁྲིམས་ལུགས་དགོཔ་ཨིན།
  ཞུ་བ་མགོ་ཡིག་ཚུ་ (`X-Iroha-Account`, `X-Iroha-Signature`) མཚན་རྟགས་བཀོད་པའི་དོན་ལུ་ཨིན།
  རྩིས་ཁྲ; བརླག་སྟོར་ཞུགས་མི་/དུས་ཡུན་ཚང་བའི་ བུནཌལ་ཚུ་ ༤༠༤ ལོག་འོང་།
- **CLI**: `iroha content publish --bundle <path.tar>` ད་ལྟ།
  རང་བཞིན་གྱིས་གསལ་སྟོན་འབདཝ་ཨིན་ གདམ་ཁ་ཅན་ `--manifest-out/--bundle-out`, དང་།
  `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--lane`, `--immutable`, `--immutable`, `--immutable`, `--immutable`,
  དང་ `--expires-at-height` བརྒལ་ཡོད། `iroha content pack --root <dir>` བསྐྲུན།
  a determistic tarball + ག་ནི་ཡང་མ་བཙུགས་པར་ གསལ་སྟོན་འབད།
- **Config**: འདྲ་མཛོད་/རྩོམ་སྒྲུང་གི་མཛུབ་མོ་ཚུ་ `content.*` ནང་ `iroha_config` ནང་སྡོད་དོ་ཡོདཔ་ཨིན།
  (I 18NI00000038X, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) དང་ དཔར་བསྐྲུན་དུས་ཚོད་ནང་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
- **SLO + ཚད་གཞི་***: `content.max_requests_per_second` / `request_burst` དང་།
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` ཁབ་ལེན་ཀླད་པ།
  proput; Torii བཱའིཊི་དང་ཕྱིར་འདྲེན་ཚུ་ ཕྱག་ཞུ་མ་ཚར་བའི་ཧེ་མ་ གཉིས་ཆ་ར་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
  `torii_content_requests_total`, Iroha, དང་།
  གྲུབ་འབྲས་ཁ་ཡིག་ཚུ་དང་གཅིག་ཁར་ `torii_content_response_bytes_total` མེ་ཊིགས། རྒྱབ་ལྗོངས།
  དམིགས་ཚད་ཚུ་ `content.target_p50_latency_ms` གི་འོག་ལུ་ཡོདཔ་ཨིན།
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **རྒྱབ་བཤུད་ཚད་འཛིན་ཚུ་**: ཚད་གཞི་གི་བཱ་ཀེཊི་ཚུ་ ཡུ་ཨའི་ཌི་/ཨེ་པི་ཨའི་ཊོ་ཀེན་/ཐག་རིང་ཨའི་པི་གིས་ ལྡེ་མིག་བརྐྱབ་ཡོདཔ་དང་ ཅིག།
  གདམ་ཁའི་ པོ་ཝ་ སྲུང་སྐྱོབ།
  ལྷག་མ་ཚར་བའི་ཧེ་མ་དགོཔ་ཨིན། DA stripe བཀོད་སྒྲིག་སྔོན་སྒྲིག་སྔོན་སྒྲིག་ཚུ་ ལས།
  `content.stripe_layout` དང་ འོང་འབབ་/སྣ་མང་ཧ་ཤེ་ཚུ་ནང་ བསྐྱར་སྒྲོག་འབདཝ་ཨིན།
- **ལེན་དང་ DA སྒྲུབ་བྱེད་**: མཐར་ཕྱིན་གྱི་ལན་འདེབས་མཉམ་སྦྲགས།
  Torii (base64 (base64T0000000001X-frabed Torii) འབག་པ།
  `bundle_id`, `path`, `file_hash`, Norito, ཕྱག་ཞུ་བའི་ཕྱག་ཞུ་མི་ ཁྱད་ཆོས།
  `chunk_root` / Iroha གདམ་ཁའི་PDP ཁས་བླངས་དང་དེ་བས་དུས་ཚོད་ཀྱི་རྟགས་བཀོད་ཡོད།
  གཟུགས་པོ་འདི་ ལོག་སྟེ་མ་ལྷག་པར་ ག་ཅི་འབག་འོང་ཡི་ག་ མཁོ་མངགས་འབད་མི་ཚུ་གིས་ བཀལ་ཚུགས།

ལྡེ་མིག་གཞི་བསྟུན་ཚུ།- གནས་སྡུད་དཔེ་ཚད།: `crates/iroha_data_model/src/content.rs`
- ལག་བསྟར་: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ལེགས་སྐྱོང་: `crates/iroha_torii/src/content.rs`
- སི་ཨེལ་ཨའི་གྲོགས་རམ་པ་: `crates/iroha_cli/src/content.rs`