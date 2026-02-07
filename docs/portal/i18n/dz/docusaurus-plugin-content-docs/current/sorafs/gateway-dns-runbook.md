---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS སྒོ་དང་ཌི་ཨེན་ཨེསི་ཀིཀ་ཨོཕ་རཱན་བུཀ།

དྲྭ་ཚིགས་འདི་གི་འདྲ་བཤུས་འདི་གིས་ ༢༠ ནང་ ཀེ་ནོ་ནིག་རཱན་བུཀ་འདི་ མེ་ལོང་ནང་ གསལ་སྟོན་འབདཝ་ཨིན།
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
འདི་གིས་ དབང་ཚད་མར་ཕབ་འབད་ཡོད་པའི་ ཌི་ཨེན་ཨེསི་ དང་ གཱེཊ་ཝེ་གི་དོན་ལུ་ ལག་ལེན་འཐབ་པའི་ བཀག་འཛིན་ཚུ་ འཛིན་བཟུང་འབདཝ་ཨིན།
ལས་ཀ་འདི་ དེ་འབདཝ་ལས་ ཡོངས་འབྲེལ་དང་ ops དེ་ལས་ ཡིག་ཆ་ཚུ་གི་འགོ་ཁྲིད་ཚུ་གིས་ བསྐྱར་སྦྱོང་འབད་ཚུགས།
2025‐03 གི་ ཀིག་ཨོཕ་གི་གདོང་ཁར་ རང་བཞིན་བང་སྒྲིག་འབད་ཡོདཔ།

## ཁྱབ་ཁོངས།

- ཌི་ཨེན་ཨེསི་ (SF‐4) དང་ འཛུལ་སྒོ་ (SF‐‐15) གཉིས་ བསྐྱར་སྦྱོང་འབད་བའི་ གཏན་འབེབས་བཟོ་ཐོག་ལས་ བསྡམས་དགོ།
  ཧོསཊི་ལས་བཞེངས་ནི་དང་ སེལ་བྱེད་སྣོད་ཐོ་གསར་བཏོན་འབད་ནི་ ཊི་ཨེལ་ཨེསི་/ཇི་ཨར་རང་བཞིན་དང་ སྒྲུབ་བྱེད་ཚུ།
  བཙན༌བཟུང།
- ཀིག་ཨོཕ་ཨིན་པུཊ་ (གྲོས་གཞི་, མགྲོན་འབོད་ བཅའ་མར་གཏོགས་མི་རྗེས་འདེད་པ།
  snapshot) ཇོ་བདག་ལས་འགན་གསརཔ་དང་མཉམ་འབྱུང་འབད་ཡོདཔ།
- གཞུང་སྐྱོང་བསྐྱར་ཞིབ་འབད་མི་ཚུ་གི་དོན་ལུ་ རྩིས་ཞིབ་འབད་བཏུབ་པའི་ ཅ་རྙིང་གི་བསྡུ་སྒྲིག་ཅིག་བཟོ་ནི།: སེལ་མཁན།
  སྣོད་ཐོ་གསར་བཏོན་འབད་ཡོད་པའི་དྲན་ཐོ་དང་ སྒོ་སྒྲིག་འཚོལ་ཞིབ་དྲན་ཐོ་ མཐུན་སྒྲིག་འཕྲུལ་ཆས་ཨའུཊི་པུཊི་དང་།
  the ཡིག་ཆ་/ཌི་ཝི་རེལ་བཅུད་དོན།

## འགན་ཁུར་དང་འགན་ཁུར།

| ལས་རིམ། | འགན་འཁྲིའི་འགན་ཁུར། | དགོས་མཁོའི་དངོས་པོ་ཚུ། |
|------------|------------------|--------------------|
| ཡོངས་འབྲེལ་ TL (DNS stack) | གཏན་འབེབས་བཟོ་ནིའི་འཆར་གཞི་རྒྱུན་སྐྱོང་འབད་ནི་དང་ RAD སྣོད་ཐོ་གསར་བཏོན་ཚུ་གཡོག་བཀོལ་ནི་ སེལ་བྱེད་བརྒྱུད་འཕྲིན་ཨིན་པུཊི་ཚུ་དཔར་བསྐྲུན་འབད། | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md`, RAD མེ་ཊ་ཌེ་ཊ་གི་དོན་ལུ་ ཁྱད་པར་ཚུ། |
| Ops འཕྲུལ་ཆས་འགོ་ཁྲིད་ (སྒོ་ལམ) | TLS/ECH/GAR རང་བཞིན་སྦྱོང་བརྡར་ཚུ་ བཀོལ་སྤྱོད་, `sorafs-gateway-probe` གཡོག་བཀོལ་, དུས་མཐུན་བཟོ་ནི་ PagerDuty hooks. | `artifacts/sorafs_gateway_probe/<ts>/`, འཚོལ་ཞིབ་ JSON, `ops/drill-log.md` ཐོ་བཀོད་ཚུ། |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh`, གུག་ཤད་སྒྲིག་ཆས་, གཏན་མཛོད་ Norito རང་དོན་ལག་ཁྱེར་གྱི་བང་རིམ། | `artifacts/sorafs_gateway_conformance/<ts>/`, Grafana. |
| ཡིག་ཆ་ / ཌེབ་རེལ | སྐར་མ་ཚུ་ མཐུད་དེ་ བཟོ་བཀོད་ཀྱི་ སྔོན་སྒྲིག་ + ཟུར་དེབ་ཚུ་ དུས་མཐུན་བཟོ་ཞིནམ་ལས་ དྲྭ་ཚིགས་འདི་ནང་ སྒྲུབ་བྱེད་བཅུད་བསྡུས་ཚུ་ དཔར་བསྐྲུན་འབད། | ཡིག་སྣོད་ཚུ་དང་ བསྐོར་བའི་དྲན་འཛིན་ཚུ་ དུས་མཐུན་བཟོ་ཡོདཔ། |

