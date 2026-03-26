---
id: portal-publish-plan
lang: ka
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

:::შენიშვნა კანონიკური წყარო
სარკეები `docs/source/sorafs/portal_publish_plan.md`. განაახლეთ ორივე ასლი, როდესაც სამუშაო პროცესი იცვლება.
:::

საგზაო რუქის პუნქტი DOCS-7 მოითხოვს ყველა დოკუმენტის არტეფაქტს (პორტალის აგება, OpenAPI სპეციფიკაცია,
SBOMs) მიედინება SoraFS მანიფესტის მილსადენში და ემსახურება `docs.sora`-ის მეშვეობით
`Sora-Proof` სათაურებით. ეს სია აერთიანებს არსებულ დამხმარეებს
ასე რომ, Docs/DevRel-ს, Storage-სა და Ops-ს შეუძლიათ გამოშვების გაშვება ნადირობის გარეშე
მრავალჯერადი წიგნები.

## 1. შექმენით და შეფუთეთ დატვირთვა

გაუშვით შეფუთვის დამხმარე (გამოტოვების ვარიანტები ხელმისაწვდომია მშრალი გაშვებისთვის):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` ხელახლა იყენებს `docs/portal/build`-ს, თუ CI უკვე გამოუშვა.
- დაამატეთ `--skip-sbom`, როდესაც `syft` მიუწვდომელია (მაგ., ჰაეროვანი რეპეტიცია).
- სკრიპტი აწარმოებს პორტალის ტესტებს, გამოსცემს CAR + manifest წყვილებს `portal`-ისთვის,
  `openapi`, `portal-sbom` და `openapi-sbom`, ამოწმებს თითოეულ მანქანას, როდესაც
  დაყენებულია `--proof` და ჩამოაგდებს Sigstore პაკეტებს, როდესაც დაყენებულია `--sign`.
- გამომავალი სტრუქტურა:

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

შეინახეთ მთელი საქაღალდე (ან სიმლინკი `artifacts/devportal/sorafs/latest`-ის საშუალებით) ასე
მმართველობის მიმომხილველებს შეუძლიათ დაადგინონ შენობის არტეფაქტები.

## 2. Pin Manifests + Aliases

გამოიყენეთ `sorafs_cli manifest submit`, რათა აიძულოთ მანიფესტები Torii-ში და დააკავშიროთ მეტსახელები.
დააყენეთ `${SUBMITTED_EPOCH}` უახლეს კონსენსუსის ეპოქაზე (დან
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` ან თქვენი დაფა).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<i105-account-id>"
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

- გაიმეორეთ `openapi.manifest.to`-სთვის და SBOM მანიფესტებისთვის (გამოტოვეთ მეტსახელის დროშები
  SBOM პაკეტები, თუ მმართველობა არ ანიჭებს სახელთა სივრცეს).
- ალტერნატივა: `iroha app sorafs pin register` მუშაობს წარდგენიდან დაიჯესტთან
  შეჯამება, თუ ორობითი უკვე დაინსტალირებულია.
- შეამოწმეთ რეესტრის მდგომარეობა
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- საყურებელი დაფები: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  მეტრიკა).

## 3. Gateway Headers & Proofs

შექმენით HTTP სათაურის ბლოკი + სავალდებულო მეტამონაცემები:

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

- შაბლონი მოიცავს `Sora-Name`, `Sora-CID`, `Sora-Proof` და
  `Sora-Proof-Status` სათაურები პლუს ნაგულისხმევი CSP/HSTS/Permissions-პოლიტიკა.
- გამოიყენეთ `--rollback-manifest-json` დაწყვილებული გადაბრუნების სათაურის ნაკრების გამოსატანად.

ტრაფიკის გამოვლენამდე, გაუშვით:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- გამოძიება ახორციელებს GAR ხელმოწერის სიახლეს, მეტსახელის პოლიტიკას და TLS სერთიფიკატს
  თითის ანაბეჭდები.
- თვითდამოწმების აღკაზმულობა ჩამოტვირთავს მანიფესტს `sorafs_fetch`-ით და ინახავს
  მანქანის გამეორების ჟურნალები; შეინახეთ შედეგები აუდიტის მტკიცებულებებისთვის.

## 4. DNS და ტელემეტრიის დაცვა

1. განაახლეთ DNS ჩონჩხი, რათა მმართველობამ დაამტკიცოს სავალდებულოობა:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. მონიტორი გაშვების დროს:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   დაფები: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` და პინის რეესტრის დაფა.

3. მოწევა გაფრთხილების წესები (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) და
   აღბეჭდეთ ჟურნალები/სკრინშოტები გამოშვების არქივისთვის.

## 5. მტკიცებულებათა ნაკრები

ჩართეთ შემდეგი გამოშვების ბილეთში ან მართვის პაკეტში:

- `artifacts/devportal/sorafs/<stamp>/` (მანქანები, მანიფესტები, SBOM, მტკიცებულებები,
  Sigstore პაკეტები, გაგზავნეთ რეზიუმეები).
- კარიბჭის ზონდი + თვითდამოწმების შედეგები
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS ჩონჩხი + სათაურის შაბლონები (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- დაფის ეკრანის ანაბეჭდები + გაფრთხილების დადასტურება.
- `status.md` განახლება, რომელიც მიუთითებს მანიფესტის შეჯამებასა და მეტსახელის სავალდებულო დროზე.

ამ საკონტროლო სიის შემდეგ აწვდის DOCS-7: პორტალი/OpenAPI/SBOM დატვირთვა არის
შეფუთული დეტერმინისტულად, მიმაგრებული მეტსახელებით, დაცული `Sora-Proof`-ით
სათაურები და დაკვირვება ბოლოდან ბოლომდე არსებული დაკვირვებადობის სტეკის მეშვეობით.