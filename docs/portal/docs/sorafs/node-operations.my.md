---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d99cea6198ef7ea6c75d7823854237983f17c3341cea2b3e491bb03e54531f2
source_last_modified: "2026-01-22T14:35:36.797300+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
မှန်ချပ်များ `docs/source/sorafs/runbooks/sorafs_node_ops.md`။ Sphinx သတ်မှတ်မှု အနားမယူမချင်း ဗားရှင်းနှစ်မျိုးလုံးကို ထပ်တူကျအောင်ထားပါ။
:::

## ခြုံငုံသုံးသပ်ချက်

ဤ runbook သည် Torii အတွင်း ထည့်သွင်းထားသော `sorafs-node` ဖြန့်ကျက်မှုကို အတည်ပြုခြင်းအားဖြင့် အော်ပရေတာများကို လမ်းလျှောက်ပေးပါသည်။ အပိုင်းတစ်ခုစီသည် SF-3 ပေးပို့နိုင်သည့်အရာများထံ တိုက်ရိုက်မြေပုံပြသည်- pin/fetch round trips၊ recovery ပြန်လည်စတင်ခြင်း၊ quota rejection နှင့် PoR နမူနာယူခြင်း။

## 1. ကြိုတင်လိုအပ်ချက်များ

- `torii.sorafs.storage` တွင် သိုလှောင်မှုဝန်ထမ်းကို ဖွင့်ပါ-

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Torii လုပ်ငန်းစဉ်သည် `data_dir` သို့ ဖတ်ရှု/ရေးခွင့်ရှိကြောင်း သေချာပါစေ။
- ကြေငြာချက်တစ်ခုမှတ်တမ်းတင်သည်နှင့်တစ်ပြိုင်နက် node သည်မျှော်လင့်ထားသောစွမ်းရည်ကို `GET /v2/sorafs/capacity/state` မှတစ်ဆင့်ကြော်ငြာကြောင်းအတည်ပြုသည်။
- ချောမွေ့မှုကို ဖွင့်ထားသောအခါ၊ ဒက်ရှ်ဘုတ်များသည် အကြမ်းထည်နှင့် ချောမွေ့သော GiB·hour/PoR ကောင်တာများကို အစက်အပြောက်တန်ဖိုးများနှင့်အတူ တုန်လှုပ်မှုမရှိသော ခေတ်ရေစီးကြောင်းများကို မီးမောင်းထိုးပြရန် ထုတ်ဖော်ပြသသည်။

### CLI Dry Run (ချန်လှပ်ထားနိုင်သည်)

HTTP အဆုံးမှတ်များကို ထုတ်ဖော်ခြင်းမပြုမီ စုစည်းထားသော CLI ဖြင့် သိုလှောင်မှုနောက်ကွယ်ကို သေချာစစ်ဆေးနိုင်ပါသည်။ 【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

ညွှန်ကြားချက်များသည် Norito JSON အနှစ်ချုပ်များကို print ထုတ်ပြီး အတုံးအခဲများ ပရိုဖိုင်း သို့မဟုတ် မကိုက်ညီမှုများကို ချေဖျက်ရန် ငြင်းဆိုခြင်းဖြင့် ၎င်းတို့သည် I18NT000000003X ဝိုင်ယာကြိုးများမတိုင်မီ CI မီးခိုးစစ်ဆေးမှုများအတွက် အသုံးဝင်စေပါသည်။【crates/sorafs_node/tests/cli.rs#L1】

### PoR Proof Rehearsal

အော်ပရေတာများသည် ယခုအခါ Torii သို့မတင်မီ အုပ်ချုပ်မှုမှထုတ်ပေးသော PoR အနုပညာပစ္စည်းများကို စက်တွင်းတွင် ပြန်လည်ပြသနိုင်ပြီဖြစ်သည်။ CLI သည် တူညီသော `sorafs-node` သုံးစွဲမှုလမ်းကြောင်းကို ပြန်လည်အသုံးပြုသည်၊ ထို့ကြောင့် ပြည်တွင်းလုပ်ဆောင်မှုများသည် HTTP API ပြန်လာမည့် တိကျသေချာသော တရားဝင်မှုအမှားများကို ပေါ်လွင်စေသည်။

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

