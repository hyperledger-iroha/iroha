---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
Հայելիներ `docs/source/sorafs/portal_publish_plan.md`. Թարմացրեք երկու պատճենները, երբ աշխատանքային հոսքը փոխվի:
:::

Ճանապարհային քարտեզի DOCS-7 կետը պահանջում է փաստաթղթերի յուրաքանչյուր արտեֆակտ (պորտալի կառուցում, OpenAPI սպեցիֆիկացիա,
SBOMs) հոսելու SoraFS մանիֆեստի խողովակաշարով և ծառայելու `docs.sora`-ի միջոցով
`Sora-Proof` վերնագրերով: Այս ստուգաթերթը միավորում է գոյություն ունեցող օգնականներին
այնպես որ Docs/DevRel-ը, Storage-ը և Ops-ը կարող են գործարկել թողարկումն առանց որսի
մի քանի runbooks.

## 1. Կառուցեք և փաթեթավորեք օգտակար բեռներ

Գործարկեք փաթեթավորման օգնականը (չոր վազքի համար բաց թողնելու տարբերակները հասանելի են).

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build`-ը նորից օգտագործում է `docs/portal/build`, եթե CI-ն արդեն արտադրել է այն:
- Ավելացրեք `--skip-sbom`, երբ `syft`-ն անհասանելի է (օրինակ՝ օդային բացված փորձ):
- Սցենարը գործարկում է պորտալի թեստերը, թողարկում է CAR + մանիֆեստի զույգեր `portal`-ի համար,
  `openapi`, `portal-sbom` և `openapi-sbom`, ստուգում է յուրաքանչյուր մեքենա, երբ
  `--proof`-ը դրված է, և թողնում է Sigstore փաթեթները, երբ դրված է `--sign`:
- Արդյունքների կառուցվածքը.

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

Պահպանեք ամբողջ թղթապանակը (կամ սիմհղումը `artifacts/devportal/sorafs/latest`-ի միջոցով)
Կառավարման վերանայողները կարող են հետևել շինարարական արտեֆակտներին:

## 2. Pin Manifests + Aliases

Օգտագործեք `sorafs_cli manifest submit`՝ մանիֆեստները Torii-ի մեջ մղելու և փոխանունները կապելու համար:
Սահմանեք `${SUBMITTED_EPOCH}`-ը վերջին կոնսենսուսի դարաշրջանին (սկսած
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` կամ ձեր վահանակը):

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<katakana-i105-account-id>"
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

- Կրկնել `openapi.manifest.to`-ի և SBOM-ի մանիֆեստների համար (բաց թողեք դրոշակները
  SBOM փաթեթներ, եթե կառավարումը չի նշանակում անվանատարածք):
- Այլընտրանք. `iroha app sorafs pin register`-ն աշխատում է ներկայացվող բովանդակության հետ
  ամփոփում, եթե երկուականն արդեն տեղադրված է:
- Ստուգեք ռեեստրի վիճակը
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Դիտելու համար վահանակներ՝ `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  չափումներ):

## 3. Դարպասի վերնագրեր և ապացույցներ

Ստեղծեք HTTP վերնագրի բլոկ + պարտադիր մետատվյալներ.

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

- Կաղապարը ներառում է `Sora-Name`, `Sora-CID`, `Sora-Proof` և
  `Sora-Proof-Status` վերնագրեր գումարած լռելյայն CSP/HSTS/Permissions-Policy:
- Օգտագործեք `--rollback-manifest-json`՝ զուգակցված հետադարձ վերնագրերի հավաքածու տրամադրելու համար:

Նախքան երթևեկությունը բացահայտելը, գործարկեք՝

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Հետաքննությունն ապահովում է GAR ստորագրության թարմությունը, կեղծանունների քաղաքականությունը և TLS վկայագիրը
  մատնահետքեր.
- Ինքնահաստատման զրահը ներբեռնում է մանիֆեստը `sorafs_fetch`-ով և պահում
  Մեքենաների վերարտադրման տեղեկամատյաններ; պահպանել արդյունքները աուդիտորական ապացույցների համար:

## 4. DNS & Telemetry Guardrails

1. Թարմացրեք DNS կմախքը, որպեսզի կառավարումը կարողանա ապացուցել պարտադիր լինելը.

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Մոնիտորինգ թողարկման ընթացքում.

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Վահանակներ՝ `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` և գրանցամատյանի փին:

3. Ծխել զգոնության կանոնները (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) և
   նկարահանել տեղեկամատյաններ/սքրինշոթներ թողարկման արխիվի համար:

## 5. Ապացույցների փաթեթ

Ներառեք հետևյալը թողարկման տոմսի կամ կառավարման փաթեթում.

- `artifacts/devportal/sorafs/<stamp>/` (մեքենաներ, մանիֆեստներ, SBOM, ապացույցներ,
  Sigstore փաթեթներ, ներկայացրեք ամփոփագրեր):
- Դարպասի զոնդ + ինքնահաստատման ելքեր
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`):
- DNS կմախք + վերնագրի ձևանմուշներ (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`):
- Վահանակի սքրինշոթեր + ազդանշանային հաստատումներ:
- `status.md` թարմացում՝ հղում անելով մանիֆեստի յուրացմանը և այլանունների պարտադիր ժամանակին:

Այս ստուգաթերթին հետևելով՝ տրամադրվում է DOCS-7. պորտալ/OpenAPI/SBOM օգտակար բեռներ են.
փաթեթավորված դետերմինիստական կերպով, ամրացված այլանուններով, պաշտպանված է `Sora-Proof`-ով
վերնագրերը և վերահսկվում են ծայրից ծայր՝ գոյություն ունեցող դիտարկելիության կույտի միջոցով: