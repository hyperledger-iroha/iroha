---
lang: my
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Governance Packet Template (Roadmap F1)

လမ်းပြမြေပုံအကြောင်းအရာအတွက် လိုအပ်သော artefact အတွဲကို ပြင်ဆင်သည့်အခါ ဤပုံစံကို အသုံးပြုပါ။
F1 (repo lifecycle documentation & tooling)။ ရည်ရွယ်ချက်မှာ ဝေဖန်သုံးသပ်သူများကို လက်ဆင့်ကမ်းရန်ဖြစ်သည်။
ထည့်သွင်းမှုတိုင်း၊ hash နှင့် အထောက်အထားအစုအဝေးတိုင်းကို ဖော်ပြသော Markdown ဖိုင်တစ်ခု
အုပ်ချုပ်ရေးကောင်စီသည် အဆိုပြုချက်တွင် ဖော်ပြထားသော ဘိုက်များကို ပြန်ဖွင့်နိုင်သည်။

> ပုံစံခွက်ကို သင်၏ကိုယ်ပိုင်အထောက်အထားလမ်းညွှန်သို့ ကူးယူပါ (ဥပမာ
> `artifacts/finance/repo/2026-03-15/packet.md`)၊ placeholder နှင့် အစားထိုးပါ။
> အောက်တွင်ဖော်ပြထားသော hashed artefacts ဘေးတွင် ၎င်းကို commit/upload လုပ်ပါ။

## 1. မက်တာဒေတာ

| လယ် | တန်ဖိုး |
|---------|-------|
| သဘောတူညီချက်/ပြောင်းလဲမှု အမှတ်အသား | `<repo-yyMMdd-XX>` |
| ပြင်ဆင်သည်/ရက်စွဲ | `<desk lead> – 2026-03-15T10:00Z` |
| သုံးသပ်သည် | `<dual-control reviewer(s)>` |
| အမျိုးအစားပြောင်း | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| စောင့်ထိန်းသူများ | `<custodian id(s)>` |
| အဆိုပြုချက်/ဆန္ဒခံယူပွဲ | `<governance ticket id or GAR link>` |
| အထောက်အထားလမ်းညွှန် | ``artifacts/finance/repo/<slug>/`` |

## 2. Instruction Payloads

စားပွဲခုံများမှ တစ်ဆင့် လက်မှတ်ထိုးပိတ်ထားသော အဆင့် Norito ညွှန်ကြားချက်များကို မှတ်တမ်းတင်ပါ။
`iroha app repo ... --output`။ ထည့်သွင်းမှုတစ်ခုစီတွင် ထုတ်လွှတ်မှု၏ hash ပါဝင်သင့်သည်။
ဖိုင်နှင့် မဲပြီးသည်နှင့် တင်သွင်းမည့် လုပ်ဆောင်မှု၏ အတိုချုံးဖော်ပြချက်
ဖြတ်သန်း

| အက် | ဖိုင် | SHA-256 | မှတ်စုများ |
|--------|------|---------|-------|
| စတင် | `instructions/initiate.json` | `<sha256>` | စားပွဲမှ အတည်ပြုထားသော ငွေသား/စရံခြေထောက်များ ပါရှိသည်။ |
| အနားသတ်ခေါ်ဆိုမှု | `instructions/margin_call.json` | `<sha256>` | ခေါ်ဆိုမှုကို အစပျိုးခဲ့သော cadence + ပါဝင်သူ ID ကို ဖမ်းယူသည်။ |
| ဖြေလျှော့ | `instructions/unwind.json` | `<sha256>` | အခြေအနေများနှင့် ကိုက်ညီပါက ခြေထောက်ပြောင်းပြန်ဖြစ်ကြောင်း သက်သေပြပါ။ |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 အုပ်ထိန်းသူ၏ အသိအမှတ်ပြုချက်များ (သုံးဖွဲ့သာ)

