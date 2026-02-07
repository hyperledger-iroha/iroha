---
id: incident-runbooks
lang: my
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

##ရည်ရွယ်ချက်

လမ်းပြမြေပုံပါ ပစ္စည်း **DOCS-9** သည် လုပ်ဆောင်နိုင်သော ကစားစာအုပ်များအပြင် အစမ်းလေ့ကျင့်မှု အစီအစဉ်ကို တောင်းဆိုပါသည်။
ပေါ်တယ်အော်ပရေတာများသည် မှန်းဆစရာမလိုဘဲ ပို့ဆောင်မှုချို့ယွင်းမှုမှ ပြန်လည်ကောင်းမွန်လာနိုင်သည်။ ဒီမှတ်စု
အချက်ပြမှု မြင့်မားသည့် ဖြစ်ရပ်သုံးမျိုး ပါဝင်သည်။
ပျက်စီးယိုယွင်းမှုနှင့် ခွဲခြမ်းစိတ်ဖြာမှု ပြတ်တောက်မှုများ—နှင့် ယင်းကို သုံးလတစ်ကြိမ်လေ့ကျင့်မှုများကို မှတ်တမ်းတင်သည်။
alias rollback နှင့် synthetic validation သည် အဆုံးအထိ လုပ်ဆောင်နေဆဲဖြစ်ကြောင်း သက်သေပြပါ။

### ဆက်စပ်ပစ္စည်း

- [`devportal/deploy-guide`](./deploy-guide) — ထုပ်ပိုးခြင်း၊ လက်မှတ်ရေးထိုးခြင်းနှင့် နာမည်တူများ
  မြှင့်တင်ရေး လုပ်ငန်းအသွားအလာ။
- [`devportal/observability`](./observability) — ထုတ်ဝေလိုက်သော တဂ်များ၊ ခွဲခြမ်းစိတ်ဖြာချက်များနှင့်
  အောက်တွင်ဖော်ပြထားသော probes များ။
