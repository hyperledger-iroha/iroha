<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API Runbook

ይህ runbook ምርት ተኮር ማሰማራትን እና ለሚከተሉት ስራዎችን ይሸፍናል።

- የ Vue3 የማይንቀሳቀስ ጣቢያ (`--template site`); እና
- Vue3 SPA + API አገልግሎት (`--template webapp`) ፣

የ Soracloud መቆጣጠሪያ-አውሮፕላን ኤፒአይዎችን በ Iroha 3 ከ SCR/IVM ግምቶች ጋር (አይደለም)
WASM የአሂድ ጥገኝነት እና ምንም Docker ጥገኝነት የለም)።

## 1. የአብነት ፕሮጀክቶችን መፍጠር

የማይንቀሳቀስ የጣቢያ ቅርፊት;

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + ኤፒአይ ስካፎል፡

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

እያንዳንዱ የውጤት ማውጫ የሚከተሉትን ያጠቃልላል

- `container_manifest.json`
- `service_manifest.json`
- የአብነት ምንጭ ፋይሎች በ `site/` ወይም `webapp/` ስር

## 2. የመተግበሪያ ቅርሶችን ይገንቡ

የማይንቀሳቀስ ጣቢያ፡

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA frontend + API፡

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. የፊት ለፊት ንብረቶችን ማሸግ እና ማተም

ለስታቲክ ማስተናገጃ በSoraFS፡

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

ለ SPA ግንባር፡

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. ለቀጥታ የሶራክሎድ መቆጣጠሪያ አውሮፕላን ያሰማሩ

የማይንቀሳቀስ ጣቢያ አገልግሎት አሰማር፡

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

የSPA + ኤፒአይ አገልግሎት አሰማር፡

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

የመንገድ ማሰሪያ እና የመልቀቅ ሁኔታን ያረጋግጡ፡-

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

የሚጠበቁ የመቆጣጠሪያ-አውሮፕላኖች ፍተሻዎች፡-

- `control_plane.services[].latest_revision.route_host` ስብስብ
- `control_plane.services[].latest_revision.route_path_prefix` ስብስብ (`/` ወይም `/api`)
- `control_plane.services[].active_rollout` ከተሻሻለ በኋላ ወዲያውኑ ይገኛል።

## 5. በጤና-የተከለለ ልቀት አሻሽል።

1. Bump `service_version` በአገልግሎት መግለጫው ውስጥ።
2. ማሻሻልን አሂድ፡

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. ከጤና ቁጥጥር በኋላ መልቀቅን ያስተዋውቁ፡-

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. ጤና ካልተሳካ ጤናማ ያልሆነን ሪፖርት ያድርጉ፡-```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

ጤናማ ያልሆኑ ሪፖርቶች የመመሪያ ገደብ ላይ ሲደርሱ፣ Soracloud በራስ-ሰር ይንከባለል
ወደ መነሻ መስመር ክለሳ እና የድጋሚ ኦዲት ክስተቶችን ይመዘግባል።

## 6. በእጅ መመለስ እና የአደጋ ምላሽ

ወደ ቀዳሚው ስሪት መመለስ;

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

ለማረጋገጥ የሁኔታ ውጤትን ተጠቀም፡-

- `current_version` ተመልሷል
- `audit_event_count` ጨምሯል።
- `active_rollout` ጸድቷል
- `last_rollout.stage` `RolledBack` ነው ለራስ-ሰር መልሶ ማሽከርከር

## 7. የክወናዎች ዝርዝር

- በአብነት የመነጩ መገለጫዎችን በስሪት ቁጥጥር ውስጥ ያቆዩ።
- የመከታተያ ችሎታን ለመጠበቅ ለእያንዳንዱ የታቀደ ልቀት `governance_tx_hash` ይመዝግቡ።
- `service_health`፣ `routing`፣ `resource_pressure`፣ እና
  `failed_admissions` እንደ የታቀፉ በር ግብዓቶች።
- በቀጥታ ሙሉ ከመቁረጥ ይልቅ የካናሪ በመቶኛ እና ግልጽ ማስተዋወቂያ ይጠቀሙ
  ተጠቃሚ ለሚሆኑ አገልግሎቶች ማሻሻያዎች።
- የክፍለ-ጊዜ/የማረጋገጫ እና የፊርማ ማረጋገጫ ባህሪን ያረጋግጡ
  `webapp/api/server.mjs` ከማምረት በፊት.