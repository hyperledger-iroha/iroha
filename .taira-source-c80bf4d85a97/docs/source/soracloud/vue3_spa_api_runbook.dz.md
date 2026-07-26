<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# སོ་རཀ་ལའུཌ་ཝུ་༣ ཨེསི་པི་ཨེ་ + ཨེ་པི་ཨའི་ རན་བུཀ།

འ་ནི་རན་བུཀ་འདི་གིས་ ཐོན་སྐྱེད་ལུ་གཞི་བཞག་པའི་བཀྲམ་སྤེལ་དང་ བཀོལ་སྤྱོད་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན།

- a Vue3 གནས་སྟངས་ས་ཁོངས་ (`--template site`); དང་ །
- a Vue3 SPA + ཨེ་པི་ཨའི་ཞབས་ཏོག་ (`--template webapp`),

ཨེསི་སི་ཨར་/ཨའི་༡༨ཨེན་ཊི་༠༠༠༠༠༠༠༠༣ཨེགསི་ བསམ་ཚུལ་ཚུ་དང་གཅིག་ཁར་ Iroha 3 གུ་ སོ་རཀ་ལའུཌ་ཚད་འཛིན་-ས་ཁོངས་ཨེ་པི་ཨའི་ཨེསི་ཚུ་ལག་ལེན་འཐབ་སྟེ་ (མེན།
WASM རན་ཊའིམ་བརྟེན་ས་དང་ Docker བརྟེན་ས་མེད་པ།)།

## 1. ཊེམ་པེལེཊི་ལས་འགུལ་ཚུ་བཟོ་བཏོན་འབད།

གནས་སྟངས་ཅན་གྱི་ས་ཁོངས་ཀྱི་ གྱང་ཁོག་:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

ཨེསི་པི་ཨེ་ + ཨེ་པི་ཨའི་ གྱང་ཁོག་:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

ཐོན་འབྲས་སྣོད་ཐོ་རེ་རེ་ནང་ཚུདཔ་ཨིན།

- ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༡༩X
- ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༢༠X
- ཊེམ་པེལེཊི་འབྱུང་ཁུངས་ཡིག་སྣོད་ཚུ་ `site/` ཡང་ན་ `webapp/` འོག་ལུ།

## 2. གློག་རིམ་གྱི་དངོས་པོ་བཟོ་བསྐྲུན་འབད།

གནས་སྟངས་ས་ཁོངས་:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

ཨེསི་པི་ཨེ་གདོང་ཕྱོགས་ + ཨེ་པི་ཨའི་:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. གདོང་ཕྱོགས་རྒྱུ་དངོས་ཚུ་ཐུམ་སྒྲིལ་འབད་དེ་དཔར་བསྐྲུན་འབད།

SoraFS བརྒྱུད་དེ་ གནས་སྟངས་གཙོ་བོའི་དོན་ལུ་:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

ཨེསི་པི་ཨེ་གདོང་ཕྱོགས་ཀྱི་དོན་ལུ་:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. སོ་རཀ་ལཱའུཌ་ཚད་འཛིན་གནམ་གྲུ་ནང་ལུ་ བཀྲམ་སྤེལ་འབད།

གནས་སྟངས་ས་ཁོངས་ཞབས་ཏོག་བཀྲམ་སྤེལ་འབད།

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

ཨེསི་པི་ཨེ་ + ཨེ་པི་ཨའི་ཞབས་ཏོག་བཀྲམ་སྤེལ་འབད།

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

འགྲུལ་ལམ་བསྡམ་བཞག་དང་ བཤུད་བརྙན་གནས་སྟངས་འདི་ བདེན་དཔྱད་འབད།

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

རེ་བ་བསྐྱེད་ཡོད་པའི་ཚད་འཛིན་-གནམ་གྲུའི་ཞིབ་དཔྱད་ཚུ་:

- `control_plane.services[].latest_revision.route_host` ཆ་ཚན།
- `control_plane.services[].latest_revision.route_path_prefix` ཆ་ཚན་ (`/` ཡང་ན་ `/api`)
- `control_plane.services[].active_rollout` ཡར་རྒྱས་བཏང་རྗེས་འཕྲལ་མར་ཡོད།

