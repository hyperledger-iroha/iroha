---
lang: dz
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii རིམ་སྒྲིག་མཐུད།

Iroha Torii གིས་ གདམ་ཁ་ཅན་གྱི་ ཝ་ལེཊི་ཀོན་ནེཀཊི་-བཟོ་རྣམ་གྱི་མཐའ་མཇུག་ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
`connect` ཀར་གོ་ཁྱད་རྣམ་འདི་ ལྕོགས་ཅན་བཟོ་བའི་སྐབས། (སྔོན་སྒྲིག་) རན་ཊའིམ་སྤྱོད་ལམ་འདི་ རིམ་སྒྲིག་ལུ་ gated::

- མཐུད་ལམ་འགྲུལ་ལམ་ཆ་མཉམ་ལྕོགས་མིན་བཟོ་ནི་ལུ་ `connect.enabled=false` གཞི་སྒྲིག་འབད། (I18NI000000005X)
- ཌབ་ལུ་ཨེསི་ལཱ་ཡུན་མཇུག་བསྡུ་ཚུ་ལྕོགས་ཅན་བཟོ་ནི་ལུ་ I18NI000000006X (སྔོན་སྒྲིག་) བཞག་ཞིནམ་ལས་ I18NI000000007X.

ཁོར་ཡུག་གི་ཚབ་མ་ཚུ་ (ལག་ལེན་པ་རིམ་སྒྲིག་ → ངོ་མའི་རིམ་སྒྲིག་):

- `CONNECT_ENABLED` (bool; སྔོན་སྒྲིག་: I18NI0000009X)
- `CONNECT_WS_MAX_SESSIONS` (ལག་ལེན་; སྔོན་སྒྲིག་: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (ལག་ལེན་; སྔོན་སྒྲིག་: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; སྔོན་སྒྲིག་: I18NI0000015X)
- `CONNECT_FRAME_MAX_BYTES` (ལག་ལེན་; སྔོན་སྒྲིག་: I18NI0000017X)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (ལག་ལེན་; སྔོན་སྒྲིག་: I18NI0000019X)
- I18NI000000020X (དུས་ཡུན་ སྔོན་སྒྲིག་: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; སྔོན་སྒྲིག་: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (དུས་ཡུན་ སྔོན་སྒྲིག་: I18NI0000025X)
- `CONNECT_DEDUPE_CAP` (ལག་ལེན་; སྔོན་སྒྲིག་: I18NI0000027X)
- I18NI0000028X (bool; སྔོན་སྒྲིག་: `true`)
- `CONNECT_RELAY_STRATEGY` (ཡིག་རྒྱུན་; སྔོན་སྒྲིག་: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; སྔོན་སྒྲིག་: `0`)

དྲན་ཐོ།

- ལག་ལེན་རིམ་སྒྲིག་དང་ `CONNECT_SESSION_TTL_MS` དང་ `CONNECT_DEDUPE_TTL_MS` ལག་ལེན་ཡུན་རིང་ དང་།
  I18NI000000036X དང་ I18NI000000037X ས་སྒོ་ཚུ་ལུ་ སབ་ཁྲ་བཟོཝ་ཨིན།
- I18NI000000038X གིས་ ཨའི་ཨའི་པི་ ལཱ་ཡུན་ཀེཔ་ ལྕོགས་མིན་བཟོཝ་ཨིན།
- I18NI000000039X གིས་ ལག་ཐོག་འཇབ་རྒོལ་ཚད་གཞི་ཚད་འཛིན་འབད་མི་འདི་ ལྕོགས་མིན་བཟོཝ་ཨིན།
- Heartbeat གིས་ བརྡ་འཚོལ་མཐུན་སྒྲིག་ཅན་གྱི་ཉུང་མཐའ་ (`ping_min_interval_ms`) ལུ་ རིམ་སྒྲིག་འབད་ཡོད་པའི་བར་མཚམས་འདི་ བསྡམ་བཞགཔ་ཨིན།
  སར་བར་གྱིས་ ཝེབ་སོ་ཀེཊི་དང་ སྒོ་མ་ཁ་བསྡམས་པའི་ཧེ་མ་ `ping_miss_tolerance` རིམ་མཐུན་སྦེ་ པོན་ཚུ་ བརླག་སྟོར་ཞུགས་ཡོདཔ་ཨིན།
  `connect.ping_miss_total` མེ་ཊིག་ཡར་སེང་འབདཝ་ཨིན།
- རན་ཊའིམ་ (I18NI0000004X) ལུ་ ལྕོགས་མིན་བཟོ་བའི་སྐབས་ ཌབ་ལུ་ཨེསི་དང་ གནས་ཚད་ཀྱི་ལམ་ཚུ་མེན།
  ཐོ་བཀོད་འབད་ཡོདཔ། ཞུ་བ་ `/v2/connect/ws` དང་ `/v2/connect/status` ལུ་ 404 སླར་ལོག་འབདཝ་ཨིན།
- སར་བར་འདི་ལུ་ `/v2/connect/session` (base64url ཡང་ན་ hex, 32 bytes) གི་དོན་ལུ་ མཁོ་སྤྲོད་འབད་མི་ `sid` དགོཔ་ཨིན།
  དེ་གིས་ ད་ལས་ཕར་ `sid` འདི་ བཏོན་མི་ཚུགས།

ཡང་བལྟ།: I18NI000000049X དང་ ནང་ སྔོན་སྒྲིག་འབད།
`crates/iroha_config/src/parameters/defaults.rs` (མོ་ཌལ་ I18NI000000051X).