ညွှန်ကြားချက်သည် JSON အနှစ်ချုပ် (manifest digest၊ provider id၊ proof digest၊ sample count၊ optional verdict ရလဒ်) ကို ထုတ်လွှတ်သည်။ သိမ်းဆည်းထားသော မန်နီးဖက်စ်သည် စိန်ခေါ်မှုအချေအတင်နှင့် ကိုက်ညီကြောင်းသေချာစေရန် `--manifest-id=<hex>` နှင့် စာရင်းစစ်အထောက်အထားအတွက် အကျဉ်းချုပ်ကို မူရင်းလက်ရာများဖြင့် သိမ်းဆည်းလိုပါက `--json-out=<path>` ပေးပါ။ `--verdict` အပါအဝင် သင်သည် HTTP API ကို မခေါ်ဆိုမီ စိန်ခေါ်မှု → သက်သေ → စီရင်ချက် အားလုံးကို အော့ဖ်လိုင်းဖြင့် အစမ်းလေ့ကျင့်နိုင်စေပါသည်။

Torii ကို တိုက်ရိုက်လွှင့်ပြီးသည်နှင့် HTTP မှတစ်ဆင့် အလားတူပစ္စည်းများကို သင်ရယူနိုင်သည်-

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

အဆုံးမှတ်နှစ်ခုလုံးကို မြှုပ်ထားသည့် သိုလှောင်မှုဝန်ထမ်းက ဆောင်ရွက်ပေးသည်၊ ထို့ကြောင့် CLI မီးခိုးစမ်းသပ်မှုများနှင့် ဂိတ်ဝေးစုံစမ်းစစ်ဆေးမှုများသည် တစ်ပြိုင်တည်းရှိနေပါသည်။【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → အသွားအပြန်ခရီးကို ရယူပါ။

1. manifest + payload bundle (ဥပမာ `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`) ကို ထုတ်လုပ်ပါ။
2. base64 ကုဒ်ဖြင့် မန်နီးဖက်စ်ကို တင်ပြပါ-

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   တောင်းဆိုချက် JSON တွင် `manifest_b64` နှင့် `payload_b64` ပါဝင်ရပါမည်။ အောင်မြင်သော တုံ့ပြန်မှုသည် `manifest_id_hex` နှင့် payload digest ကို ပြန်ပေးသည်။
3. ပင်ထိုးထားသောဒေတာကို ရယူပါ-

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64- `data_b64` အကွက်ကို ကုဒ်ဝှက်ပြီး မူရင်းဘိုက်များနှင့် ကိုက်ညီကြောင်း အတည်ပြုပါ။

## 3. Recovery Drill ကို ပြန်လည်စတင်ပါ။

1. အထက်ဖော်ပြပါအတိုင်း အနည်းဆုံး manifest တစ်ခုကို ပင်ထိုးပါ။
2. Torii လုပ်ငန်းစဉ် (သို့မဟုတ် node တစ်ခုလုံး) ကို ပြန်လည်စတင်ပါ။
3. ထုတ်ယူမှုတောင်းဆိုချက်ကို ပြန်လည်တင်ပြပါ။ ပေးဆောင်မှုအား ပြန်လည်ထုတ်ယူနိုင်ဆဲဖြစ်ရမည်ဖြစ်ပြီး ပြန်ပေးသည့်အညွှန်းသည် ကြိုတင်ပြန်လည်စတင်သည့်တန်ဖိုးနှင့် ကိုက်ညီရပါမည်။
4. ပြန်လည်စတင်ပြီးနောက် ဆက်ရှိနေသော သရုပ်များကို ထင်ဟပ်စေသည့် `bytes_used` ကို အတည်ပြုရန် `GET /v2/sorafs/storage/state` ကို စစ်ဆေးပါ။

## 4. Quota Rejection Test

1. `torii.sorafs.storage.max_capacity_bytes` ကို သေးငယ်သော တန်ဖိုးတစ်ခုသို့ ယာယီလျှော့ချပါ (ဥပမာ မန်နီးဖက်စ်တစ်ခု၏ အရွယ်အစား)။
2. one manifest ကို ပင်ထိုးပါ။ တောင်းဆိုမှုအောင်မြင်ရမည်။
3. အလားတူအရွယ်အစားရှိသော ဒုတိယဖော်ပြချက်ကို ပင်ထိုးရန်ကြိုးစားပါ။ Torii သည် HTTP `400` နှင့် `storage capacity exceeded` ပါဝင်သော အမှားအယွင်းမက်ဆေ့ဂျ်ဖြင့် တောင်းဆိုချက်ကို ပယ်ချရပါမည်။
4. ပြီးသွားသောအခါတွင် ပုံမှန်စွမ်းရည်ကန့်သတ်ချက်ကို ပြန်လည်ရယူပါ။

