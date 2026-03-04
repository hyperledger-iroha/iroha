---
lang: my
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] ဟာ့ဒ်ဝဲ spec- 8+ cores ၊ 16 GiB RAM ၊ NVMe သိုလှောင်မှု ≥ 500 MiB/s ။
- [ ] IPv4 + IPv6 လိပ်စာနှစ်ခုကို အတည်ပြုပြီး QUIC/UDP 443 ကို အထက်စီးကြောင်းမှ ခွင့်ပြုသည်။
- [ ] Relay အထောက်အထားသော့များအတွက် HSM သို့မဟုတ် သီးခြား လုံခြုံသော အဝိုင်းကို ပံ့ပိုးပေးခြင်း။
- [ ] စင့်ခ်လုပ်ထားသော canonical opt-out catalog (`governance/compliance/soranet_opt_outs.json`)။
- [ ] လိုက်နာမှုပိတ်ဆို့ခြင်းအား သံစုံတီးဝိုင်းဖွဲ့စည်းပုံသို့ ပေါင်းစည်းပါ (`03-config-example.toml` ကိုကြည့်ပါ)။
- [ ] တရားစီရင်ပိုင်ခွင့်/ ကမ္ဘာလုံးဆိုင်ရာ လိုက်နာမှု အထောက်အထားများကို ဖမ်းယူပြီး `attestations` စာရင်းကို ဖြည့်သွင်းပါ။
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (သို့မဟုတ် သင်၏ တိုက်ရိုက်လျှပ်တစ်ပြက်ရိုက်ချက်) ကိုဖွင့်ပြီး pass/fail အစီရင်ခံစာကို ပြန်လည်သုံးသပ်ပါ။
- [ ] ထပ်ဆင့်ဝန်ခံခြင်း CSR ကိုထုတ်လုပ်ပြီး အုပ်ချုပ်မှုဆိုင်ရာလက်မှတ်ထိုးထားသောစာအိတ်ကိုရယူပါ။
- [ ] guard descriptor မျိုးစေ့ကို တင်သွင်းပြီး ထုတ်ဝေထားသော hash ကွင်းဆက်ကို စစ်ဆေးပါ။
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` ကိုဖွင့်ပါ။
- [ ] Dry-run telemetry တင်ပို့ခြင်း- Prometheus ခြစ်ရာဒေသတွင် အောင်မြင်ကြောင်း သေချာပါစေ။
- [ ] brownout drill window ကို အချိန်ဇယားဆွဲပြီး တိုးမြင့်လာသည့် အဆက်အသွယ်များကို မှတ်တမ်းတင်ပါ။
- [ ] လေ့ကျင့်ရေးအထောက်အထားအတွဲ- `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json` ကို လက်မှတ်ရေးထိုးပါ။