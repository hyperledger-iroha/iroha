---
id: preview-host-exposure
lang: hy
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

DOCS‑SORA ճանապարհային քարտեզը պահանջում է յուրաքանչյուր հանրային նախադիտում՝ նույնով վարելու համար
ստուգիչ գումարի կողմից հաստատված փաթեթ, որը վերանայողները օգտագործում են տեղական մակարդակում: Օգտագործեք այս գրքույկը
այն բանից հետո, երբ գրախոսի մուտքագրումը (և հրավերի հաստատման տոմսը) ավարտված է
բետա նախադիտման հյուրընկալողը առցանց:

## Նախադրյալներ

- Գրախոսի ներբեռնման ալիքը հաստատվել և մուտք է գործել նախադիտման որոնիչ:
- Վերջին պորտալի կառուցումը ներկայացված է `docs/portal/build/`-ի և ստուգիչ գումարի ներքո
  ստուգված (`build/checksums.sha256`):
- SoraFS նախադիտման հավատարմագրերը (Torii URL, հեղինակություն, մասնավոր բանալի, ներկայացված է
  դարաշրջան) պահվում է կամ շրջակա միջավայրի փոփոխականներում կամ JSON կոնֆիգուրացիաներում, ինչպիսիք են
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json):
- DNS-ի փոփոխման տոմս, որը բացվել է ցանկալի հյուրընկալողի անունով (`docs-preview.sora.link`,
  `docs.iroha.tech` և այլն) գումարած հերթապահ կոնտակտներ:

## Քայլ 1 – Կառուցեք և հաստատեք փաթեթը

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Ստուգման սկրիպտը հրաժարվում է շարունակել, երբ ստուգիչ գումարի մանիֆեստը բացակայում է կամ
կեղծված՝ յուրաքանչյուր նախադիտման արտեֆակտ ստուգված պահելով:

## Քայլ 2 – Փաթեթավորեք SoraFS արտեֆակտները

Ստատիկ կայքը փոխակերպեք դետերմինիստական CAR/մանիֆեստ զույգի: `ARTIFACT_DIR`
կանխադրված է `docs/portal/artifacts/`:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

Կցեք ստեղծված `portal.car`, `portal.manifest.*`, նկարագրիչը և ստուգիչ գումարը
դրսևորել նախադիտման ալիքի տոմսին:

## Քայլ 3 – Հրապարակեք նախադիտման կեղծանունը

Կրկին գործարկեք փին օգնականը **առանց** `--skip-submit`-ի, երբ պատրաստ լինեք բացահայտելու
հյուրընկալողը. Տրամադրեք կամ JSON կազմաձևը կամ բացահայտ CLI դրոշակները.

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Հրամանը գրում է `portal.pin.report.json`,
`portal.manifest.submit.summary.json` և `portal.submit.response.json`, որոնք
պետք է առաքվի հրավերի ապացույցների փաթեթով:

## Քայլ 4 – Ստեղծեք DNS կտրման պլան

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

Ստացված JSON-ը տարածեք Ops-ի հետ, որպեսզի DNS անջատիչը ճշգրիտ հղում կատարի
բացահայտ մարսողություն. Ավելի վաղ նկարագրիչը որպես վերադարձի աղբյուր օգտագործելիս,
հավելված `--previous-dns-plan path/to/previous.json`:

## Քայլ 5 – Հետազոտեք տեղակայված հոսթը

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Հետաքննությունը հաստատում է մատուցված թողարկման պիտակը, CSP վերնագրերը և ստորագրության մետատվյալները:
Կրկնեք հրամանը երկու շրջաններից (կամ կցեք գանգուր ելքը), որպեսզի աուդիտորները կարողանան տեսնել
որ եզրային քեշը տաք է։

## Ապացույցների փաթեթ

Ներառեք հետևյալ արտեֆակտները նախադիտման ալիքի տոմսում և վերաբերեք դրանց
հրավերի էլ.

| Արտեֆակտ | Նպատակը |
|----------|---------|
| `build/checksums.sha256` | Ապացուցում է, որ փաթեթը համապատասխանում է CI կառուցվածքին: |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Canonical SoraFS օգտակար բեռ + մանիֆեստ: |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Ցուցադրում է մանիֆեստի ներկայացումը + կեղծանունը հաջողվել է: |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS մետատվյալներ (տոմս, պատուհան, կոնտակտներ), երթուղու առաջխաղացում (`Sora-Route-Binding`) ամփոփում, `route_plan` ցուցիչ (պլան JSON + վերնագրի ձևանմուշներ), քեշի մաքրման տեղեկություններ և հետադարձ ցուցումներ Ops-ի համար: |
| `artifacts/sorafs/preview-descriptor.json` | Ստորագրված նկարագրիչ, որը կապում է արխիվը + ստուգիչ գումարը: |
| `probe` ելք | Հաստատում է, որ կենդանի հաղորդավարը գովազդում է ակնկալվող թողարկման պիտակը: |

Հաղորդավարի ուղիղ հեռարձակումից հետո հետևեք [նախադիտել հրավիրատոմսի գրքույկը] (./public-preview-invite.md)
հղումը տարածելու, հրավերների գրանցումը և հեռաչափությունը վերահսկելու համար: