---
lang: dz
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55e3fee1f354fa023ec258958b25c01ebda75a35bafdaa049a82f3ca73dab017
source_last_modified: "2026-01-22T16:26:46.513338+00:00"
translation_last_reviewed: 2026-02-07
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
slug: /sdks/recipes/python-ledger-flow
translator: machine-google-reviewed
---

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

འདིའི་པའེ་ཐོན་གྱི་ པར་ཆས་འདི་ [CLI ledger watchthrough ](../../norito/ledger-walkthrough.md)
དང་ [Rust བཟའ་ཐངས](I18NU0000006X). འདི་གིས་ སྔོན་སྒྲིག་ I18NT0000002X ལག་ལེན་འཐབ་ཨིན།
ཡོངས་འབྲེལ་དང་ I18NI0000007X ནང་ བརྩམ་ཡོད་པའི་ བརྡ་སྟོན་ཡིག་ཆ་ཚུ་ བརྩམ་དགོ།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལེན་ཚུ་/python/ledger_flow.py"
  filena="ledger_flow.py"
  secont="ལགཔ་གིས་ཨང་རྟགས་མེད་པར་ གཡོག་བཀོལ་ནིའི་དོན་ལུ་ ཐབས་གཞི་འདི་ནང་སྟོན་ཡོད་པའི་ཡིག་གཟུགས་ཕབ་ལེན་འབད།"
།/>།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

```bash
pip install iroha-python
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## དཔེར་ཡིག་།

I18NF0000004X

`python ledger_flow.py` དང་ཅིག་ཁར་གཡོག་བཀོལ། ཐོན་འབྲས་འདི་གིས་ ཚོང་འབྲེལ་ཧེཤ་སྙན་ཞུ་འབད་དགོ།
(ཐོབ་པའི་དངུལ་འབབ་ལས་) དེ་གི་ཤུལ་ལས་ ལེན་མི་གསརཔ་གི་ལྷག་ལུས་འདི་ཨིན། རྒྱུ་དངོས་ངེས་ཚིག་འདི་ཧེ་མ་ལས་རང་ཡོད་པ་ཅིན།
ཐོ་བཀོད་བཀོད་རྒྱ་འདི་ མིན་ཊི་/བརྗེ་སོར་འདི་ འཕྲོ་མཐུད་དེ་རང་ མཐར་འཁྱོལ་འབྱུང་པའི་སྐབས་ ངོས་ལེན་མ་འབད་བར་ཡོདཔ་ཨིན།

## ཆ་འཇོག་འབད་ནི།

སི་ཨེལ་ཨའི་བརྡ་བཀོད་དེ་ Norito གི་ འགྲུལ་བསྐྱོད་ལས་ ལག་ལེན་འཐབ་སྟེ་ ཧེསི་ཚུ་ བརྒལ་ཡོདཔ་ཨིན།
དང༌རིམ༌ ཁྱོད་ཀྱིས་ ཇ་བ་ཨིསི་ཀིརིཔཊི་དང་ རསཊི་བཟོ་ཐངས་ཚུ་ གཡོག་བཀོལ་བའི་སྐབས་ ཨེསི་ཌི་ཀེ་གསུམ་ཆ་ར་ ཆ་མཉམ་དགོ།
བརྗེ་སོར་གྱི་ཧ་ཤེ་དང་ བགོ་བཤའ་རྐྱབ་པའི་རྒྱུན་འགྲུལ་གྱི་དོན་ལུ་ I18NT0000001X གླ་ཆ་ཚུ་ ཆ་འཇོག་འབད་ནི།