---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#SoraFS Gateway & DNS Kickoff Runbook

ဤပေါ်တယ် မိတ္တူသည် canonical runbook ကို ရောင်ပြန်ဟပ်သည်။
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)။
၎င်းသည် Decentralized DNS & Gateway အတွက် လည်ပတ်မှုဆိုင်ရာ အစောင့်အကြပ်များကို ဖမ်းယူသည်။
workstream သည် networking၊ ops နှင့် documentation leads တို့ကို အစမ်းလေ့ကျင့်နိုင်ပါသည်။
2025-03 ဘောလုံးပွဲမတိုင်မီ အလိုအလျောက်စနစ်သုံးစနစ်။

## အတိုင်းအတာနှင့် ပေးပို့နိုင်မှုများ

- အဆုံးအဖြတ်ကို အစမ်းလေ့ကျင့်ခြင်းဖြင့် DNS (SF‑4) နှင့် ဂိတ်ဝေး (SF‑5) မှတ်တိုင်များကို စည်းပါ
  လက်ခံသူ ဆင်းသက်ခြင်း၊ ဖြေရှင်းသူ လမ်းညွှန်ထုတ်ဝေမှုများ၊ TLS/GAR အလိုအလျောက်စနစ်နှင့် အထောက်အထားများ
  ဖမ်း။
- စတင်ခြင်းသွင်းအားစုများ (အစီအစဉ်၊ ဖိတ်ခေါ်မှု၊ တက်ရောက်မှုခြေရာခံ၊ GAR တယ်လီမီတာ
  လျှပ်တစ်ပြက်) နောက်ဆုံးပိုင်ရှင်တာဝန်များနှင့် ထပ်တူပြုပါသည်။
- အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများအတွက် စာရင်းစစ်နိုင်သော artefact အတွဲတစ်ခုကို ထုတ်လုပ်ပါ- ဖြေရှင်းသူ
  လမ်းညွှန်ထုတ်လွှတ်မှုမှတ်စုများ၊ ဂိတ်ဝေးစုံစမ်းစစ်ဆေးမှုမှတ်တမ်းများ၊ ညီညွတ်မှုကြိုးကြိုးအထွက်နှင့်
  Docs/DevRel အနှစ်ချုပ်။

## ရာထူးတာဝန်

| အလုပ်လမ်းကြောင်း | တာဝန်များ | လိုအပ်သောပစ္စည်းများ |
|--------------------|--------------------|--------------------------------|
| ကွန်ရက်ချိတ်ဆက်ခြင်း TL (DNS stack) | အဆုံးအဖြတ်ပေးသော အစီအစဉ်ကို ထိန်းသိမ်းပါ၊ RAD လမ်းညွှန်များ ထုတ်ဝေမှုများကို လုပ်ဆောင်ပါ၊ ဖြေရှင်းသူ တယ်လီမီတာ ထည့်သွင်းမှုများကို ထုတ်ဝေပါ။ | `artifacts/soradns_directory/<ts>/`၊ `docs/source/soradns/deterministic_hosts.md`၊ RAD မက်တာဒေတာ ကွာခြားချက်များ။ |
| Ops Automation Lead (တံခါးပေါက်) | TLS/ECH/GAR အလိုအလျောက် လေ့ကျင့်ခန်းများ လုပ်ဆောင်ပါ၊ `sorafs-gateway-probe` ကို run၊ PagerDuty ချိတ်များကို အပ်ဒိတ်လုပ်ပါ။ | `artifacts/sorafs_gateway_probe/<ts>/`၊ probe JSON၊ `ops/drill-log.md` ထည့်သွင်းမှုများ။ |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh` ကိုဖွင့်ပါ၊ အတွဲများကို စီမံပါ၊ Norito ကိုယ်တိုင် cert အတွဲများကို သိမ်းဆည်းပါ။ | `artifacts/sorafs_gateway_conformance/<ts>/`၊ `artifacts/sorafs_gateway_attest/<ts>/`။ |
| Docs / DevRel | မိနစ်များကို ရိုက်ကူးပါ၊ ဒီဇိုင်းအကြိုဖတ်ရန် + နောက်ဆက်တွဲများကို အပ်ဒိတ်လုပ်ပြီး ဤပေါ်တယ်တွင် အထောက်အထားအကျဉ်းချုပ်ကို ထုတ်ဝေပါ။ | အပ်ဒိတ်လုပ်ထားသော `docs/source/sorafs_gateway_dns_design_*.md` ဖိုင်များနှင့် ထုတ်ဝေမှုမှတ်စုများ။ |

## ထည့်သွင်းမှုများနှင့် ကြိုတင်လိုအပ်ချက်များ

- Deterministic host spec (`docs/source/soradns/deterministic_hosts.md`) နှင့်
  ဖြေရှင်းသူသက်သေခံချက်ငြမ်း (`docs/source/soradns/resolver_attestation_directory.md`)။
- Gateway artefacts- အော်ပရေတာလက်စွဲစာအုပ်၊ TLS/ECH အလိုအလျောက်လုပ်ဆောင်ပေးသူများ၊
  တိုက်ရိုက်မုဒ်လမ်းညွှန်ချက်၊ နှင့် `docs/source/sorafs_gateway_*` အောက်တွင် မိမိကိုယ်ကို အသိအမှတ်ပြုသည့်လုပ်ငန်းအသွားအလာ။
- Tooling: `cargo xtask soradns-directory-release`၊
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`၊
  `scripts/sorafs_gateway_self_cert.sh` နှင့် CI အကူအညီများ
  (`ci/check_sorafs_gateway_conformance.sh`၊ `ci/check_sorafs_gateway_probe.sh`)။
- လျှို့ဝှက်ချက်များ- GAR ထုတ်လွှတ်သည့်သော့၊ DNS/TLS ACME အထောက်အထားများ၊ PagerDuty လမ်းကြောင်းပြသော့၊
  ဖြေရှင်းသူ ထုတ်ယူမှုအတွက် Torii အထောက်အထား တိုကင်။

## လေယာဉ်အကြိုစစ်ဆေးရေးစာရင်း

1. အဆင့်မြှင့်တင်ခြင်းဖြင့် တက်ရောက်သူများနှင့် အစီအစဉ်များကို အတည်ပြုပါ။
   `docs/source/sorafs_gateway_dns_design_attendance.md` နှင့် ဖြန့်ကျက်ထားသည်။
   လက်ရှိအစီအစဉ် (`docs/source/sorafs_gateway_dns_design_agenda.md`)။
2. Stage artefact roots တွေဖြစ်တဲ့
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` နှင့်
   `artifacts/soradns_directory/<YYYYMMDD>/`။
3. ပြန်လည်စတင်ခြင်း (GAR manifests၊ RAD အထောက်အထားများ၊ gateway conformance အတွဲများ) နှင့်
   `git submodule` အခြေအနေသည် နောက်ဆုံးအစမ်းလေ့ကျင့်မှုတက်ဂ်နှင့် ကိုက်ညီကြောင်း သေချာပါစေ။
4. လျှို့ဝှက်ချက်များ (Ed25519 ထွက်ရှိသည့်သော့၊ ACME အကောင့်ဖိုင်၊ PagerDuty တိုကင်) တို့ကို အတည်ပြုပါ
   ပစ္စုပ္ပန်နှင့် ကိုက်ညီသော ချက်လက်မှတ်များ။
5. မီးခိုးစမ်းသပ်ခြင်း တယ်လီမီတာပစ်မှတ်များ (Pushgateway endpoint၊ GAR Grafana board) မတိုင်မီ၊
   တူးရန်။

## အလိုအလျောက် လေ့ကျင့်မှု အဆင့်များ

### Deterministic host map & RAD directory release

1. အဆိုပြုထားသော မန်နီးဖက်စ်တွင် အဆုံးအဖြတ်ပေးသော လက်ခံဆောင်ရွက်ပေးသူ ဆင်းသက်လာခြင်းကို ကူညီဆောင်ရွက်ပေးပါ။
   ပျံ့လွင့်ခြင်း မရှိဟု သတ်မှတ် အတည်ပြုသည်။
   `docs/source/soradns/deterministic_hosts.md`။
2. ဖြေရှင်းသူလမ်းညွှန်အစုအဝေးကို ဖန်တီးပါ-

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ပုံနှိပ်ထားသော လမ်းညွှန် ID၊ SHA-256 နှင့် အထွက်လမ်းကြောင်းများအတွင်း၌ မှတ်တမ်းတင်ပါ။
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` နှင့် စတင်နိုင်ခြင်း
   မိနစ်။

### DNS တယ်လီမီတာ ဖမ်းယူခြင်း။

