---
lang: dz
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# ལྷན་ཁག་དམར་པོའི་གནས་རིམ།

ཤོག་ངོས་འདིའི་ [moderation_red_team_plan.md) [moderation_red_team_plan.md)
ཉེ་འདབས་ཀྱི་སྦྱོང་བརྡར་ཟླ་ཐོ་དང་ སྒྲུབ་བྱེད་བསྡོམ་ དེ་ལས་ བཅོ་ཁ་ཚུ་ བརྟག་ཞིབ་འབད་ཐོག་ལས་ཨིན།
གནས་ཚད། འོག་ལུ་བཀོད་ཡོད་པའི་ ཅ་རྙིང་ཚུ་དང་གཅིག་ཁར་ རྒྱུག་པའི་ཤུལ་ལས་ དུས་མཐུན་བཟོ་དགོ།
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## འདོད་པ་ ཌིརིལ་།

| ཚེས་གྲངས་ (ཡུ་ཊི་སི) | པར་སྐྲུན། | ཇོ་བདག་(ཚུ་) | སྒྲུབ་བྱེད་ Prep | དྲན་ཐོ། |
|------------|---------|----------|---------------|-------|
| ༢༠༢༦-༡༡-༡༢ | **བཀོལ་སྤྱོད་ མིག་ཟུང་** — ཊའི་ཀཱའི་ སླ་བསྲེ་བའི་ཐབས་ལམ་ ནག་ཚོང་འཐབ་ཐངས་ འཛུལ་སྒོ་ མར་ཕབ་ཀྱི་ དཔའ་བཅམ་མི་ཚུ་ | བདེ་འཇགས་བཟོ་རིག་ (མི་ཡུ་སཊོ་) ལྷན་ཁག་ཨོཔ་ (ལི་ཡམ་ཨོ་ཀོ་ནར་) | `scripts/ministry/scaffold_red_team_drill.py` བཱན་ཌལ་ `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + གནས་རིམ་གྱི་སྣོད་ཐོ་ `artifacts/ministry/red-team/2026-11/operation-blindfold/` | ལུས་སྦྱོང་ GAR/Taikai གཅིག་སྒྲིལ་དང་ DNS འཐུས་ཤོར་; འགོ་མ་བཙུགས་པའི་ཧེ་མ་ Merkle shat དང་ dashboards ཚུ་ བཏོན་ཚར་བའི་ཤུལ་ལས་ `export_red_team_evidence.py` གཡོག་བཀོལ་དགོ། |

## མཇུག་གི་དྲིལ་པར་ལེན་པ།

| ཚེས་གྲངས་ (ཡུ་ཊི་སི) | པར་སྐྲུན། | སྒྲུབ་བྱེད་ཀྱི་བང་རིམ་ | བཅོ་ཁ་དང་ རྗེས་འཇུག་-ཨཔ་ |
|------------|---------|-----------------|--------------------------|
| ༢༠༢༦-༠༨-༡༨ | **བཀོལ་སྤྱོད་མཚོ་ཤེལ་*** — འཛུལ་སྒོ་ནག་ཚོང་དང་ གཞུང་སྐྱོང་བསྐྱར་རྩེད་ དེ་ལས་ ཉེན་བརྡའི་རྒྱ་སྨུག་བསྐྱར་སྦྱོང་། | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana ཕྱིར་འདྲེན་, དྲན་སྐུལ་འཕྲུལ་ཆས་དྲན་ཐོ་ `seaglass_evidence_manifest.json`) | **Open:** replay seal འཕྲུལ་ཆས་ (`MINFO-RT-17`, ཇོ་བདག་: གཞུང་སྐྱོང་ Ops, tamp 2026-09-05); SoraFS (`MINFO-RT-18`, བལྟ་རྟོག་འབད་ཚུགསཔ་, གི་དོན་ལུ་ ༢༠༢༦-༠༨-༢༥) པིན། **ཁ་བསྡམས་ཡོདཔ་:** I1NT00000001X གསལ་སྟོན་གྱི་ཧ་ཤེ་ཚུ་འབག་ནིའི་དོན་ལུ་ དུས་མཐུན་བཟོ་ཡོད་པའི་ དྲན་ཐོ་བུག་ཊེམ་པེལེཊི། |

## རྗེས་སོར་དང་ལག་ཆས།

- སྨན་ཁབ་བཙུགས་བཏུབ་པའི་ཐུམ་སྒྲིལ་འབད་ནི་ལུ་ `scripts/ministry/moderation_payload_tool.py` ལག་ལེན་འཐབ།
  མཐོང་སྣང་རེ་ལུ་ papproods དང་ ཐོ་ཡིག་ཆ་མེད་གཏང་ནི།
- ཌེཤ་བོརཌི་/དྲན་ཐོ་ཚུ་ `scripts/ministry/export_red_team_evidence.py` བརྒྱུད་དེ་ འཛིན་བཟུང་འབདཝ་ཨིན།
  སྦྱོང་བརྡར་རེ་རེ་གི་ཤུལ་ལས་ དེ་འཕྲོ་ལས་ སྒྲུབ་བྱེད་མངོན་གསལ་ནང་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ ཧ་ཤེ་ཚུ་ཡོདཔ་ཨིན།
- CI ཉེན་རྟོག་པ་ `ci/check_ministry_red_team.sh` དམག་སྦྱོང་འབད་མི་ དམག་སྦྱོང་གི་སྙན་ཞུ་ཚུ་འབད་ཡོདཔ་ཨིན།
  ས་གནས་འཛིན་མི་ཚིག་ཡིག་མེདཔ་ལས་ གཞི་བསྟུན་འབད་ཡོད་པའི་ཅ་རྙིང་ཚུ་ ཧེ་མ་ལས་ཡོདཔ་ཨིན།
  མཉམ་བསྡོམས་འབད་དོ།

ཐད་རི་བ་རི་ བཅུད་བསྡུས་གཞི་བསྟུན་འབད་ནིའི་དོན་ལུ་ `status.md` (§ *ལྷན་ཁག་དམརཔོ་སྡེ་ཚན་གནས་རིམ་*) བལྟ།
in བདུན་ཕྲག་མཉམ་འབྲེལ་འབོད་བརྡ་ཚུ།