Repo သည် `--custodian` ကိုအသုံးပြုသည့်အခါတိုင်း ဤအပိုင်းကို ဖြည့်သွင်းပါ။ အုပ်ချုပ်မှု အစုံပဲ။
အုပ်ထိန်းသူ တစ်ဦးစီထံမှ အသိအမှတ်ပြု လက်မှတ် ရေးထိုး ချက် နှင့် ဟက်ရှ် ပါ၀င်ရမည်။
`docs/source/finance/repo_ops.md` ၏ §2.8 တွင် ကိုးကားဖော်ပြထားသော ဖိုင်။

| စောင့်မျော် | ဖိုင် | SHA-256 | မှတ်စုများ |
|-----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | အချုပ်အနှောင်ဝင်းဒိုး၊ လမ်းပြအကောင့်နှင့် ဖောက်ထွင်းအဆက်အသွယ်တို့ကို ဖုံးအုပ်ထားသည့် SLA တွင် လက်မှတ်ရေးထိုးထားသည်။ |

> အသိအမှတ်ပြုချက်ကို အခြားအထောက်အထားများ (`artifacts/finance/repo/<slug>/`) ဘေးတွင် သိမ်းဆည်းပါ
> ထို့ကြောင့် `scripts/repo_evidence_manifest.py` သည် ဖိုင်ကို တူညီသောသစ်ပင်တွင် မှတ်တမ်းတင်သည်။
> အဆင့်သတ်မှတ်ထားသော ညွှန်ကြားချက်များနှင့် ပြင်ဆင်သတ်မှတ်မှု အတိုအထွာများ။ ကြည့်ပါ။
ဖြည့်စွက်ရန်အဆင်သင့်အတွက် > `docs/examples/finance/repo_custodian_ack_template.md`
> အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထား စာချုပ်နှင့် ကိုက်ညီသော ပုံစံပုံစံ။

## 3. ဖွဲ့စည်းမှုအတိုအထွာ

အစုအဝေးပေါ်ရောက်သွားမည့် `[settlement.repo]` TOML ဘလောက်ကို ကူးထည့်ပါ (အပါအဝင်
`collateral_substitution_matrix`)။ အတိုအထွာဘေးတွင် ဟက်ရှ်ကို သိမ်းဆည်းပါ။
စာရင်းစစ်များသည် repo ဘွတ်ကင်လုပ်သောအခါတွင် အသုံးပြုခဲ့သည့် runtime မူဝါဒကို အတည်ပြုနိုင်သည်။
အတည်ပြုခဲ့သည်။

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Post-Approval Configuration Snapshots