## 5. གསོ་བའི་སྒོ་སྒྲིག་ཅན་གྱི་བཀྲམ་སྤེལ་དང་གཅིག་ཁར་ཡར་འཕར་འབད།

1. ཞབས་ཏོག་གསལ་བསྒྲགས་ནང་ལུ་ `service_version` བམཔ་འབད།
2. ཡར་འཕར་གཡོག་བཀོལ།

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

༣ གསོ་བའི་བརྟག་དཔྱད་ཀྱི་ཤུལ་ལས་ ཁྱབ་སྤེལ་འབད་ནི།

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

༤ གལ་སྲིད་འཕྲོད་བསྟེན་ལུ་འཐུས་ཤོར་བྱུང་པ་ཅིན་ འཕྲོད་བསྟེན་མེདཔ་སྦེ་སྙན་ཞུ་འབད།```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

གསོ་བའི་སྙན་ཞུ་ཚུ་ སྲིད་བྱུས་ཚད་གཞི་ལུ་ལྷོདཔ་ད་ སོ་རཀ་ལཱའུཌ་གིས་ རང་བཞིན་གྱིས་ བསྐོར་བརྐྱབ་ཨིན།
གཞི་རྟེན་བསྐྱར་ཞིབ་ལུ་ལོག་འོང་ནི་དང་ རྩིས་ཞིབ་བྱུང་ལས་ཚུ་ ཐོ་བཀོད་འབདཝ་ཨིན།

## 6. ལག་ཐོག་ཕྱིར་ལོག་དང་དོན་རྐྱེན་ལན་འདེབས།

ཧེ་མའི་ཐོན་རིམ་ལུ་ལོག་འགྱོ།

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

ངེས་དཔྱད་འབད་ནི་ལུ་ གནས་ཚད་ཨའུཊི་པུཊི་ལག་ལེན་འཐབ།

- `current_version` ཕྱིར་ལོག་འབད་ཡོདཔ།
- `audit_event_count` ཡར་སེང་འབད་ཡོདཔ།
- `active_rollout` བསལ་ཡོདཔ།
- `last_rollout.stage` འདི་ རང་བཞིན་གྱི་ཕྱིར་ལོག་ཚུ་གི་དོན་ལུ་ `RolledBack` ཨིན།

## 7. བཀོལ་སྤྱོད་ཞིབ་དཔྱད་ཐོ་ཡིག།

- ཐོན་རིམ་ཚད་འཛིན་འོག་ལུ་ ཊེམ་པེལེཊི་གིས་བཟོ་བཏོན་འབད་ཡོད་པའི་ མངོན་གསལ་ཚུ་བཞག།
- འཚོལ་ཞིབ་འབད་ཚུགསཔ་སྦེ་ཉམས་སྲུང་འབད་ནི་ལུ་ བཤུད་བརྙན་རིམ་པ་རེ་རེ་གི་དོན་ལུ་ `governance_tx_hash` ཐོ་བཀོད་འབད།
- སྨན་བཅོས། `service_health`, `routing`, `resource_pressure`, དང་
  `failed_admissions` འདི་ རོལ་ཨའུཊི་གཱེཊི་ཨིན་པུཊི་ཚུ་སྦེ།
- ཐད་ཀར་དུ་ ཆ་ཚང་བཏོག་ནི་ལས་ ཀེ་ན་རི་བརྒྱ་ཆ་དང་ གསལ་ཏོག་ཏོ་སྦེ་ ཁྱབ་སྤེལ་འབད་ནི་ལས་ ལག་ལེན་འཐབ།
  ལག་ལེན་པ་ལུ་གདོང་ལེན་འབད་མི་ཞབས་ཏོག་ཚུ་གི་དོན་ལུ་ ཡར་འཕར་འབདཝ་ཨིན།
- ལཱ་ཡུན་/བདེན་བཤད་དང་ མིང་རྟགས་-བདེན་དཔྱད་སྤྱོད་ལམ་འདི་ ༢༠༢༠ ནང་ བདེན་དཔྱད་འབད།
  `webapp/api/server.mjs` ཐོན་སྐྱེད་མ་འབད་བའི་ཧེ་མ།