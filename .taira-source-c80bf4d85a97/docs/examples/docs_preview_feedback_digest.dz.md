---
lang: dz
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ཡིག་ཆ་ཚུ་ དྲྭ་ཚིགས་སྔོན་ལྟ་འཆར་སྣང་འཇུ་བྱེད་ (ཊེམ་པེལེཊི་)

གཞུང་སྐྱོང་གི་དོན་ལུ་ སྔོན་ལྟ་རླབས་ཅིག་བཅུད་བསྡུས་འབད་བའི་སྐབས་ ཊེམ་པེལེཊི་འདི་ལག་ལེན་འཐབ།
བསྐྱར་ཞིབ་ཚུ་, ཡང་ན་ `status.md`. མརཀ་ཌའོན་འདི་ བརྟག་ཞིབ་ཀྱི་ཤོག་འཛིན་ནང་ལུ་འདྲ་བཤུས་རྐྱབས།
གནད་སྡུད་ངོ་མ་ཡོད་མི་ས་གནས་ཚུ་དང་ ཇེ་ཨེསི་ཨོ་ཨེན་བཅུད་བསྡུས་འདི་བརྒྱུད་དེ་ ཕྱིར་འདྲེན་འབད་ཡོདཔ་ཨིན།
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. ཚིག༌ཕྲད
I18NI0000003X གྲོགས་རམ་འབད་མི་ (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
འོག་གི་མེ་ཊིགསི་དབྱེ་ཚན་འདི་བཟོ་བཏོན་འབདཝ་ལས་ ཁྱོད་ཀྱིས་ ནང་ལུ་རྐྱངམ་ཅིག་བཀང་དགོཔ་ཨིན།
འོད་རྟགས་ཚུ་/བྱ་བ་/སྒྱུ་རྩལ་ཚུ་གིས་ གྱལ་འབདཝ་ཨིན།

```markdown
## Wave preview-<tag> feedback digest (YYYY-MM-DD)
- Invite window: <start → end>
- Reviewers invited: <count> (open: <count>)
- Feedback submissions: <count>
- Issues opened: <count>
- Latest event timestamp: <ISO8601 from summary.json>

| Category | Details | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <e.g., “ISO builder walkthrough landed well”> | <owner + due date> |
| Blocking findings | <list issue IDs or tracker links> | <owner> |
| Minor polish items | <group cosmetic or copy edits> | <owner> |
| Telemetry anomalies | <link to dashboard snapshot / probe log> | <owner> |

## Actions
1. <Action item + link + ETA>
2. <Optional second action>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

འཇུ་བྱེད་རེ་རེ་བཞིན་ མགྲོན་བརྡ་རྗེས་མའི་ ཤོག་འཛིན་དེ་ བསྐྱར་ཞིབ་དང་ གཞུང་སྐྱོང་ འབད་ཚུགས།
CI དྲན་ཐོ་ཚུ་བརྒྱུད་དེ་ ས་བརྐོ་མ་དགོ་པར་ སྒྲུབ་བྱེད་ལམ་འདི་ ལོག་གཏང་དགོ།