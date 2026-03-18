---
id: portal-publish-plan
lang: my
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
မှန်ချပ်များ `docs/source/sorafs/portal_publish_plan.md`။ အလုပ်အသွားအလာ ပြောင်းလဲသောအခါ မိတ္တူနှစ်ခုလုံးကို အပ်ဒိတ်လုပ်ပါ။
:::

Roadmap item DOCS-7 သည် docs artefact တိုင်း လိုအပ်သည် (portal build၊ OpenAPI spec၊
SBOMs) သည် SoraFS manifest ပိုက်လိုင်းမှတဆင့် စီးဆင်းပြီး `docs.sora` မှတဆင့် ဝန်ဆောင်မှုပေးရန်
`Sora-Proof` ခေါင်းစီးများဖြင့်။ ဤစစ်ဆေးမှုစာရင်းသည် ရှိပြီးသားအကူအညီများကို ပေါင်းစည်းထားသည်။
ထို့ကြောင့် Docs/DevRel၊ Storage နှင့် Ops တို့သည် အမဲလိုက်ခြင်းမရှိဘဲ ဖြန့်ချိမှုကို လုပ်ဆောင်နိုင်သည်။
runbook မျိုးစုံ။

## 1. Build & Package Payloads

ထုပ်ပိုးမှုအကူအညီကို လုပ်ဆောင်ပါ (အခြောက်လှန်းခြင်းအတွက် ရရှိနိုင်ပါသည်)

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` သည် CI ထုတ်လုပ်ပြီးပါက `docs/portal/build` ကို ပြန်သုံးသည်။
- `syft` ကို မရရှိနိုင်သောအခါတွင် `--skip-sbom` ကို ထည့်ပါ (ဥပမာ၊ လေဝင်လေထွက်ရှိသော အစမ်းလေ့ကျင့်မှု)။
- ဇာတ်ညွှန်းသည် ပေါ်တယ်စမ်းသပ်မှုများကို လုပ်ဆောင်ပြီး `portal` အတွက် CAR + manifest အတွဲများကို ထုတ်လွှတ်သည်။
  `openapi`၊ `portal-sbom` နှင့် `openapi-sbom` သည် CAR တစ်ခုစီကို မည်သည့်အချိန်တွင် စစ်ဆေးသည်
  `--proof` ကို သတ်မှတ်ပြီး `--sign` သတ်မှတ်သောအခါတွင် Sigstore အစုအဝေးများကို ချလိုက်ပါ။
- အထွက်ဖွဲ့စည်းပုံ-

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

ဖိုင်တွဲတစ်ခုလုံးကို (သို့မဟုတ် `artifacts/devportal/sorafs/latest` မှတဆင့် symlink) ထားပါ။
အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် တည်ဆောက်သည့်အရာများကို ခြေရာခံနိုင်သည်။

## 2. Pin Manifests + Aliases

မန်နီးဖက်စ်များကို Torii သို့ တွန်းပြီး နာမည်တူများကို စည်းရန် `sorafs_cli manifest submit` ကိုသုံးပါ။
`${SUBMITTED_EPOCH}` ကို နောက်ဆုံး သဘောတူညီချက် (မှ) သို့ သတ်မှတ်ပါ။
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` သို့မဟုတ် သင်၏ ဒက်ရှ်ဘုတ်)။

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- `openapi.manifest.to` အတွက် ထပ်ကျော့ပြီး SBOM သည် ထင်ရှားသည် (အတွက် alias အလံများကို ချန်လှပ်ထားပါ
  SBOM သည် အုပ်ချုပ်မှုအမည်နေရာတစ်ခု သတ်မှတ်မပေးပါက အစုအစည်းများ)။
- အခြားရွေးချယ်စရာ- `iroha app sorafs pin register` သည် တင်သွင်းမှုမှ အနှစ်ချုပ်ဖြင့် အလုပ်လုပ်သည်။
  binary ကို ထည့်သွင်းပြီးဖြစ်ပါက အကျဉ်းချုပ်။
- မှတ်ပုံတင်ခြင်းပြည်နယ်နှင့်အတူစစ်ဆေးပါ။
  `iroha app sorafs pin list --alias docs:portal --format json | jq`။