## 5. Retention / GC စစ်ဆေးခြင်း (Read-only)

1. သိုလှောင်မှုလမ်းညွှန်ကို ဆန့်ကျင်သည့် စက်တွင်းထိန်းသိမ်းမှုစကင်န်ကို လုပ်ဆောင်ပါ-

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. သက်တမ်းကုန်နေသော သရုပ်များကိုသာ စစ်ဆေးပါ (အခြောက်ခံရုံသာ၊ ဖျက်ခြင်းမပြုပါ)။

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. လက်ခံဆောင်ရွက်ပေးသူများ သို့မဟုတ် ဖြစ်ရပ်များတစ်လျှောက် အစီရင်ခံစာများကို နှိုင်းယှဉ်သည့်အခါ အကဲဖြတ်ဝင်းဒိုးကို ပင်ထိုးရန် `--now` သို့မဟုတ် `--grace-secs` ကို အသုံးပြုပါ။

GC CLI သည် ရည်ရွယ်ချက်ရှိရှိ ဖတ်ရန်သာဖြစ်သည်။ စာရင်းစစ်လမ်းကြောင်းများအတွက် ထိန်းသိမ်းထားရမည့် သတ်မှတ်ရက်များနှင့် သက်တမ်းကုန်သွားသော စာရင်းအင်းများကို ဖမ်းယူရန် ၎င်းကို အသုံးပြုပါ။ ထုတ်လုပ်မှုတွင် ဒေတာကို ကိုယ်တိုင်မဖယ်ရှားပါနှင့်။

## 6. PoR Sampling Probe

1. မန်နီးဖက်စ်တစ်ခုကို ပင်ထိုးပါ။
2. PoR နမူနာကို တောင်းဆိုပါ-

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. တုံ့ပြန်ချက်တွင် တောင်းဆိုထားသော အရေအတွက်နှင့်အတူ `samples` ပါဝင်ကြောင်း အတည်ပြုပြီး အထောက်အထားတစ်ခုစီသည် သိမ်းဆည်းထားသော manifest အမြစ်နှင့် ဆန့်ကျင်ဘက်ဖြစ်ကြောင်း အတည်ပြုသည်။

## 7. Automation ချိတ်များ

- CI / မီးခိုးစမ်းသပ်မှုများတွင် ထည့်သွင်းထားသော ပစ်မှတ်ထားသော စစ်ဆေးမှုများကို ပြန်လည်အသုံးပြုနိုင်သည်-

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  `pin_fetch_roundtrip`၊ `pin_survives_restart`၊ `pin_quota_rejection` နှင့် `por_sampling_returns_verified_proofs` တို့ ပါဝင်သည်။
- ဒက်ရှ်ဘုတ်များသည် ခြေရာခံသင့်သည်-
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` နှင့် `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` မှတစ်ဆင့် PoR အောင်မြင်မှု/ကျရှုံးမှုကောင်တာများ ပေါ်လာသည်။
  - ဖြေရှင်းခြင်း ထုတ်ဝေရန် ကြိုးပမ်းမှုများ `sorafs_node_deal_publish_total{result=success|failure}`

ဤလေ့ကျင့်ခန်းများကို လိုက်နာခြင်းဖြင့် မြှပ်နှံထားသော သိုလှောင်မှုလုပ်သားသည် ဒေတာထည့်သွင်းနိုင်ခြင်း၊ ပြန်လည်စတင်ခြင်းများကို ရှင်သန်နိုင်စေခြင်း၊ ပြင်ဆင်သတ်မှတ်ထားသော ခွဲတမ်းများကို လေးစားလိုက်နာနိုင်ပြီး node သည် ပိုမိုကျယ်ပြန့်သောကွန်ရက်သို့ စွမ်းရည်မကြော်ငြာမီ တိကျသေချာသော PoR အထောက်အထားများကို ထုတ်လုပ်နိုင်မည်ဖြစ်သည်။