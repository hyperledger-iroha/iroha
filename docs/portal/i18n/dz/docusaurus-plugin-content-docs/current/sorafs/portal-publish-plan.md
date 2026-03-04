---
id: portal-publish-plan
lang: dz
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

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
མེ་ལོང་ `docs/source/sorafs/portal_publish_plan.md`. ལཱ་གི་རྒྱུན་རིམ་བསྒྱུར་བཅོས་འགྱོཝ་ད་ འདྲ་བཤུས་གཉིས་ཆ་རང་དུས་མཐུན་བཟོ་དགོ།
:::

རོ་ཌི་མེཔ་ཅ་ཆས་ DOCS-7 ལུ་ ཡིག་ཆ་རེ་རེ་དགོཔ་ཨིན།
SBOMs) SoraFS བརྒྱུད་དེ་ བཞུར་ནིའི་དོན་ལུ་ གསལ་སྟོན་འབད་དེ་ I18NI0000013X བརྒྱུད་དེ་ ཕྱག་ཞུ་དོ་ཡོདཔ་ཨིན།
I18NI000000014X མགོ་ཡིག་དང་མཉམ་དུ། ཞིབ་དཔྱད་ཐོ་ཡིག་འདི་གིས་ ད་ལྟོ་ཡོད་པའི་གྲོགས་རམ་པ་ཚུ་གཅིག་ཁར་བཙུགསཔ་ཨིན།
དེ་འབདཝ་ལས་ Docs/DevRel, Storage, དང་ Ops ཚུ་གིས་ འཚོལ་ཞིབ་མ་འབད་བར་ བཏོན་གཏང་ཚུགས།
རན་དེབ་མང་པོ།

## 1. བཟོ་བསྐྲུན་དང་ཐུམ་སྒྲིལ་གྱི་གླ་ཆ་ཚུ།

ཐུམ་སྒྲིལ་གྱི་གྲོགས་རམ་པ་ (སྐརམ་གྱི་གདམ་ཁ་ཚུ་ སྐམ་རན་ཚུ་གི་དོན་ལུ་འཐོབ་ཚུགས།):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` གིས་ I18NI000000016X གིས་ སི་ཨའི་གིས་ ཧེ་མ་ལས་ བཟོ་བསྐྲུན་འབད་བ་ཅིན་ ལོག་སྟེ་ལག་ལེན་འཐབ་ཨིན།
- `--skip-sbom` འདི་ I18NI000000018X མེད་པའི་སྐབས་ ཁ་སྐོང་རྐྱབས་ (དཔེར་ན་ རླུང་གིས་ གུག་གུགཔ་འབད་མི་ བསྐྱར་གསོ་)
- ཡིག་ཚུགས་འདི་གིས་ དྲྭ་ཐོག་བརྟག་དཔྱད་ཚུ་གཡོག་བཀོལ་དོ་ཡོདཔ་ད་ འདི་གིས་ CAR + གསལ་སྟོན་ཆ་གཅིག་འདི་ `portal` གི་དོན་ལུ་ བཏོནམ་ཨིན།
  I18NI000000020X, `portal-sbom`, དང་ I18NI000000022X, གིས་ CAR རེ་རེ་བཞིན་ ག་དུས་བདེན་དཔང་འབདཝ་ཨིན།
  I18NI00000000023X གཞི་སྒྲིག་འབད་ཡོདཔ་དང་ I18NI000000024X གཞི་སྒྲིག་འབད་བའི་སྐབས་ Sigstore བང་རིམ་ཚུ་བཏོནམ་ཨིན།
- ཐོན་འབྲས་གཞི་བཀོད་:

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

སྣོད་འཛིན་ཆ་མཉམ་ (ཡང་ན་ `artifacts/devportal/sorafs/latest` བརྒྱུད་དེ་) དེ་འབདཝ་ལས་ དེ་སྦེ་བཞག་དགོ།
གཞུང་སྐྱོང་བསྐྱར་ཞིབ་འབད་མི་ཚུ་གིས་ ཅ་ཆས་བཟོ་ཚུགས།

## 2. པིན་མན་ངག་ + མི།

I18NI000000026X ལག་ལེན་འཐབ། མངོན་གསལ་ཚུ་ Torii ནང་ལུ་ བསྐུལ་མ་འབད་དེ་ མིང་ཚིག་ཚུ་ བསྡམ་དགོ།
`${SUBMITTED_EPOCH}` འཕྲལ་གྱི་མོས་མཐུན་གྱི་དུས་སྐབས་ལུ་གཞི་སྒྲིག་འབད།
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` ཡང་ན་ ཁྱོད་ཀྱི་ཌེཤ་བོརཌ་)།

I18NF0000008X

- I18NI000000029X གི་དོན་ལུ་བསྐྱར་ལོག་དང་ SBOM གིས་ གསལ་སྟོན་འབདཝ་ཨིན་ (མིང་གཞན་དར་ཆ་ཚུ་ for for for to the for the for the to the for the for the for the for the for the for the for the 20
  གཞུང་སྐྱོང་གིས་ མིང་ས་སྒོ་ཅིག་འགན་སྤྲོད་མ་འབད་ཚུན་ ཨེསི་བི་ཨོ་ཨེམ་བཱན་ཌལ་ཚུ་)།
- གདམ་ཁ་: I18NI000000030X ཕུལ་མི་ལས་ ཟས་བཅུད་དང་གཅིག་ཁར་ལཱ་འབདཝ་ཨིན།
  གཉིས་ལྡན་འདི་ཧེ་མ་ལས་རང་གཞི་བཙུགས་འབད་དེ་ཡོད་པ་ཅིན་བཅུད་བསྡུས།
