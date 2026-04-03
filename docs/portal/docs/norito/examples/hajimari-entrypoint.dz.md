<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8367687fcf43fcb50ab43940a4ebeb8b8ba22a3ab8a6c3ed5088c52b1fdd7baf
source_last_modified: "2026-01-22T15:38:30.521640+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/hajimari-entrypoint
title: ཧ་ཇི་མ་རི་འཛུལ་སྒོའི་རུས་སྒྲོམ།
description: མི་མང་འཛུལ་སྒོ་གཅིག་དང་ མངའ་སྡེའི་ལག་ལེན་འཐབ་མི་གཅིག་དང་གཅིག་ཁར་ ཉུང་མཐའ་ Kotodama གན་རྒྱ་བཟོ་ཐངས།
source: crates/ivm/docs/examples/01_hajimari.ko
---

མི་མང་འཛུལ་སྒོ་གཅིག་དང་ མངའ་སྡེའི་ལག་ལེན་འཐབ་མི་གཅིག་དང་གཅིག་ཁར་ ཉུང་མཐའ་ Kotodama གན་རྒྱ་བཟོ་ཐངས།

## ལེ་ཇར་འགྲུལ་བཞུད་

- [Norito འགོ་བཙུགས་ནི་](/norito/getting-started#1-compile-a-kotodama-contract) ཡང་ན་ `cargo test -p ivm developer_portal_norito_snippets_compile` བརྒྱུད་དེ་ `koto_compile --abi 1` དང་ཅིག་ཁར་ གན་རྒྱ་འདི་བསྡུ་སྒྲིག་འབད།
- མཐུད་མཚམས་ཅིག་ལུ་མ་འཆང་བའི་ཧེ་མ་ `info!` དྲན་ཐོ་དང་ འགོ་ཐོག་སིསི་ཀཱལ་བདེན་དཔྱད་འབད་ནི་ལུ་ `ivm_run` / `developer_portal_norito_snippets_run` དང་ཅིག་ཁར་ བཱའིཊི་ཀོཌི་འདི་ ཉེ་གནས་ལུ་ ཐ་མག་བརྟག་དཔྱད་འབད།
- `iroha_cli app contracts deploy` བརྒྱུད་དེ་ བརྡ་མཚོན་འདི་བཀྲམ་སྤེལ་འབད་ཞིནམ་ལས་ [Norito འགོ་བཙུགས་ནི་](/norito/getting-started#4-deploy-via-iroha_cli) ནང་གི་རིམ་པ་ཚུ་ལག་ལེན་འཐབ་སྟེ་ གསལ་བསྒྲགས་འདི་ངེས་གཏན་བཟོ།

## འབྲེལ་ཡོད་ཨེསི་ཌི་ཀེ་ལམ་སྟོན།

- [རསཊ་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/rust)
- [པའི་ཐོན་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས](/sdks/python)
- [ཇ་བ་སི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/javascript)

[Kotodama ཐོན་ཁུངས་ཕབ་ལེན་བྱོས།](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```