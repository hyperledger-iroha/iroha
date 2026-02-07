---
lang: my
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e17a07b8d98725e24b75710496b0b69b6d8878160c7f12884e6e1eef0c0a4af7
source_last_modified: "2026-01-03T19:37:11.140795+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CI Cookbook
summary: Reference GitHub Actions workflow bundling sign + verify steps with review notes.
translator: machine-google-reviewed
---

#SoraFS CI ဟင်းချက်စာအုပ်

ဤအတိုအထွာသည် `docs/source/sorafs_ci_templates.md` တွင် လမ်းညွှန်ချက်ကို ထင်ဟပ်စေသည်
လက်မှတ်ရေးထိုးခြင်း၊ အတည်ပြုခြင်းနှင့် အထောက်အထားစစ်ဆေးမှုများကို မည်သို့ပေါင်းစပ်ရမည်ကို သရုပ်ပြသည်။
GitHub Actions အလုပ်တစ်ခုတည်း။

```yaml
name: sorafs-cli-release

on:
  push:
    branches: [main]

permissions:
  contents: read
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-rust@v1
        with:
          rust-version: 1.92

      - name: Package payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack \
            --input payload.bin \
            --car-out artifacts/payload.car \
            --plan-out artifacts/chunk_plan.json \
            --summary-out artifacts/car_summary.json
          sorafs_cli manifest build \
            --summary artifacts/car_summary.json \
            --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/manifest.to \
            --chunk-plan artifacts/chunk_plan.json \
            --bundle-out artifacts/manifest.bundle.json \
            --signature-out artifacts/manifest.sig \
            --identity-token-provider=github-actions \
            --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature \
            --manifest artifacts/manifest.to \
            --bundle artifacts/manifest.bundle.json \
            --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify \
            --manifest artifacts/manifest.to \
            --car artifacts/payload.car \
            --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## မှတ်ချက်

- `sorafs_cli` ကို အပြေးသမားတွင် ရနိုင်ရမည် (ဥပမာ၊ `cargo install --path crates/sorafs_car --features cli` ဤအဆင့်များမတိုင်မီ)။
- အလုပ်အသွားအလာသည် ရှင်းလင်းပြတ်သားသော OIDC ပရိသတ်ကို ပေးဆောင်ရမည် (ဤနေရာတွင် `sorafs`); သင်၏ Fulcio မူဝါဒနှင့် ကိုက်ညီစေရန် `--identity-token-audience` ကို ချိန်ညှိပါ။
- လွှတ်တင်ရေးပိုက်လိုင်းသည် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ရန်အတွက် `artifacts/manifest.bundle.json`၊ `artifacts/manifest.sig` နှင့် `artifacts/proof.json` ကို သိမ်းဆည်းထားသင့်သည်။
- အဆုံးအဖြတ်ပေးသော နမူနာလက်ရာများသည် `fixtures/sorafs_manifest/ci_sample` တွင် နေထိုင်သည်; ရွှေရောင်ဖော်ပြချက်များ၊ အတုံးအခဲအစီအမံများ သို့မဟုတ် ပိုက်လိုင်းကို ပြန်လည်တွက်ချက်ခြင်းမပြုဘဲ JSON အတွဲလိုက် လိုအပ်သည့်အခါ ၎င်းတို့ကို စမ်းသပ်မှုအဖြစ် ကူးယူပါ။

## Fixture Verification

ဤလုပ်ငန်းအသွားအလာအတွက် အဆုံးအဖြတ်ပေးသည့်အရာများ အောက်တွင် နေထိုင်ပါသည်။
`fixtures/sorafs_manifest/ci_sample`။ ပိုက်လိုင်းများသည် အထက်ဖော်ပြပါ အဆင့်များနှင့် ပြန်ဖွင့်နိုင်သည်။
ဥပမာ- Canonical ဖိုင်များနှင့် ၎င်းတို့၏ အထွက်များ ကွဲပြားသည်-

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

အချည်းနှီးသောကွဲပြားမှုများသည် ထုတ်လုပ်ထားသော ဘိုက်-ထပ်တူကျသော သရုပ်ပြမှုများ၊ အစီအစဉ်များနှင့် တည်ဆောက်မှုကို အတည်ပြုသည်။
လက်မှတ်အစုအဝေးများ။ အပြည့်အစုံအတွက် `fixtures/sorafs_manifest/ci_sample/README.md` ကိုကြည့်ပါ။
မှတ်တမ်းစာရင်းနှင့် ဖမ်းယူထားသော ထုတ်ဝေမှုမှတ်စုများကို ပုံစံထုတ်ခြင်းဆိုင်ရာ အကြံပြုချက်များ
အနှစ်ချုပ်