- ကြည့်ရှုရန် ဒက်ရှ်ဘုတ်များ- `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  မက်ထရစ်များ)။

## 3. Gateway Headers & Proofs

HTTP header block + binding metadata ကို ဖန်တီးပါ-

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- နမူနာပုံစံတွင် `Sora-Name`၊ `Sora-CID`၊ `Sora-Proof` နှင့်
  `Sora-Proof-Status` ခေါင်းစီးများနှင့် မူရင်း CSP/HSTS/Permissions-Policy။
- တွဲထားသော rollback ခေါင်းစီးအစုံကိုတင်ဆက်ရန် `--rollback-manifest-json` ကိုသုံးပါ။

ယာဉ်ကြောအသွားအလာကို မဖော်ပြမီ၊ ပြေးပါ-

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- စုံစမ်းစစ်ဆေးမှုတွင် GAR လက်မှတ်အသစ်အဆန်း၊ နာမည်တူမူဝါဒနှင့် TLS လက်မှတ်တို့ကို ပြဋ္ဌာန်းထားသည်။
  လက်ဗွေ
- ကိုယ်ပိုင်လက်မှတ်ကြိုးကြိုးသည် `sorafs_fetch` ဖြင့် မန်နီးဖက်စ်ကို ဒေါင်းလုဒ်လုပ်ပြီး စတိုးဆိုင်များ
  CAR ပြန်လည်ဖွင့်ခြင်းမှတ်တမ်းများ စာရင်းစစ်အထောက်အထားအတွက် ရလဒ်များကို သိမ်းဆည်းပါ။

## 4. DNS & Telemetry Guardrails

1. အုပ်ချုပ်မှုသည် စည်းနှောင်မှုကို သက်သေပြနိုင်စေရန် DNS အရိုးစုကို ပြန်လည်စတင်ပါ-

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. စတင်ထွက်စဉ်အတွင်း စောင့်ကြည့်ပါ-

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   ဒက်ရှ်ဘုတ်များ- `sorafs_gateway_observability.json`၊
   `sorafs_fetch_observability.json` နှင့် pin registry board တို့။

3. သတိပေးချက်စည်းမျဉ်းများ (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) နှင့် ဆေးလိပ်သောက်ပါ။
   ထုတ်ဝေမှုမှတ်တမ်းအတွက် မှတ်တမ်းများ/ဖန်သားပြင်ဓာတ်ပုံများကို ရိုက်ကူးပါ။

## 5. အထောက်အထားအစုအဝေး

ထုတ်ဝေခွင့်လက်မှတ် သို့မဟုတ် အုပ်ချုပ်မှုပက်ကေ့ဂျ်တွင် အောက်ပါတို့ကို ထည့်သွင်းပါ-

- `artifacts/devportal/sorafs/<stamp>/` (ကားများ၊ ဖော်ပြချက်များ၊ SBOMများ၊ အထောက်အထားများ၊
  Sigstore အစုအဝေးများ၊ အနှစ်ချုပ်များကို တင်ပြပါ)။
- Gateway probe + self-cert outputs
  (`artifacts/sorafs_gateway_probe/<stamp>/`၊
  `artifacts/sorafs_gateway_self_cert/<stamp>/`)။
- DNS skeleton + header templates (`portal.gateway.headers.txt`၊
  `portal.gateway.plan.json`၊ `portal.dns-cutover.json`)။
- ဒက်ရှ်ဘုတ် ဖန်သားပြင်ဓာတ်ပုံများ + သတိပေးချက် အသိအမှတ်ပြုချက်များ။
- `status.md` သည် manifest digest နှင့် alias binding time ကိုရည်ညွှန်းသော အပ်ဒိတ်။

ဤစစ်ဆေးမှုစာရင်းကို လိုက်နာခြင်းဖြင့် DOCS-7 ကို ပို့ဆောင်ပေးသည်- portal/OpenAPI/SBOM ပေးဆောင်မှုများသည်
`Sora-Proof` ဖြင့် စောင့်ကြပ်ထားသော နာမည်တူများဖြင့် စေ့စပ်သေချာစွာ ထုပ်ပိုးထားသည်
ခေါင်းစီးများ၊ နှင့် ရှိပြီးသား observability stack မှတဆင့် အဆုံးမှ အဆုံးအထိ စောင့်ကြည့်ခဲ့သည်။