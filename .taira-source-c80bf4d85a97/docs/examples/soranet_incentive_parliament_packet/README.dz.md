---
lang: dz
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay སྐུལ་སློང་གྲོས་ཚོགས་ཀྱི་ Packet

འདི་ སོ་ར་སྤྱི་ཚོགས་ཀྱིས་ ཆ་འཇོག་འབད་དགོ་པའི་ ཅ་རྙིང་ཚུ་ བསྡུ་ལེན་འབདཝ་ཨིན།
རང་འགུལ་བརྒྱུད་སྤྲོད་སྤྲོད་ལེན་ (SNNet-7):

- I18NI000000002X - I18NT0000000X-རིམ་སྒྲིག་ཅན་གྱི་ གསོལ་རས་འཕྲུལ་ཆས་རིམ་སྒྲིག་ གྲ་སྒྲིག་ཡོད།
  `iroha app sorafs incentives service init` གིས་བཙུགས་ནི། ཚིག༌ཕྲད
  `budget_approval_id` གཞུང་སྐྱོང་སྐར་མ་ཚུ་ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་ ཧ་ཤི་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
- `shadow_daemon.json` - ཁེ་ཕན་དང་ བུན་གཡར་གྱི་སབ་ཁྲ་འདི་ བསྐྱར་རྩེད་ཀྱིས་སྤྱོད་ཡོདཔ།
  harness (`shadow-run`) དང་ཐོན་ལས་ཌེ་མན།
- I18NI000000007X - ༢༠༢༥-༡༠ -> ༢༠༢༥-༡༡ གི་དོན་ལུ་ དྲང་བདེན་གྱི་བཅུད་བསྡུས།
  གྱིབ་མ་བཟོ་བཀོད།
- `rollback_plan.md` - རང་བཞིན་སྤྲོད་ལེན་ཚུ་ལྕོགས་མིན་བཟོ་ནིའི་དོན་ལུ་ ལག་ལེན་གྱི་རྩེད་རིམ།
- རྒྱབ་སྐྱོར་གྱི་ཅ་ཆས་ཚུ་: I18NI0000009X,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## ཆིག་སྒྲིལ་བརྟག་དཔྱད།

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

སྤྱི་ཚོགས་ཀྱི་སྐར་མ་ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་གནས་གོང་ཚུ་དང་ག་བསྡུར་འབད། བདེན༌དཔྱད༌འབད༌ནི
གྱིབ་མ་-གཡོག་བཀོལ་བའི་མིང་རྟགས་ནང་ ནང་གསལ་བཀོད་འབད་ཡོད་དོ་བཟུམ་སྦེ་
`docs/source/soranet/reports/incentive_shadow_run.md`.

## སྒམ་དུས་དུས་མཐུན་བཟོ་དོ།

1. ཁེ་འབབ་ཀྱི་ལྗིད་ཚད་ གཞི་རྩའི་གླ་ཆ་ ཡང་ན་ ཡངན་ ཡང་ན་ བསྐྱར་གསོ་འབདཝ་ཨིན།
   ཆ་འཇོག་ཧ་ཤི་འགྱུར་བ།
2. ཉིན་གྲངས་ ༦༠ གི་གྱིབ་མ་བརྟག་དཔྱད་འདི་ལོག་སྟེ་གཡོག་བཀོལ།
   ཞིབ་འཇུག་གསརཔ་, དང་ JSON + ཕྱིར་འཐེན་འབད་ཡོད་པའི་མིང་རྟགས་ཆ་གཅིག་འབད་ནི།
༣ དུས་མཐུན་བཟོ་ཡོད་པའི་ བཱན་ཌལ་འདི་ སྤྱི་ཚོགས་ནང་ བལྟ་བརྟོག་འབད་སའི་ བརྡ་རྟགས་བཀོད་ཁྲམ་དང་གཅིག་ཁར་ ཕུལ་ནི།
   ཆ་འཇོག་གསརཔ་འཚོལ་བའི་སྐབས་ཕྱིར་གཏོང་།