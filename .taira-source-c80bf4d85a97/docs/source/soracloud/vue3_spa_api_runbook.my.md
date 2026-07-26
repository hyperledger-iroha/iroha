<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
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

ဤအပြေးစာအုပ်တွင် ထုတ်လုပ်မှုကို ဦးတည်သော ဖြန့်ကျက်မှုနှင့် လုပ်ငန်းဆောင်ရွက်မှုများ ပါဝင်သည်-

- Vue3 အငြိမ်ဆိုက် (`--template site`); နှင့်
- Vue3 SPA + API ဝန်ဆောင်မှု (`--template webapp`)၊

SCR/IVM ယူဆချက်များဖြင့် Iroha 3 ရှိ Soracloud ထိန်းချုပ်ရေး-လေယာဉ် APIs ကို အသုံးပြုခြင်း (မဟုတ်ပါ
WASM runtime မှီခိုမှုနှင့် Docker မှီခိုမှု)။

## 1. နမူနာပရောဂျက်များကို ဖန်တီးပါ။

Static site scaffold-

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API Scaffold-

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

အထွက်လမ်းညွှန်တစ်ခုစီတွင်-

- `container_manifest.json`
- `service_manifest.json`
- `site/` သို့မဟုတ် `webapp/` အောက်တွင် နမူနာဖိုင်များ

## 2. အပလီကေးရှင်း ရှေးဟောင်းပစ္စည်းများကို တည်ဆောက်ပါ။

အငြိမ်ဆိုက်-

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA Frontend + API-

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. ပက်ကေ့ချ်ပြီး ရှေ့တန်းပစ္စည်းများကို ထုတ်ဝေပါ။

SoraFS မှတဆင့် static hosting အတွက်

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA ရှေ့တန်းအတွက်

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. တိုက်ရိုက် Soracloud ထိန်းချုပ်မှုလေယာဉ်ကို အသုံးပြုပါ။

အငြိမ်ဆိုက်ဝန်ဆောင်မှုကို အသုံးပြုပါ-

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API ဝန်ဆောင်မှုကို အသုံးပြုပါ-

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

လမ်းကြောင်းချိတ်ဆက်မှုနှင့် ဖြန့်ချိမှုအခြေအနေအား အတည်ပြုပါ-

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

မျှော်လင့်ထားသည့် ထိန်းချုပ်-လေယာဉ်စစ်ဆေးမှုများ-

- `control_plane.services[].latest_revision.route_host` အစုံ
- `control_plane.services[].latest_revision.route_path_prefix` အစုံ (`/` သို့မဟုတ် `/api`)
- `control_plane.services[].active_rollout` ကို အဆင့်မြှင့်ပြီးနောက် ချက်ချင်း ပေါ်လာသည်။

## 5. ကျန်းမာရေး ကန့်သတ်ထားသော ဖြန့်ချိမှုဖြင့် အဆင့်မြှင့်ပါ။

1. ဝန်ဆောင်မှု မန်နီးဖက်စ်တွင် `service_version` ကို ဖိပါ။
2. အဆင့်မြှင့်တင်မှုကို လုပ်ဆောင်ပါ-

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. ကျန်းမာရေးစစ်ဆေးမှုများပြီးနောက် ဖြန့်ချိမှုကို မြှင့်တင်ပါ-

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. ကျန်းမာရေးချို့တဲ့ပါက ကျန်းမာရေးမကောင်းကြောင်း သတင်းပို့ပါ-```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

ကျန်းမာရေးနှင့် မညီညွတ်သော အစီရင်ခံစာများသည် မူဝါဒအဆင့်သို့ ရောက်ရှိသောအခါ၊ Soracloud သည် အလိုအလျောက် လည်ပတ်သည်။
အခြေခံပြန်လည်ပြင်ဆင်မှုသို့ ပြန်သွားရန်နှင့် နောက်ကြောင်းပြန်စစ်ဆေးခြင်းဖြစ်ရပ်များကို မှတ်တမ်းတင်သည်။

## 6. လူကိုယ်တိုင် နောက်ပြန်ဆွဲခြင်းနှင့် အဖြစ်အပျက် တုံ့ပြန်ခြင်း။

ယခင်ဗားရှင်းသို့ ပြန်သွားရန်-

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

အတည်ပြုရန် status output ကိုသုံးပါ-

- `current_version` ကို ပြန်ပြောင်းထားသည်။
- `audit_event_count` တိုးထားသည်။
- `active_rollout` ကို ရှင်းလင်းထားသည်။
- `last_rollout.stage` သည် အော်တိုပြန်လှည့်ခြင်းအတွက် `RolledBack` ဖြစ်သည်

## 7. လုပ်ငန်းဆောင်ရွက်မှု စစ်ဆေးရေးစာရင်း

- ဗားရှင်းထိန်းချုပ်မှုအောက်တွင် ပုံစံပလိတ်ထုတ်လုပ်ထားသော မန်နီးဖက်စ်များကို သိမ်းဆည်းထားပါ။
- ခြေရာခံနိုင်မှုကို ထိန်းသိမ်းထားရန် စတင်သည့်အဆင့်တိုင်းအတွက် `governance_tx_hash` ကို မှတ်တမ်းတင်ပါ။
- `service_health`၊ `routing`၊ `resource_pressure` နှင့် ဆက်ဆံပါ
  `failed_admissions` သည် rollout gate inputs အဖြစ်
- တိုက်ရိုက်ဖြတ်တောက်ခြင်းထက် ကိန္နရီရာခိုင်နှုန်းများနှင့် တိကျပြတ်သားသော ပရိုမိုးရှင်းများကို အသုံးပြုပါ။
  သုံးစွဲသူမျက်နှာစာ ဝန်ဆောင်မှုများအတွက် အဆင့်မြှင့်တင်မှုများ။
- session/auth နှင့် signature-verification အပြုအမူကို validate လုပ်ပါ။
  မထုတ်လုပ်မီ `webapp/api/server.mjs`။