## ནང་འཇུག་དང་སྔོན་སྒྲིག་ཆ་རྐྱེན།

- གཏན་འཁེལ་སི་ཊིག་ཧོསིཊི་སི་ཊཱོན་ (`docs/source/soradns/deterministic_hosts.md`) དང་
  ཐག་གཅོད་བྱེད་པའི་བདེན་དཔང་ ལྕགས་ཐག་ (`docs/source/soradns/resolver_attestation_directory.md`).
- གཱེཊི་ཝེ་ཅ་རྙིང་ཚུ་: བཀོལ་སྤྱོད་པའི་ལག་དེབ།
  ཐད་ཀར་གྱི་ལམ་སྟོན་དང་ `docs/source/sorafs_gateway_*` འོག་ལུ་ རང་གིས་རང་ ལག་ཁྱེར་གྱི་ལཱ་འབད་ཐངས་འདི་ཨིན།
- ལག་ཆས་: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, དང་ CI རོགས་རམ་པ།
  (I 18NI0000029X, `ci/check_sorafs_gateway_probe.sh`).
- གསང་བ་: ཇི་ཨར་ གསར་བཏོན་ལྡེ་མིག ཌི་ཨེན་ཨེསི་/ཊི་ཨེལ་ཨེསི་ཨེ་སི་ཨེམ་ ཡིག་ཆ་ པེ་གར་ཌུ་ཊི་ འགྲུལ་ལམ་ལྡེ་མིག་,
  Torii ཐག་གཅོད་འབད་མི་ཚུ་གི་དོན་ལུ་ བདེན་བཤད་ཀྱི་རྟགས་མཚན་།

## སྔོན་གྱི་འཕུར་སྐྱོད་དཔྱད་གཞི།

