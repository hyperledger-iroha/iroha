---
lang: ba
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

:::иҫкәртергә канонлы сығанаҡ
Көҙгөләр `docs/source/sorafs/portal_publish_plan.md`. Эш ағымы үҙгәргәндә ике күсермәһен дә яңыртығыҙ.
::: 1990 й.

Юл картаһы пункты DOCS-7 һәр docs артефакт талап итә (порталь төҙөү, I18NT000000002X спец.
SBOMs) аша ағым I18NT000000000004X манифест торбаһы һәм хеҙмәт итеү аша I18NI000000013X .
менән `Sora-Proof` башлыҡтары. Был тикшерелгән исемлектә булған ярҙамсыларҙы бергә һыҙып .
Шулай итеп, Docs/DevRel, Һаҡлау, һәм Ops аша һунар итеүһеҙ релиз йүгерә ала
бер нисә runbooks.

## 1. Төҙөү & Пакет Түләүҙәр

Ҡаплау ярҙамсыһын эшләгеҙ (киҫеп варианттары ҡоро-йүгерә өсөн мөмкин):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` ҡабаттан ҡулланыла I18NI0000000016X, әгәр CI инде уны етештерә.
- `--skip-sbom` өҫтәү ҡасан I18NI000000018X юҡ (мәҫәлән, һауа-гапп репетицияһы).
- Сценарий портал һынауҙарын үткәрә, I18NI000000019X өсөн CAR + манифест парҙарын сығара,
  I18NI000000020X, `portal-sbom`, һәм `openapi-sbom`, һәр CAR раҫлай, ҡасан .
  `--proof` ҡуйылған, һәм I18NI00000000024ХХХ ҡасан ҡуйылған I18NT00000000000000000000000000000000023.
- Сығыш структураһы:

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

Бөтә папканы (йәки I18NI000000025X аша симлинк) шулай тотоғоҙ
идара итеү рецензиялары артефакттар төҙөү эҙләй ала.

## 2. Пин-Кинифестәр + псевдоним

Ҡулланыу I18NI000000026X өсөн этәрергә манифесттар I18NT000000005X һәм псевдонимдарҙы бәйләү.
I18NI000000027X комплекты һуңғы консенсус эпохаһына (1000).
`curl -s "${TORII_URL}/v2/status" | jq '.sumeragi.epoch'` йәки һеҙҙең приборҙар таҡтаһы).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v2/status | jq '.sumeragi.epoch')"

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

- I18NI000000029X һәм SBOM өсөн ҡабатлау өсөн ҡабатлауҙар (2012 йыл өсөн псевдонимдар флагтары иҙеүсе.
  SBOM өйөмдәре, әгәр идара итеү исемдәр киңлеген бирмәһә).
- Альтернатива: I18NI00000000030X тапшырыуҙан дайджест менән эшләй
  резюме, әгәр бинар инде ҡуйылған.
- ЗАГС-тың 2012 йыл менән тикшерергә.
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Ҡарау өсөн приборҙар таҡтаһы: I18NI0000000032X (I18NI000000033X
  метрикаһы).

## 3. Ҡапҡа башлыҡтары һәм дәлилдәре

HTTP башлыҡ блогын генерациялау + бәйләүсе метамағлүмәттәр:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
``` X

- Шаблон I18NI000000034X, I18NI000000035X, I18NI000000036X, һәм
  I18NI000000037X башлыҡтары плюс ғәҙәттәгесә CSP/HSTS/Рөхсәт-сәйәси.
- I18NI000000038X Xҡ ҡулланыу, парлы кире ҡайтарыу өсөн башлыҡ комплектын күрһәтеү өсөн.

Трафикты фашлағансы, йүгерергә:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- зонд GAR ҡултамғаһы яңылыҡ, псевдоним сәйәсәте, һәм TLS cert .
  бармаҡ эҙҙәре.
- Үҙ-үҙеңде сертификат жгут скачать манифест менән I18NI0000000039X һәм магазиндар .
  CAR реплей журналдары; аудит дәлилдәре өсөн сығыштарҙы һаҡларға.

## 4. DNS & Телеметрия гвардияһы

1. DNS скелетын яңыртыу, шулай итеп, идара итеү мотлаҡ иҫбатлай ала:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Прокат ваҡытында монитор:

   - I18NI000000040X
   - I18NI000000041X
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Приборҙар таҡталары: `sorafs_gateway_observability.json`, .
   I18NI000000045X, ә булавка реестры таҡтаһы.

3. Уяулыҡ ҡағиҙәләрен төтөн (I18NI000000046X) һәм
   алыу журналдары/скриншоттар өсөн сығарыу архивы.

## 5. Дәлилдәр йыйылмаһы

Түбәндәгеләрҙе индереү билет йәки идара итеү пакеты:

- I18NI000000047X (CARs, манифестар, SBOMs, дәлилдәр,
  Sigstore өйөмдәр, тапшырыу резюме).
- Ҡапҡа зонд + үҙ-үҙен сертификат сығыштары
  (I18NI000000048X,
  `artifacts/sorafs_gateway_self_cert/<stamp>/` X).
- DNS скелеты + баш ҡалыптары (I18NI000000050X,
  `portal.gateway.plan.json`, I18NI000000052X).
- Приборҙар таҡтаһы скриншоттар + иҫкәртмә таныуҙары.
- `status.md` яңыртыу һылтанма буйынса асыҡ һеңдерелгән һәм псевдоним бәйләү ваҡыты.

Был тикшерелгән исемлектән һуң DOCS-7 тапшыра: портал/I18NT00000000003X/SBOM файҙалы йөкләмәләр .
ҡапланған детерминистик, псевдонимдар менән ҡыҫтырылған, һаҡланған I18NI000000054X
башлыҡтары, һәм күҙәтеү осона тиклем аша ғәмәлдәге күҙәтеү стека.