---
lang: my
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage Packet Template (SN13-C)

လမ်းပြမြေပုံပါ ပစ္စည်း **SN13-C — Manifests & SoraNS ကျောက်ဆူး** သည် နာမည်ပွားတိုင်း လိုအပ်သည်
အဆုံးအဖြတ်ရှိသော အထောက်အထားအစုအဝေးကို ပို့ဆောင်ရန် လှည့်ခြင်း။ ဤပုံစံပြားကို သင့်ထံကူးယူပါ။
artefact directory (ဥပမာ
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) နှင့် အစားထိုးပါ။
packet ကို အုပ်ချုပ်မှုသို့ မတင်ပြမီ placeholder များ။

## 1. မက်တာဒေတာ

| လယ် | တန်ဖိုး |
|---------|-------|
| ပွဲ ID | `<taikai.event.launch-2026-07-10>` |
| တိုက်ရိုက်/ပြန်ဆို | `<main-stage>` |
| Alias ​​namespace/name | `<sora / docs>` |
| အထောက်အထားလမ်းညွှန် | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| အော်ပရေတာ ဆက်သွယ်ရန် | `<name + email>` |
| GAR / RPT လက်မှတ် | `<governance ticket or GAR digest>` |

## အစုအဝေးအကူအညီပေးသူ (ချန်လှပ်ထားနိုင်သည်)

spool artefacts ကို ကူးယူပြီး JSON (ချန်လှပ်ထားနိုင်သည်) အကျဉ်းချုပ်ကို ထုတ်လွှတ်ပါ။
ကျန်အပိုင်းများကိုဖြည့်ပါ

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

အကူအညီပေးသူက `taikai-anchor-request-*`၊ `taikai-trm-state-*`၊
Taikai spool လမ်းညွှန်မှ `taikai-lineage-*`၊ စာအိတ်များနှင့် ကင်းစောင့်များ
(`config.da_ingest.manifest_store_dir/taikai`) ဆိုတော့ အထောက်အထား ဖိုင်တွဲ ပြီးသွားပြီ
အောက်တွင်ကိုးကားထားသော ဖိုင်အတိအကျပါရှိသည်။

## 2. မျိုးရိုး လယ်ဂျာနှင့် အရိပ်အမြွက်

on-disk lineage လယ်ဂျာနှင့် JSON Torii ရေးထားသော အရိပ်အမြွက်ကို ပူးတွဲပါ
ပြတင်းပေါက်။ ဒါတွေက တိုက်ရိုက်လာတာပါ။
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` နှင့်
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`။

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| မျိုးရိုးလယ်ဂျာ | `taikai-trm-state-docs.json` | `<sha256>` | ယခင် manifest digest/window ကို သက်သေပြပါ။ |
| မျိုးရိုး အရိပ်အမြွက် | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS ကျောက်ဆူးသို့ မတင်မီ ရိုက်ကူးထားသည်။ |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Anchor payload ဖမ်းယူခြင်း။

ကျောက်ဆူးဝန်ဆောင်မှုသို့ Torii ပေးပို့သည့် POST ဝန်ဆောင်အား မှတ်တမ်းတင်ပါ။ ဝန်ဆောင်ခ
`envelope_base64`၊ `ssm_base64`၊ `trm_base64` နှင့် inline ပါဝင်သည်
`lineage_hint` အရာဝတ္ထု; အရိပ်အမြွက်သက်သေပြရန် စာရင်းစစ်များသည် ဤဖမ်းယူမှုကို အားကိုးသည်။
SoraNS သို့ ပေးပို့ခဲ့သည်။ ယခု Torii သည် ဤ JSON ကို အလိုအလျောက်ရေးသည်။
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool directory (`config.da_ingest.manifest_store_dir/taikai/`) အတွင်းမှာ
အော်ပရေတာများသည် HTTP မှတ်တမ်းများကို ဖျက်မည့်အစား ၎င်းကို တိုက်ရိုက်ကူးယူနိုင်သည်။

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | `taikai-anchor-request-*.json` (Taikai spool) မှ ကူးယူထားသော တောင်းဆိုချက် အကြမ်း။ |

## 4. အသိအမှတ်ပြုချက်ကို ထုတ်ဖော်ပြသပါ။

| လယ် | တန်ဖိုး |
|---------|-------|
| manifest digest | အသစ် `<hex digest>` |
| ယခင် manifest digest (အရိပ်အမြွက်မှ) | `<hex digest>` |
| Window start/end | `<start seq> / <end seq>` |
| လက်ခံချိန်တံဆိပ် | `<ISO8601>` |

အထက်တွင်မှတ်တမ်းတင်ထားသော လယ်ဂျာ/အရိပ်အမြွက်များကို ကိုးကား၍ ပြန်လည်သုံးသပ်သူများမှ အတည်ပြုနိုင်စေရန်
အစားထိုးထားသော ပြတင်းပေါက်။

## 5. Metrics / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` လျှပ်တစ်ပြက်- `<Prometheus query + export path>`
- `/status taikai_alias_rotations` အမှိုက်ပုံကြီး (အမည်တူတစ်ခုလျှင်): `<file path + hash>`

ကောင်တာကိုပြသသော Prometheus/Grafana သို့မဟုတ် `curl` အထွက်အား ပံ့ပိုးပါ
တိုးမြှင့်ခြင်းနှင့် ဤအမည်တူအတွက် `/status` အခင်းအကျင်း။

## 6. အထောက်အထားလမ်းညွှန်အတွက် မန်နီးဖက်စ်

အထောက်အထား လမ်းညွှန်၏ အဆုံးအဖြတ် ထင်ရှားသော သရုပ်ကို ဖန်တီးပါ (spool ဖိုင်များ၊
payload capture၊ metrics snapshots) ထို့ကြောင့် အုပ်ချုပ်ရေးသည် hash တိုင်းကို စစ်ဆေးနိုင်သည်။
archive ကို ထုပ်ပိုးခြင်း

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| သက်သေအထောက်အထား | `manifest.json` | `<sha256>` | ၎င်းကို အုပ်ချုပ်မှုပက်ကတ်/GAR တွင် ပူးတွဲပါ။ |

## 7. စစ်ဆေးရန်စာရင်း

- [ ] မျိုးရိုးလယ်ဂျာကို ကူးယူ + တွဲထားသည်။
- [ ] မျိုးရိုးအရိပ်အမြွက်ကို ကူးယူသည် + hashed ။
- [ ] Anchor POST payload ဖမ်းပြီး hashed။
- [ ] ဖြည့်သွင်းထားသော ဇယားကွက်ကို ဖော်ပြပါ။
- [ ] မက်ထရစ် လျှပ်တစ်ပြက်ပုံများ (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifest ကို `scripts/repo_evidence_manifest.py` ဖြင့် ထုတ်လုပ်သည်။
- [ ] အထုပ်ကို hashes + contact information ဖြင့် အုပ်ချုပ်မှုသို့ အပ်လုဒ်လုပ်ထားသည်။

အလှည့်အပြောင်းတိုင်းအတွက် ဤပုံစံပုံစံကို ထိန်းသိမ်းခြင်းသည် SoraNS အုပ်ချုပ်မှုကို ထိန်းသိမ်းထားသည်။
မျိုးပွားနိုင်သောအစုအဝေးနှင့် မျိုးရိုးဆက်နွယ်မှုဆိုင်ရာ အရိပ်အမြွက်များကို GAR/RPT အထောက်အထားများနှင့် တိုက်ရိုက်ချိတ်ဆက်ထားသည်။