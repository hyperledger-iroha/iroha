---
lang: dz
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI མཚན་རྟགས་བཀོད་པ།
---------------

- Torii OpenAPI spec (`torii.json`) མཚན་རྟགས་བཀོད་དགོཔ་དང་ གསལ་སྟོན་འདི་ `cargo xtask openapi-verify` གིས་བདེན་དཔྱད་འབད་ཡོདཔ་ཨིན།
- མཚན་རྟགས་བཀོད་མི་ལྡེ་མིག་ཚུ་ `allowed_signers.json` ནང་སྡོད་བཅུགཔ་ཨིན། མཚན་རྟགས་ལྡེ་མིག་འདི་བསྒྱུར་བཅོས་འགྱོཝ་ད་ ཡིག་སྣོད་འདི་བསྒྱིར་དགོ། `version` འདི་ `1` ལུ་བཞག་དགོ།
- CI (`ci/check_openapi_spec.sh`) གིས་ ཧེ་མ་ལས་རང་ གསརཔ་དང་ད་ལྟོའི་ཁྱད་ཚད་གཉིས་ཆ་རའི་དོན་ལུ་ ཐོ་ཡིག་འདི་ བསྟར་སྤྱོད་འབདཝ་ཨིན། དྲྭ་ཚིགས་གཞན་ཅིག་ཡང་ན་ པའིཔ་ལའིན་གྱིས་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ ཁྱད་ཚད་འདི་ ཟ་སྤྱོད་འབད་བ་ཅིན་ བདེན་དཔྱད་ཀྱི་གོ་རིམ་འདི་ ཐོ་ཡིག་ཡིག་སྣོད་གཅིག་ནང་ བརྡ་སྟོནམ་ཨིན།
- ལྡེ་མིག་བསྒྱིར་བའི་ཤུལ་ལས་ ལོག་སྟེ་མཚན་རྟགས་བཀོད་ནི།
  1. མི་མང་ལྡེ་མིག་གསརཔ་དང་གཅིག་ཁར་ `allowed_signers.json` དུས་མཐུན་འབད།
  ༢ ཁྱད་ཚད་འདི་ བསྐྱར་བཟོ་འབད་ནི/མིང་རྟགས་བཀོད།: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  ༣ གསལ་སྟོན་འདི་ ཐོ་ཡིག་དང་མཐུན་སྒྲིག་འབད་ནི་ལུ་ `ci/check_openapi_spec.sh` (ཡང་ན་ `cargo xtask openapi-verify` ལག་ཐོག་ལས་) ལོག་སྟེ་གཡོག་བཀོལ་དགོ།