ပြည်လုံးကျွတ်ဆန္ဒခံယူပွဲ သို့မဟုတ် အုပ်ချုပ်မှုမဲပေးပြီးနောက် `[settlement.repo]`၊
အပြောင်းအလဲကို စတင်လိုက်ပါပြီ၊ ရွယ်တူတိုင်းမှ `/v1/configuration` လျှပ်တစ်ပြက်ရိုက်ချက်များ
စာရင်းစစ်များသည် အတည်ပြုထားသောမူဝါဒသည် အစုအဝေးအတွင်း တိုက်ရိုက်ရှိကြောင်း သက်သေပြနိုင်သည် (ကြည့်ရှုပါ။
အထောက်အထား အလုပ်အသွားအလာအတွက် `docs/source/finance/repo_ops.md` §2.9)။

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| သက်တူရွယ်တူ / source | ဖိုင် | SHA-256 | အမြင့် | မှတ်စုများ |
|----------------|------|---------|-----------------|--------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | စီစဉ်သတ်မှတ်မှု စတင်ပြီးနောက် ချက်ချင်းဆိုသလို လျှပ်တစ်ပြက်ဓာတ်ပုံကို ရိုက်ကူးခဲ့သည်။ |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` သည် အဆင့်သတ်မှတ်ထားသော TOML နှင့် ကိုက်ညီကြောင်း အတည်ပြုသည်။ |

`hashes.txt` တွင် သက်တူရွယ်တူ ID များနှင့်အတူ အချေအတင်များကို မှတ်တမ်းတင်ပါ (သို့မဟုတ် ညီမျှသည်
အနှစ်ချုပ်) သို့မှသာ ပြန်လည်သုံးသပ်သူများသည် အပြောင်းအလဲကို ထည့်သွင်းထားသည့် ဆုံမှတ်များကို ခြေရာခံနိုင်မည်ဖြစ်သည်။ လျှပ်တစ်ပြက်
TOML အတိုအထွာဘေးရှိ `config/peers/` အောက်တွင် တိုက်ရိုက်နေထိုင်ပြီး ကောက်ယူမည်
`scripts/repo_evidence_manifest.py` ဖြင့် အလိုအလျောက်။

## 4. Deterministic Test Artefacts

နောက်ဆုံးထွက်ရလဒ်များကို ပူးတွဲပါ-

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

မှတ်တမ်းအစုအဝေးများအတွက် သို့မဟုတ် သင့် CI မှထုတ်လုပ်သော JUnit XML အတွက် ဖိုင်လမ်းကြောင်းများ + hashes များကို မှတ်တမ်းတင်ပါ။
စနစ်။

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| ဘဝသံသရာ အထောက်အထားမှတ်တမ်း | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` အထွက်ဖြင့် ရိုက်ကူးထားသည်။ |
| ပေါင်းစပ်စမ်းသပ်မှုမှတ်တမ်း | `tests/repo_integration.log` | `<sha256>` | အစားထိုးမှု + အနားသတ် အချိုးအစား အကျုံးဝင်မှု ပါဝင်သည်။ |

## 5. Lifecycle Proof Snapshot

ပက်ကတ်တိုင်းတွင် ထုတ်ယူထားသော အဆုံးအဖြတ်ပေးသော ဘဝသံသရာလျှပ်တစ်ပြက်ပုံ ပါဝင်ရပါမည်။
`repo_deterministic_lifecycle_proof_matches_fixture`။ ကြိုးသိုင်းကို ပြေးပါ။
ထုတ်ယူသည့်ခလုတ်များကို ဖွင့်ထားသောကြောင့် ပြန်လည်သုံးသပ်သူများသည် JSON ဘောင်ကို ကွဲပြားစေပြီး ဆန့်ကျင်စွာ ချေဖျက်နိုင်သည်။
`crates/iroha_core/tests/fixtures/` တွင် ခြေရာခံထားသော ခံစစ်မှူး (ကြည့်ရှုပါ။
`docs/source/finance/repo_ops.md` §2.7)။

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