- `docs/source/sorafs_node_client_protocol.md`
  နှင့် [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - မှတ်ပုံတင်ကြေးနန်းတင်ခြင်းနှင့် တိုးမြှင့်ခြင်းအဆင့်များ။
- `docs/portal/scripts/sorafs-pin-release.sh` နှင့် `npm run probe:*` အကူအညီများ
  စစ်ဆေးစာရင်းများတစ်လျှောက် ကိုးကားထားသည်။

### တယ်လီမီတာနှင့် ကိရိယာများကို မျှဝေထားသည်။

| Signal / Tool | ရည်ရွယ်ချက် |
| ------------- | -------|
| `torii_sorafs_replication_sla_total` (met/missed/pending) | ပုံတူပွားဆိုင်များနှင့် SLA ချိုးဖောက်မှုများကို ရှာဖွေတွေ့ရှိသည်။ |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | စမ်းသုံးမှုအတွက် backlog အတိမ်အနက်နှင့် ပြီးဆုံးချိန်နေချိန်ကို တွက်ချက်သည်။ |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | မကောင်းသော ဖြန့်ကျက်မှုနောက်သို့ လိုက်သွားလေ့ရှိသော ဂိတ်ဝ-ဘက်ခြမ်း ချို့ယွင်းချက်များကို ပြသည်။ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | ဂိတ်ပေါက်များ ထုတ်ပေးပြီး ပြန်ပြေးမှုများကို မှန်ကန်ကြောင်း အတည်ပြုသည့် ဓာတုဗေဒ ပရောဖက်များ။ |
| `npm run check:links` | ကျိုးပဲ့လင့်ခ်တံခါး; လျှော့ချမှုတိုင်းပြီးနောက် အသုံးပြုသည်။ |
| `sorafs_cli manifest submit … --alias-*` (`scripts/sorafs-pin-release.sh`) | နာမည်များ မြှင့်တင်ရေး/ပြောင်းပြန်လှန်ခြင်း ယန္တရား။ |
| `Docs Portal Publishing` Grafana ဘုတ် (`dashboards/grafana/docs_portal.json`) | ငြင်းဆို/အမည်များ/TLS/ပုံတူတယ်လီမီတာကို စုစည်းသည်။ PagerDuty သည် ဤအကန့်များကို အထောက်အထားအတွက် ရည်ညွှန်းပါသည်။ |

## Runbook — အသုံးချမှု မအောင်မြင်ပါ သို့မဟုတ် မကောင်းတဲ့ လက်ရာ

### အခြေအနေများ အစပျိုးသည်။

- အစမ်းကြည့်ရှုခြင်း/ ထုတ်လုပ်ရေးဆိုင်ရာ စုံစမ်းစစ်ဆေးမှုများ မအောင်မြင်ပါ (`npm run probe:portal -- --expect-release=…`)။
- Grafana တွင် `torii_sorafs_gateway_refusals_total` သို့မဟုတ်
  စတင်ပြီးနောက် `torii_sorafs_manifest_submit_total{status="error"}`။
- Manual QA သည် ပျက်သွားသော လမ်းကြောင်းများ သို့မဟုတ် Try-It proxy ကျရှုံးပြီးနောက် ချက်ချင်း အကြောင်းကြားသည်။
  နာမည် အရောင်းမြှင့်တင်ရေး။

### ချက်ခြင်းချုပ်နှောင်ခြင်း။

1. ** ဖြန့်ကျက်မှုကို ရပ်တန့်လိုက်ပါ-** CI ပိုက်လိုင်းကို `DEPLOY_FREEZE=1` (GitHub ဖြင့် အမှတ်အသားပြုပါ
   အလုပ်အသွားအလာ ထည့်သွင်းခြင်း) သို့မဟုတ် Jenkins အလုပ်ကို ခေတ္တရပ်ထားခြင်းဖြင့် အပိုပစ္စည်းများ ထွက်မလာပါ။
2. **အရာများကို ဖမ်းယူပါ-** ပျက်ကွက်နေသော တည်ဆောက်မှု၏ `build/checksums.sha256` ကို ဒေါင်းလုဒ်လုပ်ပါ
   `portal.manifest*.{json,to,bundle,sig}`၊ နှင့် probe output သို့ ပြန်လှည့်နိုင်သည်။
   အတိအကျအချေအတင်များကိုကိုးကား။
3. **သက်ဆိုင်သူများကို အကြောင်းကြားပါ-** သိုလှောင်မှု SRE၊ Docs/DevRel ဦးဆောင်မှု၊ နှင့် အုပ်ချုပ်မှု
   သတိတရားအတွက် တာဝန်မှူး (အထူးသဖြင့် `docs.sora` သက်ရောက်မှုရှိသောအခါ)။

### ပြန်လှည့်ခြင်းလုပ်ငန်းစဉ်

1. နောက်ဆုံးသိထားသော-ကောင်းသော (LKG) ကို ဖော်ထုတ်ပါ။ ထုတ်လုပ်မှုလုပ်ငန်းအသွားအလာစတိုးဆိုင်များ
   ၎င်းတို့သည် `artifacts/devportal/<release>/sorafs/portal.manifest.to` အောက်တွင်ရှိသည်။
2. ပို့ဆောင်ရေးအကူအညီပေးသူနှင့် ထိုဖော်ပြချက်တွင် နံမည်ကို ပြန်လည်ပေါင်းစည်းပါ-

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. အဖြစ်အပျက်လက်မှတ်တွင် နောက်ကြောင်းပြန်အကျဉ်းချုပ်ကို LKG နှင့် အတူ မှတ်တမ်းတင်ပါ။
   မအောင်မြင်သော ရလဒ်များကို ဖော်ပြသည်။

### အတည်ပြုခြင်း။

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`။
2. `npm run check:links`။
3. `sorafs_cli manifest verify-signature …` နှင့် `sorafs_cli proof verify …`
   ပြန်လည်မြှင့်တင်ထားသောဖော်ပြချက်သည် ကိုက်ညီနေသေးကြောင်း အတည်ပြုရန် (အသုံးပြုမှုလမ်းညွှန်ကို ကြည့်ပါ)
   မော်ကွန်းတင်ထားသော CAR
4. Try-It staging proxy ပြန်လာကြောင်းသေချာစေရန် `npm run probe:tryit-proxy`။

### အခင်းအကျင်း

1. မူလအကြောင်းအရင်းကို နားလည်ပြီးမှသာ ဖြန့်ကျက်ပိုက်လိုင်းကို ပြန်ဖွင့်ပါ။
2. Backfill [`devportal/deploy-guide`](./deploy-guide) "သင်ယူခဲ့သောသင်ခန်းစာများ"
   gotchas အသစ်များပါ၀င်သည်များရှိပါက။
3. ပျက်ကွက်စမ်းသပ်မှုအစုံအတွက် ဖိုင်ချို့ယွင်းချက်များ (စုံစမ်းစစ်ဆေးခြင်း၊ လင့်ခ်စစ်ဆေးခြင်းစသည်ဖြင့်)။

## Runbook — ကူးယူမှု ပျက်ယွင်းခြင်း။

### အခြေအနေများ အစပျိုးသည်။

- သတိပေးချက်- `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 မိနစ်။
- `torii_sorafs_replication_backlog_total > 10` 10 မိနစ် (ကြည့်ပါ။
  `pin-registry-ops.md`)။
- ထုတ်ဝေပြီးသည့်နောက်တွင် အုပ်ချုပ်မှုစနစ်သည် နှေးကွေးသောအမည်များရရှိနိုင်မှုကို အစီရင်ခံသည်။

### Triage

1. အတည်ပြုရန် [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) ဒက်ရှ်ဘုတ်များကို စစ်ဆေးပါ
   backlog ကို သိုလှောင်မှု အတန်း သို့မဟုတ် ပံ့ပိုးပေးသူ အဖွဲ့သို့ ဒေသစံသတ်မှတ်ခြင်း ရှိ၊မရှိ၊
2. Torii မှ `sorafs_registry::submit_manifest` သတိပေးချက်များအတွက် အပြန်အလှန်စစ်ဆေးခြင်းမှတ်တမ်းများ
   တင်ပြချက်များကိုယ်တိုင် ပျက်ကွက်ခြင်းရှိမရှိ ဆုံးဖြတ်ပါ။
3. နမူနာပုံတူ ကျန်းမာရေး `sorafs_cli manifest status --manifest …` (စာရင်းများ
   ပံ့ပိုးသူ တစ်ဦးချင်း ပုံတူပွားခြင်း ရလဒ်များ)။

### လျော့ပါးစေခြင်း။

1. ပိုမိုမြင့်မားသောပုံတူအရေအတွက် (`--pin-min-replicas 7`) ကိုအသုံးပြု၍ မန်နီးဖက်စ်ကို ပြန်လည်ထုတ်ပေးပါ
   `scripts/sorafs-pin-release.sh` ထို့ကြောင့် အချိန်ဇယားဆွဲသူက ပိုကြီးသော ဝန်ကို ပျံ့နှံ့စေသည်။
   ပံ့ပိုးပေးသူသတ်မှတ်။ အဖြစ်အပျက်မှတ်တမ်းတွင် ဖော်ပြချက်အသစ်ကို မှတ်တမ်းတင်ပါ။
2. backlog ကို ပံ့ပိုးသူတစ်ခုတည်းနှင့် ချိတ်ဆက်ထားပါက၊ ၎င်းကို ယာယီပိတ်ထားပါ။
   ကူးယူမှုအချိန်ဇယား (`pin-registry-ops.md` တွင်မှတ်တမ်းတင်ထားသည်) နှင့်အသစ်တစ်ခုတင်သွင်းပါ။
   အခြားဝန်ဆောင်မှုပေးသူများကို alias အား ပြန်လည်စတင်ရန် အတင်းအကျပ် ထင်ရှားစေသည်။
3. ပုံတူပွားခြင်း ညီမျှခြင်းထက် alias လတ်ဆတ်မှုသည် ပိုအရေးကြီးသောအခါ၊ ၎င်းကို ပြန်ချည်နှောင်ပါ။
   စီစဉ်ပြီးသော နွေးထွေးသော သရုပ်သဏ္ဍာန် (`docs-preview`) ကို ထုတ်ဝေပြီးနောက်၊
   SRE သည် backlog ကိုရှင်းပြီးသည်နှင့်နောက်ဆက်တွဲဖော်ပြချက်။

### ပြန်လည်ရယူခြင်းနှင့် ပိတ်ခြင်း။

1. Monitor `torii_sorafs_replication_sla_total{outcome="missed"}` ကိုသေချာစေရန်
   ကုန်းပြင်မြင့်ကို ရေတွက်ပါ။
2. ပုံစံတူတိုင်းသည် အထောက်အထားအဖြစ် `sorafs_cli manifest status` အထွက်ကို ဖမ်းယူပါ
   လိုက်လျောညီထွေ ပြန်ဖြစ်သွားသည်။
3. နောက်အဆင့်များနှင့်အတူ ကူးယူမှုဆိုင်ရာ မှတ်တမ်းကို ဖိုင် သို့မဟုတ် အပ်ဒိတ်လုပ်ပါ။
   (ပံ့ပိုးပေးသူကို စကေးချခြင်း၊ ချန်းကာချိန်ညှိခြင်း စသည်ဖြင့်)။

## Runbook — ပိုင်းခြားစိတ်ဖြာမှု သို့မဟုတ် တယ်လီမီတာ ပြတ်တောက်မှု

### အခြေအနေများ အစပျိုးသည်။

- `npm run probe:portal` အောင်မြင်သော်လည်း ဒက်ရှ်ဘုတ်များသည် ထည့်သွင်းခြင်းကို ရပ်သွားပါသည်။
  `AnalyticsTracker` ဖြစ်ရပ်များ > 15 မိနစ်။
- ကိုယ်ရေးကိုယ်တာ ပြန်လည်သုံးသပ်မှု ကျဆင်းသွားသော ဖြစ်ရပ်များတွင် မမျှော်လင့်ထားသော တိုးလာမှုကို အလံပြသည်။
- `npm run probe:tryit-proxy` သည် `/probe/analytics` လမ်းကြောင်းများတွင် မအောင်မြင်ပါ။

### တုံ့ပြန်မှု

1. တည်ဆောက်ချိန်ထည့်သွင်းမှုများကို အတည်ပြုပါ- `DOCS_ANALYTICS_ENDPOINT` နှင့်
   မအောင်မြင်သော ထုတ်လွှတ်မှု လက်ရာ (`build/release.json`) တွင် `DOCS_ANALYTICS_SAMPLE_RATE`။
2. `npm run probe:portal` ကို `DOCS_ANALYTICS_ENDPOINT` ညွှန်ပြပြီး ပြန်ဖွင့်ပါ
   ခြေရာခံကိရိယာကို အတည်ပြုရန် အဆင့်လိုက်စုဆောင်းသူသည် payloads များကို ထုတ်လွှတ်ဆဲဖြစ်သည်။
3. စုဆောင်းသူများ ကျဆင်းသွားပါက `DOCS_ANALYTICS_ENDPOINT=""` ကို သတ်မှတ်ပြီး ပြန်လည်တည်ဆောက်ပါ။
   ခြေရာခံကိရိယာ တိုတောင်းသောဆားကစ်များ; အဖြစ်အပျက်အချိန်ဇယားတွင် ပြတ်တောက်မှုပြတင်းပေါက်ကို မှတ်တမ်းတင်ပါ။
4. `scripts/check-links.mjs` ရှိနေသေးသော လက်ဗွေရာ `checksums.sha256` ကို အတည်ပြုပါ
   (ခွဲခြမ်းစိတ်ဖြာမှု ပြတ်တောက်မှုများသည် ဆိုက်မြေပုံအတည်ပြုခြင်းကို *မဟုတ်* ပိတ်ဆို့ရပါမည်)။
5. စုဆောင်းသူပြန်လည်ကောင်းမွန်လာသည်နှင့် လေ့ကျင့်ခန်းပြုလုပ်ရန် `npm run test:widgets` ကို run ပါ။
   ပြန်လည်ထုတ်ဝေခြင်းမပြုမီ ခွဲခြမ်းစိတ်ဖြာမှုအကူအညီယူနစ် စမ်းသပ်မှုများ။

### အခင်းအကျင်း

1. မည်သည့်စုဆောင်းသူအသစ်နှင့်မဆို [`devportal/observability`](./observability) ကို အပ်ဒိတ်လုပ်ပါ။
   ကန့်သတ်ချက်များ သို့မဟုတ် နမူနာလိုအပ်ချက်များ။
2. ခွဲခြမ်းစိတ်ဖြာမှုဒေတာကို ပြုတ်ကျပါက သို့မဟုတ် ပြင်ပတွင် ပြန်လည်ပြင်ဆင်ပါက ဖိုင်အုပ်ချုပ်မှု သတိပေးချက်
   မူဝါဒ။

## သုံးလပတ်ခံနိုင်ရည်လေ့ကျင့်ခန်း

လေးပုံတစ်ပုံစီ၏ **ပထမအင်္ဂါနေ့** (ဇန်န၀ါရီ/ဧပြီ/ဇူလိုင်/အောက်တိုဘာ)အတွင်း လေ့ကျင့်ခန်းနှစ်ခုစလုံးကို လုပ်ဆောင်ပါ။
သို့မဟုတ် ကြီးကြီးမားမား အခြေခံအဆောက်အအုံ ပြောင်းလဲမှုတစ်ခုခုပြီးနောက် ချက်ချင်းပင်။ အောက်တွင် ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းပါ။
`artifacts/devportal/drills/<YYYYMMDD>/`။

| တူး | ခြေလှမ်းများ | အထောက်အထား |
| -----| -----| --------|
| Alias ​​rollback အစမ်းလေ့ကျင့်မှု | 1. နောက်ဆုံးထွက်ရှိမှု မန်နီးဖက်စ်ကို အသုံးပြု၍ "အသုံးပြုမှု မအောင်မြင်ပါ" နောက်ပြန်လှည့်မှုကို ပြန်ဖွင့်ပါ။<br/>2. စုံစမ်းစစ်ဆေးမှုများပြီးသည်နှင့် ထုတ်လုပ်မှုသို့ ပြန်လည်ချိတ်ဆက်ပါ။<br/>၃။ `portal.manifest.submit.summary.json` ကို မှတ်တမ်းတင်ပြီး drill ဖိုဒါတွင် မှတ်တမ်းများကို မှတ်တမ်းတင်ပါ။ | `rollback.submit.json`၊ စုံစမ်းစစ်ဆေးခြင်း ရလဒ်နှင့် အစမ်းလေ့ကျင့်မှု၏ ထုတ်လွှတ်မှု tag။ |
| Synthetic validation စာရင်းစစ် | 1. `npm run probe:portal` နှင့် `npm run probe:tryit-proxy` ကို ထုတ်လုပ်မှုနှင့် အဆင့်သတ်မှတ်ခြင်းအား ဆန့်ကျင်၍ လုပ်ဆောင်ပါ။<br/>၂။ `npm run check:links` ကိုဖွင့်ပြီး `build/link-report.json` ကို သိမ်းဆည်းပါ။<br/>၃။ စုံစမ်းစစ်ဆေးမှု အောင်မြင်ကြောင်း အတည်ပြုသည့် Grafana အကန့်များ၏ ဖန်သားပြင်ဓာတ်ပုံများ/ ထုတ်ယူမှုများကို ပူးတွဲပါ။ | ထင်ရှားသော လက်ဗွေကို ရည်ညွှန်းသည့် စုံစမ်းစစ်ဆေးမှုမှတ်တမ်းများ + `link-report.json`။ |

Docs/DevRel မန်နေဂျာနှင့် SRE အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်းသို့ လွတ်သွားသောလေ့ကျင့်ခန်းများကို တိုးမြှင့်ပါ။
လမ်းပြမြေပုံသည် အဆုံးအဖြတ်ပေးသော၊ သုံးလတစ်ကြိမ် အထောက်အထားများ လိုအပ်သောကြောင့် နှစ်ခုလုံးသည် အမည်များဖြစ်သည်။
rollback နှင့် portal probes များသည် ကျန်းမာနေပါသည်။

## PagerDuty & on-call ညှိနှိုင်းခြင်း။

- PagerDuty ဝန်ဆောင်မှု **Docs Portal Publishing** မှထုတ်ပေးသော သတိပေးချက်များကို ပိုင်ဆိုင်သည်။
  `dashboards/grafana/docs_portal.json`။ ဇတ်ကား `DocsPortal/GatewayRefusals`၊
  `DocsPortal/AliasCache` နှင့် `DocsPortal/TLSExpiry` စာမျက်နှာ Docs/DevRel
  အလယ်တန်းအဖြစ် Storage SRE ဖြင့် အဓိက။
- စာမျက်နှာပေးသောအခါ၊ `DOCS_RELEASE_TAG` ကိုထည့်သွင်းပါ၊ ထိခိုက်မှု၏ဖန်သားပြင်ဓာတ်ပုံများကို ပူးတွဲပါ။
  Grafana အကန့်များ၊ နှင့် ဆက်စပ်မှု/လင့်ခ်-စစ်ဆေးခြင်းအထွက်ကို အဖြစ်အပျက်မှတ်စုများရှေ့တွင်၊
  လျော့ပါးရေးစတင်သည်။
- လျှော့ချပြီးနောက် (နောက်ပြန်ဆွဲခြင်း သို့မဟုတ် ပြန်လည်အသုံးချခြင်း)၊ `npm run probe:portal` ကို ပြန်လည်လုပ်ဆောင်ပါ။
  `npm run check:links` နှင့် မက်ထရစ်များကိုပြသသည့် လတ်ဆတ်သော Grafana လျှပ်တစ်ပြက်ရိုက်ချက်များ
  ကန့်သတ်ဘောင်များအတွင်း ပြန်လည်ရောက်ရှိခြင်း။ PagerDuty ဖြစ်ရပ်မတိုင်မီ အထောက်အထားအားလုံးကို ပူးတွဲပါ
  အဲဒါကို ဖြေရှင်းတယ်။
- သတိပေးချက်နှစ်ခု ပြိုင်တူမီးလောင်ပါက (ဥပမာ TLS သက်တမ်းကုန်ဆုံးချိန် နှင့် backlog)၊ triage
  ဦးစွာ ငြင်းဆိုသည် (ထုတ်ဝေခြင်းကို ရပ်လိုက်သည်)၊ ပြန်လှည့်ခြင်းလုပ်ငန်းစဉ်ကို လုပ်ဆောင်ပြီးနောက် ရှင်းလင်းပါ။
  တံတားပေါ်တွင် Storage SRE ပါသော TLS/backlog ပစ္စည်းများ။