- དང་བཅས་ ཐོ་བཀོད་གནས་སྟངས་བདེན་དཔྱད་འབད།
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- ཌེཤ་བོརཌ་བལྟ་ནི་: `sorafs_pin_registry.json` (I18NI000000033X)
  metrics).

## 3. གྷེཊ་ཝེ་ མགོ་ཡིག་དང་བདེན་དཔང་།

ཨེཆ་ཊི་ཊི་པི་མགོ་ཡིག་སྡེབ་ཚན་ + བཱའིན་ཌིང་མེ་ཊ་ཌེ་ཊ་: བཏོན་གཏང་།

I18NF0000009X

- ཊེམ་པེལེཊ་ནང་ `Sora-Name`, I18NI000000035X, I18NI000000036X, དང་།
  `Sora-Proof-Status` མགོ་ཡིག་ཚུ་དང་ སྔོན་སྒྲིག་སི་ཨེསི་པི་/ཨེཆ་ཨེསི་ཊི་ཨེསི་/པེར་མརསི་-སྲིད་བྱུས།
- ཆ་སྒྲིག་འབད་ཡོད་པའི་བཤུད་མགོ་ཆ་ཚན་ཅིག་བཟོ་ནི་ལུ་ `--rollback-manifest-json` ལག་ལེན་འཐབ།

འགྲུལ་སྐྱོད་ཕྱིར་བཏོན་མ་འབད་བའི་ཧེ་མ་ གཡོག་བཀོལ།

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- འཚོལ་ཞིབ་འདི་གིས་ གཱར་མིང་རྟགས་གསརཔ་དང་ མིང་གཞན་སྲིད་བྱུས་ དེ་ལས་ ཊི་ཨེལ་ཨེསི་ལག་ཁྱེར་ཚུ་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
  མཛུབ་མོའི་པར་རིས།
- རང་གིས་རང་གི་ལག་ཁྱེར་གྱི་ དམ་སྦྱིས་འདི་གིས་ `sorafs_fetch` དང་ཚོང་ཁང་ཚུ་གི་ཐོག་ལས་ གསལ་སྟོན་འདི་ཕབ་ལེན་འབདཝ་ཨིན།
  CAR བསྐྱར་རྩེད་དྲན་ཐོ་ཚུ་; རྩིས་ཞིབ་སྒྲུབ་བྱེད་ཀྱི་ཐོན་འབྲས་ཚུ་བཞག་དགོ།

## 4. DNS & Telemetry Guardrails

༡ ཌི་ཨེན་ཨེསི་ཀེང་རུས་གསརཔ་འདི་གིས་ གཞུང་སྐྱོང་འདི་གིས་ བཱའིན་ཌིང་འདི་ བདེན་ཁུངས་བཀལ་ཚུགས།

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. འགོ་བཙུགས་སྐབས་བལྟ་རྟོག་པ།

   - `torii_sorafs_alias_cache_refresh_total`
   - I18NI0000041X
   - I18NI0000042X / `_failures_total`

   ཌེཤ་བོརཌ་: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json`, དང་ པིན་ཐོ་བཀོད་བང་ཡིག།

3. ཉེན་བརྡའི་སྒྲིག་གཞི་ (I18NI0000046X) དང་།
   བཟུང་བའི་དྲན་ཐོ་/གསར་བཏོན་ཡིག་མཛོད་ཀྱི་དོན་ལུ་ གསལ་གཞི་ཚུ།

## 5. སྒྲུབ་བྱེད་ཀྱི་བསྡམས།

གསར་བཏོན་གྱི་ ཤོག་འཛིན་ཡང་ན་ གཞུང་སྐྱོང་ཐུམ་སྒྲིལ་ནང་ གཤམ་གསལ་ཚུ་ བཙུགས་ནི།

- I18NI00000047 (CARs, གསལ་སྟོན་, SBOMs, བདེན་དཔང་།
  Sigstore བཱན་ཌལ་ཚུ་, བཅུད་བསྡུས་བཙུགས།)
- སྒོ་ཁའི་འཚོལ་ཞིབ་ + རང་དོན་ལག་ཁྱེར་ཐོན་འབྲས་ཚུ།
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  I18NI000004X).
- DNS skeleton + མགོ་ཡིག་ཊེམ་པེལེཊི་ (I18NI000000050X,
  `portal.gateway.plan.json`, I18NI0000052X).
- ཌེཤ་བོརཌི་གསལ་གཞི་ཚུ་ + ཉེན་བརྡ་ཚུ་ཉེན་བརྡ་འབདཝ་ཨིན།
- `status.md` གིས་ གསལ་སྟོན་དང་ མིང་གཞན་ བཱའིན་ཌིང་དུས་ཚོད་ལུ་ གཞི་བསྟུན་འབད་ནི་ དུས་མཐུན་བཟོ་ཡོདཔ།

འདི་གི་ཤུལ་ལས་ བརྟག་ཞིབ་ཐོ་ཡིག་འདི་ DOCS-7: དྲྭ་ཚིགས་/OpenAPI/SBOM པེ་ལོཌི་ཚུ་ ༡.
ཐུམ་སྒྲིལ་ཅན་གྱི་གཏན་འབེབས་ལྟར་ན། མིང་གཞན་དང་མཉམ་དུ་བསྡམས་ཡོད། I18NI000000054X གིས་སྲུང་ཡོད།
མགོ་ཡིག་ཚུ་དང་ ད་ལྟོ་ཡོད་པའི་ བལྟ་བརྟོག་འབད་ཚུགས་པའི་ བང་རིམ་བརྒྱུད་དེ་ མཇུག་ལས་མཇུག་ཚུན་ཚོད་ ལྟ་རྟོག་འབད་ཡོདཔ་ཨིན།