သို့မဟုတ် တပ်ဆင်ထားသော ကိရိယာများကို ပြန်ထုတ်ပြီး သင့်ထံ ကူးယူရန် ပင်ထိုးထားသော အကူအညီကို အသုံးပြုပါ။
အဆင့်တစ်ဆင့်တွင် သက်သေအတွဲလိုက်-

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | သက်သေကြိုးကြိုးဖြင့် ထုတ်လွှတ်သော Canonical lifecycle frame |
| Digest ဖိုင် | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` မှ အကြီးစား hex digest ၊ မပြောင်းလဲသည့်တိုင် ပူးတွဲပါ ။ |

## 6. သက်သေပြချက်

စာရင်းစစ်များသည် အတည်ပြုနိုင်စေရန် အထောက်အထားလမ်းညွှန်တစ်ခုလုံးအတွက် မန်နီးဖက်စ်ကို ဖန်တီးပါ။
archive ကို မဖွင့်ဘဲ hashes လုပ်သည်။ အကူအညီပေးသူက ဖော်ပြထားသော အလုပ်အသွားအလာကို ထင်ဟပ်စေသည်။
`docs/source/finance/repo_ops.md` §3.2 တွင်

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | ဖိုင် | SHA-256 | မှတ်စုများ |
|----------|------|---------|-------|
| သက်သေအထောက်အထား | `manifest.json` | `<sha256>` | အုပ်ချုပ်မှုလက်မှတ်/ဆန္ဒခံယူပွဲမှတ်စုများတွင် checksum ကို ထည့်သွင်းပါ။ |

## 7. Telemetry & Event Snapshot

သက်ဆိုင်ရာ `AccountEvent::Repo(*)` နှင့် ဒက်ရှ်ဘုတ်များ သို့မဟုတ် CSV တစ်ခုခုကို ထုတ်ယူပါ။
`docs/source/finance/repo_ops.md` တွင် ကိုးကားထားသော ပို့ကုန်များ။ ဖိုင်များကို + မှတ်တမ်းတင်ပါ။
ဝေဖန်သုံးသပ်သူများသည် အထောက်အထားဆီသို့ တည့်မတ်စွာ ခုန်ဆင်းနိုင်စေရန် ဤနေရာတွင် hashe ရှိသည်။

| တင်ပို့ | ဖိုင် | SHA-256 | မှတ်စုများ |
|--------|------|---------|-------|
| Repo ဖြစ်ရပ်များ JSON | `evidence/repo_events.ndjson` | `<sha256>` | အကြမ်းထည် Torii ဖြစ်ရပ်စီးကြောင်းကို စားပွဲအကောင့်များသို့ စစ်ထုတ်ထားသည်။ |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo Margin အကန့်ကို အသုံးပြု၍ Grafana မှ ထုတ်ယူခဲ့သည်။ |

## 8. အတည်ပြုချက်များနှင့် လက်မှတ်များ

- **Dual-control လက်မှတ်ထိုးသူများ-** `<names + timestamps>`
- **GAR / မိနစ်အနှစ်ချုပ်-** လက်မှတ်ထိုးထားသော GAR PDF ၏ `<sha256>` သို့မဟုတ် မိနစ်ပိုင်းကို အပ်လုဒ်လုပ်ပါ။
- **သိုလှောင်မှုတည်နေရာ-** `governance://finance/repo/<slug>/packet/`

## 9. စစ်ဆေးရန်စာရင်း

ပစ္စည်းတစ်ခုစီကို ပြီးသည်နှင့် မှတ်သားပါ။

- [ ] ညွှန်ကြားချက် ပေးဆောင်ရမည့်အရာများကို အဆင့်လိုက်၊ hashed လုပ်ပြီး ပူးတွဲပါ ။
- [ ] ဖွဲ့စည်းမှုအတိုအထွာ hash မှတ်တမ်းတင်ထားသည်။
- [ ] ဖမ်းယူထားသော စမ်းသပ်မှုမှတ်တမ်းများ + hashed ။
- [ ] ဘဝသံသရာလျှပ်တစ်ပြက် + အချေအတင်ကို ထုတ်ယူပြီးပါပြီ။
- [ ] သက်သေထင်ရှားစွာထုတ်ပေးပြီး hash မှတ်တမ်းတင်ထားသည်။
- [ ] ဖမ်းယူထားသော ဖြစ်ရပ်/တယ်လီမီတာ ပို့ကုန်များ + ဟက်ဒ်။
- [ ] Dual-control အသိအမှတ်ပြုချက်များကို သိမ်းဆည်းပြီးပါပြီ။
- [ ] GAR/ မိနစ် အပ်လုဒ်လုပ်ပါ။ အပေါ်မှာ မှတ်တမ်းတင်ထားပါတယ်။

packet တိုင်းနှင့်အတူ ဤပုံစံပုံစံကို ထိန်းသိမ်းခြင်းသည် အုပ်ချုပ်မှု DAG ကို ထိန်းသိမ်းထားသည်။
အဆုံးအဖြတ်ပေးပြီး repo lifecycle အတွက် သယ်ဆောင်ရလွယ်ကူသော manifest ဖြင့် စာရင်းစစ်များကို ပံ့ပိုးပေးပါသည်။
ဆုံးဖြတ်ချက်များ။