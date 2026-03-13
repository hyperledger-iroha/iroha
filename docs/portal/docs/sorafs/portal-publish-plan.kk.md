---
lang: kk
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

:::ескерту Канондық дереккөз
Айналар `docs/source/sorafs/portal_publish_plan.md`. Жұмыс процесі өзгерген кезде екі көшірмені де жаңартыңыз.
:::

Жол картасының DOCS-7 элементі әрбір құжат артефактісін қажет етеді (портал құрастыру, OpenAPI ерекшелігі,
SBOMs) SoraFS манифест құбыры арқылы ағып, `docs.sora` арқылы қызмет етеді.
`Sora-Proof` тақырыптарымен. Бұл бақылау тізімі бар көмекшілерді біріктіреді
сондықтан Docs/DevRel, Storage және Ops шығарылымды іздеусіз іске қоса алады
бірнеше runbook.

## 1. Пайдалы жүктемелерді құрастыру және пакеттеу

Буып-түю көмекшісін іске қосыңыз (өткізу опциялары құрғақ жұмыс үшін қолжетімді):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` `docs/portal/build` қайта пайдаланады, егер CI оны әлдеқашан шығарса.
- `syft` қолжетімді болмаған кезде `--skip-sbom` қосыңыз (мысалы, ауа саңылаулары бар репетиция).
- Сценарий портал сынақтарын іске қосады, `portal` үшін CAR + манифест жұптарын шығарады,
  `openapi`, `portal-sbom` және `openapi-sbom`, әрбір CAR тексергенде
  `--proof` орнатылды және `--sign` орнатылған кезде Sigstore бумаларын түсіреді.
- Шығару құрылымы:

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

Бүкіл қалтаны (немесе `artifacts/devportal/sorafs/latest` арқылы символдық сілтеме) солай сақтаңыз
басқаруды тексерушілер құрастыру артефактілерін бақылай алады.

## 2. Манифесттерді бекіту + бүркеншік аттар

Манифесттерді Torii ішіне жылжыту және бүркеншік аттарды байланыстыру үшін `sorafs_cli manifest submit` пайдаланыңыз.
`${SUBMITTED_EPOCH}` соңғы консенсус дәуіріне (
`curl -s "${TORII_URL}/v2/status" | jq '.sumeragi.epoch'` немесе сіздің бақылау тақтаңыз).

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

- `openapi.manifest.to` және SBOM манифесттері үшін қайталаңыз (үшін бүркеншік ат жалаушаларын өткізбеңіз)
  SBOM бумалары, егер басқару аттар кеңістігін тағайындамаса).
- Балама: `iroha app sorafs pin register` жіберілген дайджестпен жұмыс істейді
  екілік жүйе әлдеқашан орнатылған болса, қорытынды.
- арқылы тізілім күйін тексеріңіз
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Қарауға арналған бақылау тақталары: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  көрсеткіштер).

## 3. Шлюз тақырыптары және дәлелдер

HTTP тақырыбы блогын + байланыстыру метадеректерін жасаңыз:

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

- Үлгіде `Sora-Name`, `Sora-CID`, `Sora-Proof` және
  `Sora-Proof-Status` тақырыптары плюс әдепкі CSP/HSTS/Рұқсаттар-саясат.
- Жұптастырылған кері тақырып жинағын көрсету үшін `--rollback-manifest-json` пайдаланыңыз.

Трафикті ашпас бұрын келесі әрекеттерді орындаңыз:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Тексеру GAR қолтаңбасының жаңалығын, бүркеншік ат саясатын және TLS сертификатын қамтамасыз етеді
  саусақ іздері.
- Өзін-өзі растау құралы манифестті `sorafs_fetch` арқылы жүктеп алып, сақтайды.
  CAR қайта ойнату журналдары; аудиторлық дәлелдер үшін нәтижелерді сақтау.

## 4. DNS және телеметриялық қоршаулар

1. Басқару байланыстыруды дәлелдеу үшін DNS қаңқасын жаңартыңыз:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Шығарылым кезінде бақылау:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Бақылау тақталары: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` және PIN тіркеу тақтасы.

3. Ескерту ережелерін (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) және
   шығарылым мұрағаты үшін журналдарды/скриншоттарды түсіріңіз.

## 5. Дәлелдер жинағы

Шығару билетіне немесе басқару пакетіне мыналарды қосыңыз:

- `artifacts/devportal/sorafs/<stamp>/` (CAR, манифесттер, SBOM, дәлелдер,
  Sigstore жинақтары, қорытындыларды жіберіңіз).
- Шлюз зонды + өзін-өзі растау шығыстары
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS қаңқасы + тақырып үлгілері (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Бақылау тақтасының скриншоттары + ескертулерді растау.
- `status.md` манифест дайджестіне және бүркеншік атпен байланыстыру уақытына сілтеме жасайтын жаңарту.

Осы бақылау тізімінен кейін DOCS-7 жеткізіледі: портал/OpenAPI/SBOM пайдалы жүктемелері
анықтаушы түрде оралған, бүркеншік аттармен бекітілген, `Sora-Proof` арқылы қорғалған
тақырыптар және бұрыннан бар бақыланатын стек арқылы бақыланады.