༡ དུས་མཐུན་བཟོ་སྟེ་ བཅའ་མར་གཏོགས་མི་དང་ གྲོས་གཞི་ཚུ་ བདེན་དཔྱད་འབད་ནི།
   `docs/source/sorafs_gateway_dns_design_attendance.md` དང་བསྐོར་སྐྱོད་འབད་ནི།
   ད་ལྟའི་སྒྲ་ (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. རིམ་པ་ཅན་གྱི་ཅ་རྙིང་རྩ་བཟུམ།
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` དང་།
   `artifacts/soradns_directory/<YYYYMMDD>/`.
༣ གསར་བསྐྲུན་གྱི་སྒྲིག་བཀོད་ (GAR མངོན་གསལ་དང་ RAD བདེན་དཔང་། འཛུལ་སྒོ་སྒྲིག་བཀོད་བང་རིམ།) དང་།
   `git submodule` མངའ་སྡེ་གིས་ བསྐྱར་སྦྱོང་གི་ངོ་རྟགས་གསརཔ་འདི་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
༤ གསང་བ་ཚུ་བདེན་དཔྱད་འབད་ (Ed25519 བཏོན་གཏང་ལྡེ་མིག་, ACME རྩིས་ཐོ་ཡིག་སྣོད་, PagerDuty བརྡ་མཚོན་) ཚུ་ཨིན།
   ཕུལཝ་དང་ བརྡ་རྟགས་བརྟག་དཔྱད་ཚུ་མཐུན་སྒྲིག་འབད།
༥ ཐ་མག་བརྟག་དཔྱད་ཀྱི་དམིགས་ཚད་ (Pushteway མཐའ་མཚམས་, GAR Grafana board) སྔོན་འགྲོ།
   དམག་སྦྱོང་ལུ།

## རང་འགུལ་སྦྱོང་བརྡར།

### གཏན་འབེབས་ཧོསཊི་ས་ཁྲ་དང་ཨར་ཨེ་ཌི་སྣོད་ཐོ་གསར་བཏོན་འབད།༡ གྲོས་འཆར་ཕུལ་མི་ གསལ་སྟོན་ལུ་ གཏན་འབེབས་བཟོ་མི་ འབྱུང་ཁུངས་གྲོགས་རམ་པ་ གཡོག་བཀོལ།
   set དང་ ལས་ ཌིཕཊི་མེདཔ་ ངེས་གཏན་བཟོ།
   `docs/source/soradns/deterministic_hosts.md`.
2. ཐག་གཅོད་སྣོད་ཐོའི་བང་སྒྲིག་ཅིག་བསྐྲུན།

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

༣ དཔར་བསྐྲུན་འབད་ཡོད་པའི་སྣོད་ཐོ་ ID, SHA-256, དང་ ཐོན་འབྲས་འགྲུལ་ལམ་ཚུ་ ནང་འཁོད་ལུ་ཐོ་བཀོད་འབད།
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` དང་ འགོ་འབྱེད་པ།
   གྲོས་ཆོད།

### DNS ཊེ་ལི་མི་ཊི་རི་བཟུང་།

- མཇུག་བསྡུའི་ཐབས་ཤེས་དྭངས་གསལ་དྲན་ཐོ་ཚུ་ ≥10 སྐར་མའི་དོན་ལུ་ལག་ལེན་འཐབ་ཡོདཔ།
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- ཕུང་པོ་དང་མཉམ་པའི་ NDJSON པར་ལེན་ཚུ་ Pushgateway metrics དང་ ཡིག་མཛོད་ཕྱིར་འདྲེན་འབད།
  ID སྣོད་ཐོ།

### སྒོ་ཁའི་རང་འགུལ་དོ་དམ།

༡ TLS/ECH འཚོལ་ཞིབ་ལག་ལེན་འཐབ།

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. འཁྲིལ་བའི་འཆམ་མཐུན་ (`ci/check_sorafs_gateway_conformance.sh`) དང་།
   རང་གིས་རང་ལུ་ཕན་ཐོགས་པའི་གྲོགས་རམ་པ་ (`scripts/sorafs_gateway_self_cert.sh`) གསར་བསྐྲུན་འབད་ནིའི་དོན་ལུ་
   Norito བདེན་དཔང་བྱེད་པའི་བང་རིམ།
3. རང་བཞིན་ལས་ཀ་མཇུག་བསྡུ་བའི་བདེན་དཔང་འབད་ནིའི་དོན་ལུ་ PagerDuty/Webhook བྱུང་ལས་ཚུ་ བཟུང་དགོ།
   མཇགུ་བསྡུ།

### བདེན་དཔང་ཐུམ་སྒྲིལ་།

- དུས་ཚོད་མཚོན་རྟགས་དང་ བཅའ་མར་གཏོགས་མི་ དེ་ལས་ འཚོལ་ཞིབ་ཧེ་ཤེ་ཚུ་དང་གཅིག་ཁར་ `ops/drill-log.md` དུས་མཐུན་བཟོ་ནི།
- གཡོག་བཀོལ་བའི་ཨའི་ཌི་སྣོད་ཐོ་ཚུ་གི་འོག་ལུ་ ཅ་རྙིང་ཚུ་གསོག་འཇོག་འབད་ཞིནམ་ལས་ བཀོད་ཁྱབ་བཅུད་བསྡུས་ཅིག་དཔར་བསྐྲུན་འབད།
  ཡིག་ཆ་/DevRel ཞལ་འཛོམས་ཀྱི་སྐར་མ་ནང་།
- འགོ་འབྱེད་མ་འབད་བའི་ཧེ་མར་ གཞུང་སྐྱོང་གི་ཤོག་འཛིན་ནང་ སྒྲུབ་བྱེད་ཚུ་ འབྲེལ་མཐུད་འབད།

## ཚོགས་འདུའི་མཐུན་རྐྱེན་དང་ སྒྲུབ་བྱེད་ལག་གདུབ།

- **བར་མཚམས་དུས་ཚོད་ལས:**  
  - T‐T‐24h — ལས་རིམ་འཛིན་སྐྱོང་གིས་ `#nexus-steering` ནང་ དྲན་སྐུལ་+ གྲོས་གཞི་/ལས་སྣ་པར་ལེན་ཚུ་ བརྡ་བསྐུལ་འབདཝ་ཨིན།  
  - T‐T‐2h — ཡོངས་འབྲེལ་ནང་ TL གིས་ GAR ཊེ་ལི་མི་ཊི་པར་ལེན་འབད་དེ་ `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ནང་ ཌེལ་ཊ་ཚུ་ ཐོ་བཀོད་འབདཝ་ཨིན།  
  - T‐15m — Ops Automation གིས་ འཚོལ་ཞིབ་གྲ་སྒྲིག་བདེན་དཔྱད་འབད་ཞིནམ་ལས་ ཤུགས་ལྡན་གཡོག་བཀོལ་བའི་ ID འདི་ `artifacts/sorafs_gateway_dns/current` ནང་ལུ་བྲིས།  
  - ཁ་པར་གྱི་སྐབས་ལུ་ — བཀོད་སྒྲིག་འབད་མི་གིས་ རན་དེབ་འདི་བགོ་བཤའ་རྐྱབ་སྟེ་ ཐད་རི་བ་རི་ཡིག་དཔར་རྐྱབ་ཨིན། Docs/DevRel གིས་ བྱ་བའི་རྣམ་གྲངས་ཚུ་ ནང་ན་འཛིན་བཟུང་འབདཝ་ཨིན།
- **དཀར་ཆག་ཊེམ་པེལེཊི་:** ཀེང་རུས་འདི་ལས་ འདྲ་བཤུས་རྐྱབས།
  `docs/source/sorafs_gateway_dns_design_minutes.md` (དེ་བཞིན་དྲྭ་ཚིགས་ནང་དུ་མེ་ལོང་།
  bundle) དང་ ལཱ་ཡུན་རེ་ལུ་ དཔེ་ཚད་གཅིག་བཀང་དགོ། བཅའ་མར་གཏོགས་མི་ཐོ་ཡིག་ཚུ་ཚུདཔ་ཨིན།
  ཐག་གཅོད་དང་ བྱ་བའི་རྣམ་གྲངས་ སྒྲུབ་བྱེད་ཀྱི་ཧ་ཤེ་ དེ་ལས་ ཉེན་ཁ་མེད་པའི་ ཁྱད་འཕགས་ཚུ།
- **བདེན་དཔང་ཡར་འཕེལ་:** བསྐྱར་སྦྱོང་ལས་ `runbook_bundle/` སྣོད་ཐོ་ ཟིཔ་ .
  སྐར་མ་ + གྲོས་གཞི་ནང་ SHA-256 ཧེ་ཤེ་ཚུ་ ཐོ་བཀོད་འབད་ཡོད་པའི་སྐར་མ་ཚུ་ མཉམ་སྦྲགས་འབད།
  དེ་ལས་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་པ་ མིང་གཞན་གྱིས་ ཚར་གཅིག་ ས་ཆ་ སྐྱེལ་བཙུགས་འབདཝ་ཨིན།
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## སྒྲུབ་བྱེད་ཀྱི་པར་རིས། (༢༠༢༥ ཟླ་ ༣ ཚེས་ ༡༠)

གསར་ཤོས་སྦྱོང་བརྡར་/དངོས་གྲུབ་ཀྱི་ཅ་རྙིང་ཚུ་ ས་ཁྲ་དང་ གཞུང་སྐྱོང་ནང་ གཞི་བཞག་སྟེ་ཡོདཔ་ཨིན།
སྐར་མའི་འོག་ལུ་ `s3://sora-governance/sorafs/gateway_dns/` བཱ་ཀེཊ། ཧམ་སམ།
འོག་ལུ་ ཁྲིམས་ལུགས་ཀྱི་གསལ་སྟོན་ (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) ཨིན།

- **སྐམ་བང་རྒྱུག་ — ༢༠༢༥-༠༣-༠༢ (`artifacts/sorafs_gateway_dns/20250302/`)**
  - བཱན་ཌལ་ཊར་བཱོལ་: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - སྐར་མ PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ལས་རིམ། — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - SoraFS
  - Torii
  - ```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```
  - ```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(སྐྱེལ་བཙུགས་འབད་ནི།: `gateway_dns_minutes_20250303.pdf` — ཌོཀ་/ཌི་ཝི་རེལ་གྱིས་ བཱན་ཌལ་ནང་ པི་ཌི་ཨེཕ་ས་ཆ་ཚུ་ བཀྲམ་སྟོན་འབད་ཚར་བའི་ཤུལ་ལས་ ཨེསི་ཨེཆ་ཨེ་-༢༥༦ འདི་ མཐུད་འོང་།)_

## འབྲེལ་བའི་རྒྱུ་ཆ།

- [གེ་ཊི་ཝེ་བཀོལ་སྤྱོད་དེབ་ཨང་](./operations-playbook.md)
- [SoraFS བལྟ་རྟོག་འཆར་གཞི་](./observability-plan.md)
- [བཀག་ཆ་འབད་ཡོད་པའི་ ཌི་ཨེན་ཨེསི་ དང་ གཱེཊ་ཝེ་རྗེས་འདེད་པ།](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)