- Tail ဖြေရှင်းသူ ပွင့်လင်းမြင်သာမှုမှတ်တမ်းများကို ≥10 မိနစ်အသုံးပြုခြင်း။
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`။
- Pushgateway မက်ထရစ်များကို ထုတ်ယူပြီး ပြေးနှင့်အတူ NDJSON လျှပ်တစ်ပြက်ရိုက်ချက်များကို သိမ်းဆည်းပါ။
  ID လမ်းညွှန်။

### Gateway အလိုအလျောက် လေ့ကျင့်မှု

1. TLS/ECH စုံစမ်းစစ်ဆေးမှုကို လုပ်ဆောင်ပါ-

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. လိုက်လျောညီထွေရှိသောကြိုးကြိုး (`ci/check_sorafs_gateway_conformance.sh`) ကို run ပါ။
   ပြန်လည်ဆန်းသစ်ရန် self-cert helper (`scripts/sorafs_gateway_self_cert.sh`)
   Norito သက်သေခံချက်အတွဲ။
3. Automation လမ်းကြောင်း အလုပ်မလုပ်ကြောင်း သက်သေပြရန် PagerDuty/Webhook ဖြစ်ရပ်များကို ဖမ်းယူပါ
   အဆုံး

### အထောက်အထားတွေ များပါတယ်။

- `ops/drill-log.md` ကို အချိန်တံဆိပ်၊ ပါဝင်သူများ၊ နှင့် စုံစမ်းစစ်ဆေးမှု hashe များဖြင့် အပ်ဒိတ်လုပ်ပါ။
- လုပ်ဆောင်ထားသော ID လမ်းညွှန်များအောက်တွင် ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းပြီး အမှုဆောင်အနှစ်ချုပ်ကို ထုတ်ပြန်ပါ။
  Docs/DevRel အစည်းအဝေး မိနစ်များတွင်။
- စတင်စစ်ဆေးခြင်းမပြုမီ အုပ်ချုပ်မှုလက်မှတ်ရှိ အထောက်အထားအစုအဝေးကို ချိတ်ဆက်ပါ။

## Session အတွက် ပံ့ပိုးကူညီမှုနှင့် အထောက်အထားများ လက်ဆင့်ကမ်းခြင်း။

- ** ထိန်းကျောင်းချိန်လိုင်း-**  
  - T‑24h — ပရိုဂရမ်စီမံခန့်ခွဲမှုသည် `#nexus-steering` တွင် သတိပေးချက် + အစီအစဉ်/တက်ရောက်မှု လျှပ်တစ်ပြက်ဓာတ်ပုံကို ပို့စ်တင်သည်။  
  - T‑2h — ကွန်ရက်ချိတ်ဆက်ခြင်း TL သည် GAR တယ်လီမီတာလျှပ်တစ်ပြက်ကို ပြန်လည်ဆန်းသစ်စေပြီး `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` တွင် မြစ်ဝကျွန်းပေါ်ဒေသများကို မှတ်တမ်းတင်သည်။  
  - T‑15m — Ops Automation သည် စုံစမ်းစစ်ဆေးရန် အဆင်သင့်ဖြစ်မှုကို အတည်ပြုပြီး `artifacts/sorafs_gateway_dns/current` တွင် လက်ရှိလည်ပတ်နေသော ID ကို ရေးပေးသည်။  
  - ဖုန်းခေါ်ဆိုမှုအတွင်း — Moderator သည် ဤ runbook ကိုမျှဝေပြီး တိုက်ရိုက်စာရေးဆရာကို တာဝန်ပေးသည်။ Docs/DevRel သည် လုပ်ဆောင်ချက် ဖိုင်များကို လိုင်းအတွင်း ဖမ်းယူပါ။
- **မိနစ် နမူနာပုံစံ-** အရိုးစုကို ကူးယူပါ။
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ပေါ်တယ်တွင်လည်း ရောင်ပြန်ဟပ်ထားသည်။
  bundle) နှင့် session တစ်ခုလျှင် ပြည့်စုံသော instance တစ်ခုကို ကတိပြုပါ။ တက်ရောက်သူ လိပ်ပါဝင်သည် ၊
  ဆုံးဖြတ်ချက်များ၊ လုပ်ဆောင်ချက်အရာများ၊ သက်သေပြချက်များနှင့် ထူးထူးခြားခြား အန္တရာယ်များ။
- **အထောက်အထား အပ်လုဒ်-** အစမ်းလေ့ကျင့်မှုမှ `runbook_bundle/` လမ်းညွှန်ကို ဇစ်ထည့်ပါ။
  ပြန်ဆိုထားသော မိနစ် PDF ကို ပူးတွဲပါ၊ SHA-256 hashe များကို မိနစ် + အစီအစဉ်တွင် မှတ်တမ်းတင်ပါ၊
  မြေယာကို အပ်လုဒ်လုပ်ပြီးသည်နှင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူအမည်ကို ping နှိပ်ပါ။
  `s3://sora-governance/sorafs/gateway_dns/<date>/`။

## အထောက်အထား လျှပ်တစ်ပြက် (၂၀၂၅ ခုနှစ် မတ်လ စတင်ခြင်း)

လမ်းပြမြေပုံနှင့် အုပ်ချုပ်မှုတွင် ကိုးကားထားသော နောက်ဆုံးအစမ်းလေ့ကျင့်မှု/တိုက်ရိုက်အရာဝတ္ထုများ
`s3://sora-governance/sorafs/gateway_dns/` ပုံးအောက်တွင် မိနစ်များနေထိုင်ပါ။ Hashes
အောက်တွင် canonical manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) ကို ကြည့်ပါ။

- **အခြောက်ပြေးခြင်း — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - tarball အတွဲ- `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - မိနစ် PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- ** တိုက်ရိုက်အလုပ်ရုံဆွေးနွေးပွဲ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(ဆိုင်းငံ့ထားသော အပ်လုဒ်- `gateway_dns_minutes_20250303.pdf` — Docs/DevRel အစုအဝေးတွင် ပြန်ဆိုထားသော PDF သည် ပြန်ဆိုထားသည်နှင့် တပြိုင်နက် SHA-256 ကို ထပ်ဖြည့်ပေးပါမည်။)_

## ဆက်စပ်ပစ္စည်း

- [Gateway operations playbook](./operations-playbook.md)
- [SoraFS ကြည့်ရှုနိုင်မှု အစီအစဉ်](./observability-plan.md)
- [ဗဟိုချုပ်ကိုင်မှုလျှော့ချထားသော DNS & Gateway ခြေရာခံသူ](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)