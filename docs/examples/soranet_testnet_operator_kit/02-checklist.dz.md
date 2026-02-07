---
lang: dz
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] བསྐྱར་ཞིབ།
- [ ] IPv4 + IPv6 ཁ་བྱང་གཉིས་ངེས་འཛིན་འབད་དེ་ ཡར་འཕེལ་གྱིས་ QUIC/UDP 443.
- [ ] དགོངས་དོན་ HSM ཡང་ན་ ངོ་རྟགས་ལྡེ་མིག་ཚུ་ བརྒྱུད་སྤྲོད་འབད་ནིའི་དོན་ལུ་ བློ་གཏད་ཅན་གྱི་ ཉེན་སྲུང་ཅན་གྱི་ enclave.
- [ ] མཉམ་འབྱུང་ཚད་འཛིན-ཕྱིར་ཐོན་ཐོ་གཞུང་ (I18NI0000001X).
- [ ] མཉམ་བསྡོམས་བཀག་ཆ་བཀག་ཆ་འདི་ སྙན་ཆའི་སྡེ་ཚན་རིམ་སྒྲིག་ནང་ (`03-config-example.toml` ལུ་བལྟ།)
- [ ] དབང་ཚད་/འཛམ་གླིང་བསྟར་སྤྱོད་ཀྱི་བདེན་དཔང་དང་ `attestations` ཐོ་ཡིག་མི་འབོར།
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` གཡོག་བཀོལ་ཞིནམ་ལས་ ཆོག་ཐམ་/འཐུས་ཤོར་སྙན་ཞུ་འདི་བསྐྱར་ཞིབ་འབད།
- [] རི་ལེ་འཛུལ་ཞུགས་ CSR བཟོ་སྐྲུན་འབད་དེ་ གཞུང་སྐྱོང་མིང་རྟགས་བཀོད་ཡོད་པའི་ ཡིག་ཤུབས་ཚུ་ ལེན་དགོ།
- [ ] ནང་འདྲེན་སྲུང་སྐྱོབ་འགྲེལ་བཤད་ཀྱི་སོན་དང་དཔར་བསྐྲུན་འབད་ཡོད་པའི་ཧེ་ཤི་རིམ་སྒྲིག་ལུ་བདེན་དཔྱད་འབད།
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` རྒྱུགས།
- [ ] Dry-ran teltyparty ཕྱིར་གཏོང་: Prometheus གི་གཅུས་གཟེར་འདི་ས་གནས་ནང་མཐར་འཁྱོལ་ཅན་སྦེ་ངེས་གཏན་བཟོ།
- [ ] ཐོ་གཞུང་རྒྱ་སྨུག་སྦྱོང་བརྡར་སྒོ་སྒྲིག་དང་ ཡར་འཕར་གྱི་འབྲེལ་བ་ཚུ་ཐོ་བཀོད་འབད།
- [ ] དམག་སྦྱོང་སྒྲུབ་བྱེད་ཀྱི་བསྡུ་སྒྲིག་ལུ་རྟགས་བཀལ།: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.