---
id: preview-host-exposure
lang: ka
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

DOCS‑SORA საგზაო რუკა მოითხოვს ყველა საჯარო გადახედვას იმავეზე ტარებისთვის
საკონტროლო ჯამით დამოწმებული ნაკრები, რომელსაც მიმომხილველები ახორციელებენ ადგილობრივად. გამოიყენეთ ეს სახელმძღვანელო
რეცენზენტის ჩასვლის შემდეგ (და მოწვევის დამტკიცების ბილეთი) დასრულებულია განთავსება
ბეტა წინასწარი გადახედვის მასპინძელი ონლაინ.

## წინაპირობები

- მიმომხილველის ჩასვლის ტალღა დაამტკიცა და შესულია გადახედვის ტრეკერში.
- უახლესი პორტალის აშენება წარმოდგენილია `docs/portal/build/`-ით და საკონტროლო ჯამით
  დამოწმებული (`build/checksums.sha256`).
- SoraFS წინასწარი გადახედვის სერთიფიკატები (Torii URL, ავტორიტეტი, პირადი გასაღები, გაგზავნილი
  ეპოქა) ინახება ან გარემოს ცვლადებში ან JSON კონფიგურაციაში, როგორიცაა
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- DNS შეცვლის ბილეთი გახსნილია სასურველი ჰოსტის სახელით (`docs-preview.sora.link`,
  `docs.iroha.tech` და ა.შ.) პლუს გამოძახების კონტაქტები.

## ნაბიჯი 1 – შექმენით და გადაამოწმეთ პაკეტი

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

გადამოწმების სკრიპტი უარს ამბობს გაგრძელებაზე, როდესაც არ არის საკონტროლო ჯამის მანიფესტი ან
შეცვლილია, ყოველი გადახედვის არტეფაქტის აუდიტის შენახვა.

## ნაბიჯი 2 – შეფუთეთ SoraFS არტეფაქტები

გადაიყვანეთ სტატიკური ადგილი დეტერმინისტულ CAR/მანიფესტის წყვილად. `ARTIFACT_DIR`
ნაგულისხმევად არის `docs/portal/artifacts/`.

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

მიამაგრეთ გენერირებული `portal.car`, `portal.manifest.*`, აღმწერი და გამშვები ჯამი
მანიფესტი გადახედვის ტალღის ბილეთზე.

## ნაბიჯი 3 – გამოაქვეყნეთ წინასწარი გადახედვის მეტსახელი

ხელახლა გაუშვით პინის დამხმარე ** `--skip-submit`-ის გარეშე, როგორც კი მზად იქნებით გამოსაქვეყნებლად
მასპინძელი. მიაწოდეთ JSON კონფიგურაცია ან აშკარა CLI დროშები:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

ბრძანება წერს `portal.pin.report.json`,
`portal.manifest.submit.summary.json` და `portal.submit.response.json`, რომელიც
უნდა გაიგზავნოს მოწვევის მტკიცებულებათა ნაკრები.

## ნაბიჯი 4 – შექმენით DNS ამოღების გეგმა

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

გაუზიარეთ მიღებული JSON Ops-ს, რათა DNS გადამრთველი ზუსტად მიუთითებდეს
მანიფესტი მონელება. ადრინდელი აღწერის ხელახლა გამოყენებისას, როგორც დაბრუნების წყაროს,
დაურთოს `--previous-dns-plan path/to/previous.json`.

## ნაბიჯი 5 – გამოიკვლიეთ განლაგებული ჰოსტი

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

გამოძიება ადასტურებს მოწოდებული გამოშვების ტეგს, CSP სათაურებს და ხელმოწერის მეტამონაცემებს.
გაიმეორეთ ბრძანება ორი რეგიონიდან (ან მიამაგრეთ გადახვევის გამომავალი), რათა აუდიტორებმა დაინახონ
რომ ზღვარზე ქეში თბილია.

## მტკიცებულებათა ნაკრები

ჩართეთ შემდეგი არტეფაქტები გადახედვის ტალღის ბილეთში და მიმართეთ მათში
მოწვევის ელფოსტა:

| არტეფაქტი | დანიშნულება |
|----------|---------|
| `build/checksums.sha256` | ადასტურებს, რომ პაკეტი ემთხვევა CI-ს. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Canonical SoraFS payload + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | აჩვენებს მანიფესტის წარდგენას + მეტსახელის შეკვრა წარმატებით დასრულდა. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS მეტამონაცემები (ბილეთი, ფანჯარა, კონტაქტები), მარშრუტის პოპულარიზაცია (`Sora-Route-Binding`) შეჯამება, `route_plan` მაჩვენებელი (გეგმის JSON + სათაურის შაბლონები), ქეშის გასუფთავების ინფორმაცია და დაბრუნების ინსტრუქციები Ops-ისთვის. |
| `artifacts/sorafs/preview-descriptor.json` | ხელმოწერილი აღმწერი, რომელიც აკავშირებს არქივს + საკონტროლო ჯამს. |
| `probe` გამომავალი | ადასტურებს, რომ ცოცხალი მასპინძელი აქვეყნებს მოსალოდნელ გამოშვების ტეგს. |

მას შემდეგ, რაც მასპინძელი ცოცხალია, მიჰყევით [მოწვევის წიგნს წინასწარ გადახედვა] (./public-preview-invite.md)
ბმულის გასავრცელებლად, მოწვევის შესვლა და ტელემეტრიის მონიტორინგი.