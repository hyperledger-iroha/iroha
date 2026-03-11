---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ee0a1d26e0c9f3c1ab8908f7eb0dc73049d452e451aff9d2d892c19d733557e
source_last_modified: "2026-01-22T16:26:46.492940+00:00"
translation_last_reviewed: 2026-02-07
id: deploy-guide
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
---

## მიმოხილვა

ეს სათამაშო წიგნი გარდაქმნის საგზაო რუქის ერთეულებს **DOCS-7** (SoraFS გამომცემლობა) და **DOCS-8**
(CI/CD pin ავტომატიზაცია) დეველოპერის პორტალისთვის მოქმედ პროცედურაში.
იგი მოიცავს აწყობის/ჩამოყრის ფაზას, SoraFS შეფუთვას, Sigstore-ზე დამყარებულ მანიფესტს
ხელმოწერის, მეტსახელის პრომოუშენის, გადამოწმების და უკან დაბრუნების წვრთნები ასე რომ ყოველი გადახედვა და
გამოშვების არტეფაქტი არის რეპროდუცირებადი და აუდიტორული.

ნაკადი ვარაუდობს, რომ თქვენ გაქვთ `sorafs_cli` ორობითი (აშენებული
`--features cli`), წვდომა Torii საბოლოო წერტილზე პინი-რეესტრის ნებართვებით და
OIDC რწმუნებათა სიგელები Sigstore-ისთვის. შეინახეთ ხანგრძლივი საიდუმლოებები (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii ჟეტონები) თქვენს CI სარდაფში; ადგილობრივ გაშვებებს შეუძლიათ მათი წყარო
ჭურვის ექსპორტიდან.

## წინაპირობები

- კვანძი 18.18+ `npm` ან `pnpm`.
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli`-დან.
- Torii URL, რომელიც ასახავს `/v1/sorafs/*`-ს, პლუს ავტორიტეტული ანგარიშის/პირადი გასაღების
  რომელსაც შეუძლია მანიფესტებისა და მეტსახელების წარდგენა.
- OIDC გამომცემელი (GitHub Actions, GitLab, სამუშაო დატვირთვის იდენტიფიკაცია და ა.შ.)
  `SIGSTORE_ID_TOKEN`.
- სურვილისამებრ: `examples/sorafs_cli_quickstart.sh` მშრალი სირბილისთვის და
  `docs/source/sorafs_ci_templates.md` GitHub/GitLab სამუშაო ნაკადის ხარაჩოებისთვის.
- დააკონფიგურირეთ Try it OAuth ცვლადები (`DOCS_OAUTH_*`) და გაუშვით
  [უსაფრთხოების გამკვრივების საკონტროლო სია] (./security-hardening.md) კონსტრუქციის დაწინაურებამდე
  ლაბორატორიის გარეთ. პორტალის აწყობა ახლა ვერ ხერხდება, როდესაც ეს ცვლადები აკლია
  ან როდესაც TTL/საარჩევნო ღილაკები ცვივა იძულებითი ფანჯრების გარეთ; ექსპორტი
  `DOCS_OAUTH_ALLOW_INSECURE=1` მხოლოდ ერთჯერადი ადგილობრივი გადახედვისთვის. მიამაგრეთ
  კალამი ტესტის მტკიცებულება გათავისუფლების ბილეთზე.

## ნაბიჯი 0 - გადაიღეთ Try it proxy პაკეტი

სანამ Netlify-ზე ან კარიბჭეზე გადახედვის პოპულარიზაციას განახორციელებთ, დანიშნეთ ბეჭდით Try it proxy
წყაროები და ხელმოწერილი OpenAPI მანიფესტი დაიჯესტი დეტერმინისტულ პაკეტში:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` აკოპირებს პროქსი/ზონდი/დაბრუნების დამხმარეებს,
ამოწმებს OpenAPI ხელმოწერას და წერს `release.json` plus
`checksums.sha256`. მიამაგრეთ ეს პაკეტი Netlify/SoraFS კარიბჭის აქციაზე
ბილეთი, რათა მიმომხილველებმა შეძლონ გადაითამაშონ ზუსტი პროქსი წყაროები და Torii სამიზნე მინიშნებები
აღდგენის გარეშე. პაკეტში ასევე აღირიცხება იყო თუ არა კლიენტის მიერ მიწოდებული მატარებლები
ჩართულია (`allow_client_auth`), რათა შეინარჩუნოს გაშვების გეგმა და CSP წესები სინქრონიზებული.

## ნაბიჯი 1 - შექმენით და გააფართოვეთ პორტალი

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` ავტომატურად ახორციელებს `scripts/write-checksums.mjs`-ს, აწარმოებს:

- `build/checksums.sha256` — SHA256 მანიფესტი, რომელიც შესაფერისია `sha256sum -c`-ისთვის.
- `build/release.json` — მეტამონაცემები (`tag`, `generated_at`, `source`) ჩამაგრებული
  ყოველი მანქანა/მანიფესტი.

დაარქივეთ ორივე ფაილი CAR-ის შეჯამებასთან ერთად, რათა მიმომხილველებმა შეძლონ გადახედვის განსხვავება
არტეფაქტები აღდგენის გარეშე.

## ნაბიჯი 2 - შეფუთეთ სტატიკური აქტივები

გაუშვით CAR შეფუთვა Docusaurus გამომავალი დირექტორიაში. მაგალითი ქვემოთ
წერს ყველა არტეფაქტს `artifacts/devportal/` ქვეშ.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

JSON-ის შეჯამება აღწერს ნაწილთა რაოდენობას, დაიჯესტს და მტკიცებულების დაგეგმვის მინიშნებებს, რომ
`manifest build` და CI დაფები ხელახლა გამოიყენება მოგვიანებით.

## ნაბიჯი 2b - პაკეტი OpenAPI და SBOM კომპანიონები

DOCS-7 მოითხოვს პორტალის საიტის, OpenAPI სნეპშოტის და SBOM დატვირთვის გამოქვეყნებას
როგორც განსხვავებულად ვლინდება, ისე კარიბჭეებს შეუძლიათ დაამაგრონ `Sora-Proof`/`Sora-Content-CID`
სათაურები თითოეული არტეფაქტისთვის. გათავისუფლების დამხმარე
(`scripts/sorafs-pin-release.sh`) უკვე შეფუთულია OpenAPI დირექტორია
(`static/openapi/`) და SBOM-ები, რომლებიც გამოსხივებულია `syft`-ით ცალკე
`openapi.*`/`*-sbom.*` მანქანები და ჩაწერს მეტამონაცემებს
`artifacts/sorafs/portal.additional_assets.json`. ხელით ნაკადის გაშვებისას,
გაიმეორეთ ნაბიჯები 2–4 თითოეული დატვირთვისთვის საკუთარი პრეფიქსებითა და მეტამონაცემების ეტიკეტებით
(მაგალითად `--car-out "$OUT"/openapi.car` პლუს
`--metadata alias_label=docs.sora.link/openapi`). დაარეგისტრირეთ ყველა მანიფესტი/ალიასი
დაწყვილება Torii-ში (საიტი, OpenAPI, პორტალი SBOM, OpenAPI SBOM) DNS-ის გადართვამდე.
კარიბჭე შეიძლება ემსახურებოდეს ყველა გამოქვეყნებულ არტეფაქტს.

## ნაბიჯი 3 - შექმენით მანიფესტი

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

დაარეგულირეთ pin-policy flags თქვენი გამოშვების ფანჯარაში (მაგალითად, `--pin-storage-class
ცხელი` კანარებისთვის). JSON ვარიანტი არჩევითია, მაგრამ მოსახერხებელია კოდის განხილვისთვის.

## ნაბიჯი 4 — მოაწერეთ ხელი Sigstore-ით

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

ნაკრები იწერს მანიფესტის შეჯამებას, ნაწილაკების დაშლას და BLAKE3 ჰეშის
OIDC ჟეტონი JWT-ის შენარჩუნების გარეშე. შეინახეთ შეკვრაც და განცალკევებით
ხელმოწერა; წარმოების აქციებს შეუძლიათ იგივე არტეფაქტების ხელახლა გამოყენება თანამდებობიდან გადადგომის ნაცვლად.
ლოკალურ გაშვებებს შეუძლიათ შეცვალონ პროვაიდერის დროშები `--identity-token-env`-ით (ან დააყენოთ
`SIGSTORE_ID_TOKEN` გარემოში) როდესაც გარე OIDC დამხმარე გასცემს
ჟეტონი.

## ნაბიჯი 5 - გაგზავნეთ პინის რეესტრში

გაგზავნეთ ხელმოწერილი მანიფესტი (და ნაწილის გეგმა) Torii-ზე. ყოველთვის მოითხოვეთ რეზიუმე
ასე რომ, რეესტრის ჩანაწერის/ალიასის მტკიცებულება არის აუდიტი.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

გადახედვისას ან კანარის მეტსახელის (`docs-preview.sora`) გამოქვეყნებისას, გაიმეორეთ
წარდგენა უნიკალური მეტსახელით, რათა QA-მ შეძლოს კონტენტის შემოწმება წარმოებამდე
დაწინაურება.

Alias-ის დაკავშირება მოითხოვს სამ ველს: `--alias-namespace`, `--alias-name` და
`--alias-proof`. Governance აწარმოებს მტკიცებულების პაკეტს (base64 ან Norito ბაიტი)
როდესაც მეტსახელის მოთხოვნა დამტკიცდება; შეინახეთ იგი CI საიდუმლოებებში და მოათავსეთ როგორც ა
შეიყვანეთ `manifest submit`-ის გამოძახებამდე. დატოვეთ მეტსახელის დროშები დაუყენებელი, როდესაც თქვენ
განზრახული აქვს მხოლოდ მანიფესტის ჩამაგრება DNS-ის შეხების გარეშე.

## ნაბიჯი 5b - შექმენით მმართველობის წინადადება

ყველა მანიფესტმა უნდა იმოგზაუროს პარლამენტისთვის მზა წინადადებით ისე, რომ ნებისმიერი სორა
მოქალაქეს შეუძლია ცვლილება შეიტანოს პრივილეგირებული სერთიფიკატების სესხის გარეშე.
გაგზავნის/ხელმოწერის ნაბიჯების შემდეგ გაუშვით:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` იღებს კანონიკურ `RegisterPinManifest`-ს
ინსტრუქცია, ნაწილის დაიჯესტი, პოლიტიკა და მეტსახელის მინიშნება. მიამაგრეთ იგი მმართველობას
ბილეთი ან პარლამენტის პორტალი, ასე რომ დელეგატებს შეუძლიათ განასხვავონ დატვირთვა აღდგენის გარეშე
არტეფაქტები. რადგან ბრძანება არასოდეს ეხება Torii ავტორიტეტის გასაღებს, ნებისმიერს
მოქალაქეს შეუძლია წინადადების შედგენა ადგილობრივად.

## ნაბიჯი 6 - გადაამოწმეთ მტკიცებულებები და ტელემეტრია

ჩამაგრების შემდეგ, გაიარეთ გადამოწმების განმსაზღვრელი ნაბიჯები:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- შეამოწმეთ `torii_sorafs_gateway_refusals_total` და
  `torii_sorafs_replication_sla_total{outcome="missed"}` ანომალიებისთვის.
- გაუშვით `npm run probe:portal` Try-It პროქსისა და ჩაწერილი ბმულების გამოსაყენებლად
  ახლად ჩამაგრებული კონტენტის წინააღმდეგ.
- აიღეთ მონიტორინგის მტკიცებულებები, რომლებიც აღწერილია
  [გამოქვეყნება და მონიტორინგი] (./publishing-monitoring.md) ასე რომ DOCS-3c's
  დაკვირვებადობის კარიბჭე დაკმაყოფილებულია გამოქვეყნების საფეხურებთან ერთად. დამხმარე
  ახლა იღებს მრავალ `bindings` ჩანაწერს (საიტი, OpenAPI, პორტალი SBOM, OpenAPI
  SBOM) და ახორციელებს `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` სამიზნეზე
  მასპინძელი არჩევითი `hostname` მცველის მეშვეობით. ქვემოთ მოწოდება წერს ორივე ა
  ერთი JSON შეჯამება და მტკიცებულებების ნაკრები (`portal.json`, `tryit.json`,
  `binding.json` და `checksums.sha256`) გამოშვების დირექტორიაში:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## ნაბიჯი 6a - დაგეგმეთ კარიბჭის სერთიფიკატები

შექმენით TLS SAN/გამოწვევის გეგმა GAR პაკეტების შექმნამდე
გუნდი და DNS-ის დამმტკიცებლები განიხილავენ იგივე მტკიცებულებებს. ახალი დამხმარე სარკეა
DG-3 ავტომატიზაციის შეყვანები კანონიკური ბუნების მასპინძლების ჩამოთვლით,
ლამაზი მასპინძელი SAN-ები, DNS-01 ლეიბლები და რეკომენდებული ACME გამოწვევები:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

ჩააბარეთ JSON გამოშვების პაკეტთან ერთად (ან ატვირთეთ იგი ცვლილებასთან ერთად
ბილეთი), რათა ოპერატორებმა შეძლონ SAN მნიშვნელობების ჩასმა Torii-ში
`torii.sorafs_gateway.acme` კონფიგურაცია და GAR მიმომხილველებს შეუძლიათ დაადასტურონ
კანონიკური/საკმაოდ შესრულებული რუქები მასპინძელი წარმოებულების ხელახლა გაშვების გარეშე. დაამატეთ დამატებითი
`--name` არგუმენტები თითოეული სუფიქსისთვის, რომელიც დაწინაურებულია იმავე გამოცემაში.

## ნაბიჯი 6b - გამოიღეთ მასპინძლის კანონიკური რუკები

GAR დატვირთვის შაბლონის შექმნამდე, ჩაწერეთ ჰოსტის განმსაზღვრელი რუკა თითოეულისთვის
მეტსახელი. `cargo xtask soradns-hosts` ჰეშირებს თითოეულ `--name`-ს თავის კანონიკურში
ლეიბლი (`<base32>.gw.sora.id`), ასხივებს საჭირო ბუნებრივ ბარათს
(`*.gw.sora.id`) და იღებს ლამაზ ჰოსტს (`<alias>.gw.sora.name`). დაჟინებით
გამოსავალი გამოშვების არტეფაქტებში, რათა DG-3-ის მიმომხილველებმა შეძლონ განასხვავონ რუქები
GAR წარდგენის პარალელურად:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

გამოიყენეთ `--verify-host-patterns <file>` სწრაფად წარუმატებლობისთვის GAR ან კარიბჭის დროს
binding JSON გამოტოვებს ერთ-ერთ საჭირო ჰოსტს. დამხმარე იღებს მრავალჯერადს
ვერიფიკაციის ფაილები, რაც გაადვილებს როგორც GAR შაბლონს, ასევე
დამაგრებული `portal.gateway.binding.json` იმავე გამოძახებაში:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

მიამაგრეთ შემაჯამებელი JSON და ვერიფიკაციის ჟურნალი DNS/gateway-ის შეცვლის ბილეთს
აუდიტორებს შეუძლიათ დაადასტურონ კანონიკური, wildcard და ლამაზი ჰოსტები ხელახლა გაშვების გარეშე
წარმოებული სკრიპტები. ხელახლა გაუშვით ბრძანება, როდესაც მას ახალი მეტსახელები დაემატება
შეფუთეთ, ამიტომ GAR-ის შემდგომი განახლებები მემკვიდრეობით მიიღებს იგივე მტკიცებულების კვალს.

## ნაბიჯი 7 — შექმენით DNS cutover descriptor

წარმოების წყვეტები საჭიროებს აუდიტორულ ცვლილებების პაკეტს. წარმატებულის შემდეგ
წარდგენა (ასევე სავალდებულო), დამხმარე გამოსცემს
`artifacts/sorafs/portal.dns-cutover.json`, გადაღება:- მეტამონაცემების მეტამონაცემების მეტამონაცემების (სახელების სივრცე/სახელი/მტკიცებულება, მანიფესტის შეჯამება, Torii URL,
  წარდგენილი ეპოქა, ავტორიტეტი);
- გამოშვების კონტექსტი (თეგი, მეტსახელის ლეიბლი, მანიფესტის/CAR ბილიკები, ნაწილის გეგმა, Sigstore
  შეკვრა);
- გადამოწმების მაჩვენებლები (ზონდის ბრძანება, მეტსახელი + Torii საბოლოო წერტილი); და
- სურვილისამებრ ცვლილების კონტროლის ველები (ბილეთის ID, ამოღების ფანჯარა, ოპერაციული კონტაქტი,
  წარმოების ჰოსტის სახელი/ზონა);
- მარშრუტის სარეკლამო მეტამონაცემები, რომლებიც მიღებულია დამაგრებული `Sora-Route-Binding`-დან
  სათაური (კანონიკური ჰოსტი/CID, სათაური + სავალდებულო ბილიკები, გადამოწმების ბრძანებები),
  გარანტია, რომ GAR-ის დაწინაურება და სარეზერვო წვრთნები ეხება იმავე მტკიცებულებებს;
- გენერირებული მარშრუტის გეგმის არტეფაქტები (`gateway.route_plan.json`,
  სათაურის შაბლონები და სურვილისამებრ დაბრუნების სათაურები) ამიტომ შეცვალეთ ბილეთები და CI
  lint hook-ებს შეუძლიათ დაადასტურონ, რომ ყველა DG-3 პაკეტი მიუთითებს კანონიკურზე
  ხელშეწყობა/დაბრუნების გეგმები დამტკიცებამდე;
- სურვილისამებრ ქეშის გაუქმების მეტამონაცემები (გასუფთავების საბოლოო წერტილი, auth ცვლადი, JSON
  payload და მაგალითად `curl` ბრძანება); და
- უკან დაბრუნების მინიშნებები, რომლებიც მიუთითებს წინა აღწერზე (გაათავისუფლეთ ტეგი და მანიფესტი
  დაიჯესტი) ასე რომ, შეცვალეთ ბილეთები და მიიღეთ დეტერმინისტული სარეზერვო გზა.

როდესაც გამოშვება მოითხოვს ქეშის გაწმენდას, შექმენით კანონიკური გეგმა გვერდით
ჭრის აღმწერი:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

მიამაგრეთ მიღებული `portal.cache_plan.json` DG-3 პაკეტზე, რათა ოპერატორები
ჰქონდეთ დეტერმინისტული ჰოსტები/ბილიკები (და შესაბამისი auth მინიშნებები) გაცემისას
`PURGE` ითხოვს. აღწერის არჩევითი ქეშის მეტამონაცემების განყოფილებას შეუძლია მიმართოს
ეს ფაილი პირდაპირ, ზუსტად რომელზეა გათვლილი ცვლილებების კონტროლის მიმომხილველები
ბოლო წერტილები ჩამოირეცხება ამოჭრის დროს.

ყველა DG-3 პაკეტს ასევე სჭირდება ხელშეწყობა + დაბრუნების ჩამონათვალი. გენერირება მეშვეობით
`cargo xtask soradns-route-plan`, რათა ცვლილებების კონტროლის მიმომხილველებმა შეძლონ ზუსტი მიკვლევა
ფრენის წინ, გათიშვისა და უკან დაბრუნების ნაბიჯები მეტსახელის მიხედვით:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

ემიტირებული `gateway.route_plan.json` იჭერს კანონიკურ/ლამაზ მასპინძლებს, დადგმული
ჯანმრთელობის შემოწმების შეხსენებები, GAR სავალდებულო განახლებები, ქეშის გასუფთავება და უკან დაბრუნება.
შეაერთეთ იგი GAR/შემკვრელი/დაჭრილი არტეფაქტებით ცვლილების გაგზავნამდე
ბილეთი, რათა Ops-მა შეძლოს რეპეტიცია და ხელი მოაწეროს იმავე სკრიპტულ ნაბიჯებს.

`scripts/generate-dns-cutover-plan.mjs` ამარაგებს ამ დესკრიპტორს და მუშაობს
ავტომატურად `sorafs-pin-release.sh`-დან. მისი რეგენერაცია ან მორგება
ხელით:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

შეავსეთ არასავალდებულო მეტამონაცემები გარემოს ცვლადების მეშვეობით პინის გაშვებამდე
დამხმარე:

| ცვლადი | დანიშნულება |
|----------|---------|
| `DNS_CHANGE_TICKET` | ბილეთის ID ინახება აღწერში. |
| `DNS_CUTOVER_WINDOW` | ISO8601 ამოჭრილი ფანჯარა (მაგ., `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | წარმოების ჰოსტის სახელი + ავტორიტეტული ზონა. |
| `DNS_OPS_CONTACT` | გამოძახების მეტსახელი ან ესკალაციის კონტაქტი. |
| `DNS_CACHE_PURGE_ENDPOINT` | ქეშის გასუფთავების საბოლოო წერტილი ჩაწერილია აღმწერში. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var შეიცავს გაწმენდის ჟეტონს (ნაგულისხმევი `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | გადაბრუნების მეტამონაცემების წინა ამოღების აღწერის გზა. |

მიამაგრეთ JSON DNS-ის ცვლილების მიმოხილვას, რათა დამმტკიცებლებმა შეძლონ მანიფესტის დადასტურება
digests, alias bindings და probe ბრძანებები CI ჟურნალების გაფუჭების გარეშე.
CLI დროშები `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` და `--previous-dns-plan` უზრუნველყოფს იგივე გადაფარვას
დამხმარე CI-ს გარეთ გაშვებისას.

## ნაბიჯი 8 - გადამწყვეტი ზონის ფაილის ჩონჩხის გამოშვება (სურვილისამებრ)

როდესაც წარმოების ამოღების ფანჯარა ცნობილია, გამოშვების სკრიპტს შეუძლია გამოუშვას ის
SNS ზონის ფაილის ჩონჩხი და გადამწყვეტი ფრაგმენტი ავტომატურად. გაიარეთ სასურველი DNS
ჩანაწერები და მეტამონაცემები გარემოს ცვლადების ან CLI პარამეტრების მეშვეობით; დამხმარე
დარეკვისთანავე დაურეკავს `scripts/sns_zonefile_skeleton.py`-ს
წარმოიქმნება აღმწერი. მიუთითეთ მინიმუმ ერთი A/AAAA/CNAME მნიშვნელობა და GAR
დაიჯესტი (BLAKE3-256 ხელმოწერილი GAR ტვირთის). თუ ზონა/მასპინძლის სახელი ცნობილია
და `--dns-zonefile-out` გამოტოვებულია, წერს დამხმარე
`artifacts/sns/zonefiles/<zone>/<hostname>.json` და ავსებს
`ops/soradns/static_zones.<hostname>.json` როგორც გადამწყვეტი ფრაგმენტი.

| ცვლადი / დროშა | დანიშნულება |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | გენერირებული ზონის ფაილის ჩონჩხის გზა. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Resolver snippet-ის ბილიკი (ნაგულისხმევად `ops/soradns/static_zones.<hostname>.json`, როდესაც გამოტოვებულია). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL გამოიყენება გენერირებულ ჩანაწერებზე (ნაგულისხმევი: 600 წამი). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 მისამართები (მძიმით გამოყოფილი env ან განმეორებადი CLI დროშა). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 მისამართები. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | სურვილისამებრ CNAME სამიზნე. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI ქინძისთავები (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | დამატებითი TXT ჩანაწერები (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | გამოთვალეთ zonefile ვერსიის ლეიბლი. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | იძულებითი `effective_at` დროის შტამპი (RFC3339) გადაწყვეტის ფანჯრის დაწყების ნაცვლად. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | გადააჭარბეთ მეტამონაცემებში ჩაწერილი მტკიცებულების სიტყვასიტყვით. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | გადააჭარბეთ მეტამონაცემებში ჩაწერილი CID. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | მეურვის გაყინვის მდგომარეობა (რბილი, მყარი, დათბობა, მონიტორინგი, საგანგებო). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | მეურვის/საკრებულოს ბილეთის მითითება გაყინვისთვის. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 დათბობის დროის შტამპი. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | დამატებითი გაყინვის შენიშვნები (მძიმით გამოყოფილი env ან განმეორებადი დროშა). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 დაიჯესტი (ექვსკუთხა) ხელმოწერილი GAR ტვირთის. საჭიროა კარიბჭის შესაკრავების არსებობისას. |

GitHub Actions სამუშაო ნაკადი კითხულობს ამ მნიშვნელობებს საცავის საიდუმლოებიდან, ასე რომ, წარმოების ყველა პინი ავტომატურად გამოსცემს zonefile არტეფაქტებს. დააკონფიგურირეთ შემდეგი საიდუმლოებები (სტრიქონები შეიძლება შეიცავდეს მძიმით გამოყოფილ სიებს მრავალმნიშვნელოვანი ველებისთვის):

| საიდუმლო | დანიშნულება |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | წარმოების ჰოსტის სახელი/ზონა გადაეცა დამხმარეს. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | გამოძახების მეტსახელი ინახება აღწერში. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 ჩანაწერები გამოსაქვეყნებლად. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | სურვილისამებრ CNAME სამიზნე. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI ქინძისთავები. |
| `DOCS_SORAFS_ZONEFILE_TXT` | დამატებითი TXT ჩანაწერები. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | ჩონჩხში ჩაწერილი მეტამონაცემების გაყინვა. |
| `DOCS_SORAFS_GAR_DIGEST` | თექვსმეტობით კოდირებული BLAKE3 დაიჯესტი ხელმოწერილი GAR ტვირთის. |

`.github/workflows/docs-portal-sorafs-pin.yml` ჩართვისას მიაწოდეთ `dns_change_ticket` და `dns_cutover_window` შეყვანები, რათა აღწერილმა/ზონის ფაილმა დაიმკვიდროს სწორი ცვლილების ფანჯრის მეტამონაცემები. დატოვე ისინი ცარიელი მხოლოდ მშრალი გაშვებისას.

ტიპიური მოწოდება (შეესაბამება SN-7 მფლობელის წიგნს):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

დამხმარე ავტომატურად ატარებს ცვლილების ბილეთს, როგორც TXT ჩანაწერს და
მემკვიდრეობით იღებს ამოღების ფანჯრის დაწყებას, როგორც `effective_at` დროის ნიშანს, თუ
გადააჭარბა. სრული ოპერაციული სამუშაო პროცესისთვის იხ
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### საჯარო DNS დელეგაციის შენიშვნა

zonefile ჩონჩხი განსაზღვრავს მხოლოდ ავტორიტეტულ ჩანაწერებს ზონისთვის. შენ
ჯერ კიდევ გჭირდებათ მშობლის ზონის NS/DS დელეგაციის კონფიგურაცია თქვენს რეგისტრატორში ან DNS-ში
პროვაიდერი, რათა რეგულარულ ინტერნეტს შეეძლოს სახელების სერვერების აღმოჩენა.

- apex/TLD ამონაჭრებისთვის გამოიყენეთ ALIAS/ANAME (პროვაიდერის სპეციფიკური) ან გამოაქვეყნეთ A/AAAA
  ჩანაწერები, რომლებიც მიუთითებს კარიბჭეზე anycast IP-ებზე.
- ქვედომენებისთვის, გამოაქვეყნეთ CNAME მიღებული ლამაზი ჰოსტისთვის
  (`<fqdn>.gw.sora.name`).
- კანონიკური ჰოსტი (`<hash>.gw.sora.id`) რჩება კარიბჭის დომენის ქვეშ და
  არ არის გამოქვეყნებული თქვენს საჯარო ზონაში.

### კარიბჭის სათაურის შაბლონი

განლაგების დამხმარე ასევე ასხივებს `portal.gateway.headers.txt` და
`portal.gateway.binding.json`, ორი არტეფაქტი, რომელიც აკმაყოფილებს DG-3-ს
კარიბჭე-შიგთავსის სავალდებულო მოთხოვნა:

- `portal.gateway.headers.txt` შეიცავს სრულ HTTP სათაურის ბლოკს (მათ შორის
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS და
  `Sora-Route-Binding` აღმწერი), რომელიც კიდეების კარიბჭეები უნდა დამაგრდეს ყველა
  პასუხი.
- `portal.gateway.binding.json` იწერს იგივე ინფორმაციას მანქანით წასაკითხად
  ფორმა ისე, რომ შეცვალეთ ბილეთები და ავტომატიზაციამ შეიძლება განასხვავოს მასპინძელი/ციდი კავშირის გარეშე
  scraping shell გამომავალი.

ისინი იქმნება ავტომატურად მეშვეობით
`cargo xtask soradns-binding-template`
და აიღეთ მეტსახელი, მანიფესტის დაიჯესტი და კარიბჭის ჰოსტის სახელი, რომელიც იყო მოწოდებული
`sorafs-pin-release.sh`-მდე. სათაურის ბლოკის რეგენერაციის ან მორგებისთვის, გაუშვით:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

გაიარეთ `--csp-template`, `--permissions-template`, ან `--hsts-template` გადასაფარებლად
ნაგულისხმევი სათაურის შაბლონები, როდესაც კონკრეტულ განლაგებას სჭირდება დამატებითი
დირექტივები; დააკავშირეთ ისინი არსებულ `--no-*` კონცენტრატორებთან სათაურის ჩამოსაშლელად
მთლიანად.

მიამაგრეთ ჰედერის ფრაგმენტი CDN ცვლილების მოთხოვნას და მიამაგრეთ JSON დოკუმენტი
კარიბჭის ავტომატიზაციის მილსადენში, ასე რომ, მასპინძლის რეალური ხელშეწყობა ემთხვევა
გაათავისუფლეს მტკიცებულებები.

გამოშვების სკრიპტი ავტომატურად აწარმოებს გადამოწმების დამხმარეს, ასე რომ DG-3 ბილეთებს
ყოველთვის შეიცავს უახლეს მტკიცებულებებს. ხელახლა გაუშვით ხელით, როცა შეცვლით
JSON-ის ხელით შეკვრა:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

ბრძანება ამოწმებს `Sora-Proof` დატვირთვას, რომელიც დაფიქსირებულია სავალდებულო პაკეტში,
უზრუნველყოფს `Sora-Route-Binding` მეტამონაცემების მანიფესტის CID + ჰოსტის სახელს,
და სწრაფად იშლება, თუ რომელიმე სათაური გადაინაცვლებს. დაარქივეთ კონსოლის გამომავალი გვერდით
განლაგების სხვა არტეფაქტები, როდესაც თქვენ აწარმოებთ ბრძანებას CI-ს გარეთ DG-3
რეცენზენტებს აქვთ მტკიცებულება, რომ სავალდებულო იყო დამოწმებული გადაწყვეტამდე.> **DNS აღწერის ინტეგრაცია:** `portal.dns-cutover.json` ახლა ჩაშენებულია
> `gateway_binding` განყოფილება, რომელიც მიუთითებს ამ არტეფაქტებზე (ბილიკები, შინაარსის CID,
> მტკიცებულების სტატუსი და პირდაპირი სათაურის შაბლონი) **და** `route_plan` სტროფი
> მითითება `gateway.route_plan.json` პლუს მთავარ + დაბრუნების სათაური
> შაბლონები. ჩართეთ ეს ბლოკები DG-3-ის ყველა ცვლილების ბილეთში, რათა მიმომხილველებმა შეძლონ
> განასხვავეთ ზუსტი `Sora-Name/Sora-Proof/CSP` მნიშვნელობები და დაადასტურეთ, რომ მარშრუტი
> დაწინაურების/დაბრუნების გეგმები ემთხვევა მტკიცებულებების პაკეტს კონსტრუქციის გახსნის გარეშე
> არქივი.

## ნაბიჯი 9 - გაუშვით საგამომცემლო მონიტორები

საგზაო რუკის ამოცანა **DOCS-3c** მოითხოვს უწყვეტ მტკიცებულებას, რომ პორტალი, სცადეთ
პროქსი, და კარიბჭის საკინძები რჩება ჯანმრთელი გამოშვების შემდეგ. გაუშვით კონსოლიდირებული
დააკვირდით დაუყოვნებლივ 7–8 ნაბიჯების შემდეგ და შეაერთეთ იგი თქვენს დაგეგმილ ზონდებში:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` ატვირთავს კონფიგურაციის ფაილს (იხ
  `docs/portal/docs/devportal/publishing-monitoring.md` სქემისთვის) და
  ახორციელებს სამ შემოწმებას: პორტალის ბილიკების გამოკვლევები + CSP/ნებართვები-პოლიტიკის ვალიდაცია,
  სცადეთ პროქსი ზონდები (სურვილისამებრ შეეხეთ მის `/metrics` საბოლოო წერტილს) და
  კარიბჭის სავალდებულო დამადასტურებელი (`cargo xtask soradns-verify-binding`), რომელიც ამოწმებს
  აღბეჭდილი სავალდებულო პაკეტი მოსალოდნელი მეტსახელის, მასპინძლის, მტკიცებულების სტატუსის წინააღმდეგ,
  და მანიფესტი JSON.
- ბრძანება გამოდის ნულის გარეშე, როდესაც რომელიმე ზონდი ვერ ხერხდება, ამიტომ CI, cron სამუშაოები ან
  runbook ოპერატორებს შეუძლიათ შეაჩერონ გამოშვება მეტსახელების პოპულარიზაციამდე.
- `--json-out`-ის გავლისას იწერება ერთი შემაჯამებელი JSON დატვირთვა თითო სამიზნეზე
  სტატუსი; `--evidence-dir` ასხივებს `summary.json`, `portal.json`, `tryit.json`,
  `binding.json` და `checksums.sha256`, რათა მმართველობის მიმომხილველებმა განასხვავონ
  შედეგები მონიტორების ხელახლა გაშვების გარეშე. დაარქივეთ ეს დირექტორია ქვეშ
  `artifacts/sorafs/<tag>/monitoring/` Sigstore პაკეტთან და DNS-თან ერთად
  ამოჭრის აღმწერი.
- ჩართეთ მონიტორის გამომავალი, Grafana ექსპორტი (`dashboards/grafana/docs_portal.json`),
  და Alertmanager საბურღი ID გამოშვების ბილეთში, რათა DOCS-3c SLO იყოს
  აუდიტი მოგვიანებით. გამომცემლობის მონიტორის სათამაშო წიგნი ცხოვრობს
  `docs/portal/docs/devportal/publishing-monitoring.md`.

პორტალის ზონდებს სჭირდება HTTPS და უარყოფს `http://` საბაზისო URL-ებს, თუ
`allowInsecureHttp` დაყენებულია მონიტორის კონფიგურაციაში; წარმოება/დადგმის შენარჩუნება
მიზნად ისახავს TLS-ს და ჩართეთ მხოლოდ ადგილობრივი გადახედვის უგულებელყოფა.

მონიტორის ავტომატიზაცია `npm run monitor:publishing`-ის საშუალებით Buildkite/cron-ში ერთხელ
პორტალი პირდაპირ ეთერშია. იგივე ბრძანება, რომელიც მითითებულია წარმოების URL-ებზე, აწვდის მიმდინარეობას
ჯანმრთელობის შემოწმებები, რომლებსაც SRE/Docs ეყრდნობა გამოშვებებს შორის.

## ავტომატიზაცია `sorafs-pin-release.sh`-ით

`docs/portal/scripts/sorafs-pin-release.sh` აერთიანებს ნაბიჯებს 2–6. ეს:

1. დაარქივებს `build/` დეტერმინისტულ ტარბოლში,
2. მუშაობს `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   და `proof verify`,
3. სურვილისამებრ ახორციელებს `manifest submit` (მათ შორის მეტსახელის სავალდებულოა), როდესაც Torii
   რწმუნებათა სიგელები არსებობს და
4. წერს `artifacts/sorafs/portal.pin.report.json`, სურვილისამებრ
  `portal.pin.proposal.json`, DNS cutover descriptor (გაგზავნის შემდეგ),
  და კარიბჭის შესაკრავის ნაკრები (`portal.gateway.binding.json` პლუს
  ტექსტის სათაურის ბლოკი) ასე რომ მმართველობის, ქსელის და ოპერაციების გუნდებს შეუძლიათ განასხვავონ ისინი
  მტკიცებულებათა ნაკრები CI ჟურნალების გახეხვის გარეშე.

დააყენეთ `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` და (სურვილისამებრ)
`PIN_ALIAS_PROOF_PATH` სკრიპტის გამოძახებამდე. გამოიყენეთ `--skip-submit` გასაშრობად
ეშვება; ქვემოთ აღწერილი GitHub სამუშაო ნაკადი ცვლის ამას `perform_submit`-ის მეშვეობით
შეყვანა.

## ნაბიჯი 8 — გამოაქვეყნეთ OpenAPI სპეციფიკაციები და SBOM პაკეტები

DOCS-7 საჭიროებს პორტალის აგებას, OpenAPI სპეციფიკაციას და SBOM არტეფაქტებს მოგზაურობისთვის
იგივე დეტერმინისტული მილსადენით. არსებული დამხმარეები მოიცავს სამივეს:

1. ** რეგენერაცია და ხელი მოაწერეთ სპეციფიკაციას. **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   გადაიტანეთ გამოშვების ლეიბლი `--version=<label>`-ის საშუალებით, როდესაც გსურთ შეინარჩუნოთ
   ისტორიული სურათი (მაგალითად, `2025-q3`). დამხმარე წერს კადრს
   `static/openapi/versions/<label>/torii.json`-მდე, ასახავს მას
   `versions/current` და ჩაწერს მეტამონაცემებს (SHA-256, მანიფესტის სტატუსი და
   განახლებული დროის შტამპი) `static/openapi/versions.json`-ში. დეველოპერის პორტალი
   კითხულობს ამ ინდექსს, რათა Swagger/RapiDoc პანელებმა წარმოადგინონ ვერსიის ამომრჩევი
   და აჩვენეთ ასოცირებული დაიჯესტის/ხელმოწერის ინფორმაცია ხაზში. გამოტოვება
   `--version` ინარჩუნებს წინა გამოშვების ლეიბლებს ხელუხლებლად და მხოლოდ განაახლებს
   `current` + `latest` მაჩვენებლები.

   მანიფესტი იჭერს SHA-256/BLAKE3-ის მონელებას, რათა კარიბჭე შეძლოს დამაგრება
   `Sora-Proof` სათაურები `/reference/torii-swagger`-ისთვის.

2. **Emit CycloneDX SBOMs.** გამოშვების მილსადენი უკვე მოელის syft-ზე დაფუძნებულს
   SBOM-ები `docs/source/sorafs_release_pipeline_plan.md`-ზე. შეინახეთ გამომავალი
   შენობის არტეფაქტების გვერდით:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** ჩაალაგეთ თითოეული ტვირთი მანქანაში.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   მიჰყევით იგივე `manifest build` / `manifest sign` ნაბიჯებს, როგორც მთავარი საიტი,
   თითო აქტივზე მეტსახელების დარეგულირება (მაგალითად, `docs-openapi.sora` სპეციფიკაციისთვის და
   `docs-sbom.sora` ხელმოწერილი SBOM პაკეტისთვის). განსხვავებული მეტსახელების შენარჩუნება
   ინახავს SoraDNS მტკიცებულებებს, GAR-ებს და უკან დაბრუნების ბილეთებს ზუსტი დატვირთვის ფარგლებში.

4. **გააგზავნეთ და დააკავშირეთ.** ხელახლა გამოიყენეთ არსებული ავტორიტეტი + Sigstore პაკეტი, მაგრამ
   ჩაწერეთ მეტსახელი tuple გამოშვების სიაში, რათა აუდიტორებმა შეძლონ თვალყური ადევნონ რომელი
   სორას სახელების რუქები, რომლებზეც მანიფესტი დაიჯესტება.

სპეციფიკაციის/SBOM-ის არქივირება პორტალის აწყობასთან ერთად უზრუნველყოფს ყველა
გამოშვების ბილეთი შეიცავს სრულ არტეფაქტის კომპლექტს შეფუთვის ხელახლა გაშვების გარეშე.

### ავტომატიზაციის დამხმარე (CI/პაკეტის სკრიპტი)

`./ci/package_docs_portal_sorafs.sh` აკოდირებს ნაბიჯებს 1–8, ასე რომ, საგზაო რუქის ერთეული
**DOCS‑7** შეიძლება განხორციელდეს ერთი ბრძანებით. დამხმარე:

- აწარმოებს პორტალის საჭირო მომზადებას (`npm ci`, OpenAPI/norito სინქრონიზაცია, ვიჯეტის ტესტები);
- ასხივებს პორტალს, OpenAPI და SBOM CARs + manifest წყვილებს `sorafs_cli`-ის მეშვეობით;
- სურვილისამებრ მუშაობს `sorafs_cli proof verify` (`--proof`) და Sigstore ხელმოწერა
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- ჩამოაგდებს ყველა არტეფაქტს `artifacts/devportal/sorafs/<timestamp>/`-ის ქვეშ და
  წერს `package_summary.json`, რათა CI/release tooling-მა შეძლოს პაკეტის გადაყლაპვა; და
- განაახლებს `artifacts/devportal/sorafs/latest`, რათა მიუთითოს ბოლო გაშვებაზე.

მაგალითი (სრული მილსადენი Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

დროშები, რომლებიც უნდა იცოდეთ:

- `--out <dir>` – არტეფაქტის ფესვის გადაფარვა (ნაგულისხმევი ინახავს დროის შტამპის საქაღალდეებს).
- `--skip-build` - ხელახლა გამოიყენე არსებული `docs/portal/build` (მოხერხებულია, როცა CI ვერ
  რეკონსტრუქცია ოფლაინ სარკეების გამო).
- `--skip-sync-openapi` – გამოტოვეთ `npm run sync-openapi` როდესაც `cargo xtask openapi`
  ვერ აღწევს crates.io.
- `--skip-sbom` - მოერიდეთ `syft`-ს დარეკვას, როდესაც ორობითი არ არის დაინსტალირებული (
  სკრიპტი ბეჭდავს გაფრთხილებას).
- `--proof` – გაუშვით `sorafs_cli proof verify` თითოეული CAR/მანიფესტის წყვილისთვის. მრავალ-
  ფაილების დატვირთვა კვლავ საჭიროებს chunk-plan მხარდაჭერას CLI-ში, ამიტომ დატოვეთ ეს დროშა
  გააუქმეთ დაყენება, თუ დააწექით `plan chunk count` შეცდომებს და გადაამოწმეთ ხელით ერთხელ
  ზემოთ კარიბჭის მიწები.
- `--sign` – გამოძახება `sorafs_cli manifest sign`. მიაწოდეთ ნიშანი
  `SIGSTORE_ID_TOKEN` (ან `--sigstore-token-env`) ან ნება მიეცით CLI-ს მიიღოს ის გამოყენებით
  `--sigstore-provider/--sigstore-audience`.

წარმოების არტეფაქტების გადაზიდვისას გამოიყენეთ `docs/portal/scripts/sorafs-pin-release.sh`.
ახლა ის ავსებს პორტალს, OpenAPI და SBOM დატვირთვას, ხელს აწერს თითოეულ მანიფესტს და
აღრიცხავს დამატებით აქტივების მეტამონაცემებს `portal.additional_assets.json`-ში. დამხმარე
ესმის იგივე არჩევითი სახელურები, რომლებიც გამოიყენება CI პაკეტერის მიერ, პლუს ახალი
`--openapi-*`, `--portal-sbom-*` და `--openapi-sbom-*` გადამრთველები ასე რომ თქვენ შეგიძლიათ
მივანიჭოთ მეტსახელის ტოპები თითო არტეფაქტზე, გადააჭარბეთ SBOM წყაროს მეშვეობით
`--openapi-sbom-source`, გამოტოვეთ გარკვეული დატვირთვა (`--skip-openapi`/`--skip-sbom`),
და მიუთითეთ არანაგულისხმევი `syft` ორობითი `--syft-bin`-ით.

სკრიპტი ასახავს ყველა ბრძანებას, რომელსაც ის აწარმოებს; დააკოპირეთ ჟურნალი გამოშვების ბილეთში
`package_summary.json`-თან ერთად, რათა მიმომხილველებმა განასხვავონ CAR დაჯესტები, დაგეგმონ
მეტამონაცემები და Sigstore ნაკრების ჰეშები სპელუნგური ad-hoc ჭურვის გამოშვების გარეშე.

## ნაბიჯი 9 - კარიბჭე + SoraDNS დადასტურება

შეწყვეტის გამოცხადებამდე, დაადასტურეთ, რომ ახალი მეტსახელი წყდება SoraDNS-ის მეშვეობით და ეს
კარიბჭეები ახალი მტკიცებულებებია:

1. **გაუშვით ზონდის კარი.** `ci/check_sorafs_gateway_probe.sh` სავარჯიშოები
   `cargo xtask sorafs-gateway-probe` დემო მოწყობილობების წინააღმდეგ
   `fixtures/sorafs_gateway/probe_demo/`. რეალური განლაგებისთვის, მიუთითეთ ზონდი
   სამიზნე ჰოსტის სახელზე:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   ზონდი შიფრავს `Sora-Name`, `Sora-Proof` და `Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` და მარცხდება მანიფესტის მონელებისას,
   TTL-ები, ან GAR-ის შეკვრა დრიფტი.

   მსუბუქი ლაქების შემოწმებისთვის (მაგალითად, როდესაც მხოლოდ შეკვრაა
   შეიცვალა), გაუშვით `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   დამხმარე ამოწმებს დაჭერილ შეკვრას და მოსახერხებელია გამოსაშვებად
   ბილეთები, რომლებსაც სჭირდებათ მხოლოდ სავალდებულო დადასტურება სრული გამოძიების სავარჯიშოს ნაცვლად.

2. **საბურღი მტკიცებულების დაჭერა.** ოპერატორის საბურღი ან PagerDuty მშრალი გაშვებისთვის, შეფუთეთ
   ზონდი `scripts/telemetry/run_sorafs_gateway_probe.sh --სცენარით
   devportal-rollout --…`. შეფუთვა ინახავს სათაურებს/ ჟურნალებს ქვეშ
   `artifacts/sorafs_gateway_probe/<stamp>/`, განახლებები `ops/drill-log.md` და
   (სურვილისამებრ) ააქტიურებს rollback hooks ან PagerDuty payloads. კომპლექტი
   `--host docs.sora`, რათა დაადასტუროს SoraDNS გზა, IP-ის მყარი კოდირების ნაცვლად.3. ** გადაამოწმეთ DNS-ის კავშირები.** როცა მმართველობა აქვეყნებს ალიასის მტკიცებულებას, ჩაწერეთ
   GAR ფაილი, რომელიც მითითებულია ზონდში (`--gar`) და მიამაგრეთ იგი გამოშვებას
   მტკიცებულება. Resolver-ის მფლობელებს შეუძლიათ იგივე შეყვანის ასახვა
   `tools/soradns-resolver` იმის უზრუნველსაყოფად, რომ ქეშირებული ჩანაწერები დაიცვან ახალი მანიფესტი.
   JSON-ის დამაგრებამდე გაუშვით
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   ასე რომ, მასპინძლის განმსაზღვრელი რუკების, მანიფესტის მეტამონაცემების და ტელემეტრიის ეტიკეტები არის
   დადასტურებულია ხაზგარეშე. დამხმარეს შეუძლია გამოუშვას `--json-out` რეზიუმე გვერდით
   ხელი მოაწერა GAR-ს, რათა მიმომხილველებს ჰქონდეთ გადამოწმებადი მტკიცებულებები ბინარის გახსნის გარეშე.
  ახალი GAR-ის შედგენისას უპირატესობა მიანიჭეთ
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (დაბრუნდით `--manifest-cid <cid>`-ზე მხოლოდ მაშინ, როდესაც manifest ფაილი არ არის
  ხელმისაწვდომი). დამხმარე ახლა იღებს CID ** და ** BLAKE3 დაიჯესტს პირდაპირ საიდან
  მანიფესტის JSON, წყვეტს თეთრ სივრცეს, ამოიღებს განმეორებით `--telemetry-label`-ს
  მონიშვნა, ახარისხებს ლეიბლებს და გამოსცემს ნაგულისხმევ CSP/HSTS/Permissions-პოლიტიკას
  შაბლონები JSON-ის დაწერამდე, ასე რომ დატვირთვა დეტერმინისტული რჩება მაშინაც კი, როცა
  ოპერატორები იჭერენ ეტიკეტებს სხვადასხვა ჭურვიდან.

4. **უყურეთ მეტსახელის მეტიკას.** შეინახეთ `torii_sorafs_alias_cache_refresh_duration_ms`
   და `torii_sorafs_gateway_refusals_total{profile="docs"}` ეკრანზე, ხოლო
   ზონდი მუშაობს; ორივე სერია ჩარტშია
   `dashboards/grafana/docs_portal.json`.

## ნაბიჯი 10 - მონიტორინგი და მტკიცებულებების შეფუთვა

- **Dashboards.** ექსპორტი `dashboards/grafana/docs_portal.json` (პორტალი SLOs),
  `dashboards/grafana/sorafs_gateway_observability.json` (კარიბჭის შეყოვნება +
  ჯანმრთელობის მტკიცებულება), და `dashboards/grafana/sorafs_fetch_observability.json`
  (ორკესტრის ჯანმრთელობა) ყოველი გამოშვებისთვის. მიამაგრეთ JSON ექსპორტი
  გაათავისუფლეთ ბილეთი, რათა მიმომხილველებმა შეძლონ Prometheus მოთხოვნების გამეორება.
- **გამოძიების არქივები.** შეინახეთ `artifacts/sorafs_gateway_probe/<stamp>/` git-დანართში
  ან თქვენი მტკიცებულებების თაიგული. ჩართეთ გამოძიების რეზიუმე, სათაურები და PagerDuty
  ტელემეტრიული სკრიპტით აღბეჭდილი დატვირთვა.
- ** პაკეტის გამოშვება. ** შეინახეთ პორტალი/SBOM/OpenAPI CAR შეჯამებები, მანიფესტი
  პაკეტები, Sigstore ხელმოწერები, `portal.pin.report.json`, Try-It probe ჟურნალები და
  ბმულის შემოწმება ანგარიშების ერთი დროის შტამპით საქაღალდეში (მაგალითად,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **საბურღი ჟურნალი.** როცა ზონდები საბურღი ნაწილია, ნება
  `scripts/telemetry/run_sorafs_gateway_probe.sh` დაურთოს `ops/drill-log.md`
  ასე რომ, იგივე მტკიცებულება აკმაყოფილებს SNNet-5 ქაოსის მოთხოვნას.
- **ბილეთების ბმულები.** მიუთითეთ Grafana პანელის ID ან თანდართული PNG ექსპორტი
  შეცვლის ბილეთი, გამოძიების ანგარიშის გზასთან ერთად, ასე რომ, ცვლილებები-მიმომხილველები
  შეუძლია SLO-ების ჯვარედინი შემოწმება ჭურვის წვდომის გარეშე.

## ნაბიჯი 11 - მრავალ წყაროს მოტანის საბურღი და ანგარიშის დაფის მტკიცებულება

SoraFS-ზე გამოქვეყნება ახლა მოითხოვს მრავალ წყაროს მოპოვების მტკიცებულებებს (DOCS-7/SF-6)
ზემოთ DNS/gateway მტკიცებულებებთან ერთად. მანიფესტის ჩამაგრების შემდეგ:

1. **გაუშვით `sorafs_fetch` პირდაპირი მანიფესტის წინააღმდეგ.** გამოიყენეთ იგივე გეგმა/მანიფესტი
   2–3 ნაბიჯებში წარმოებული არტეფაქტები პლუს თითოეულისთვის გაცემული კარიბჭის სერთიფიკატები
   პროვაიდერი. შეასრულეთ ყველა გამომავალი, რათა აუდიტორებმა შეძლონ ორკესტრის გამეორება
   გადაწყვეტილების ბილიკი:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - ჯერ მიიღეთ პროვაიდერის რეკლამები, რომლებიც მითითებულია მანიფესტში (მაგალითად
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     და გაიარეთ ისინი `--provider-advert name=path`-ის მეშვეობით, რათა დაფასმა შეძლოს
     შეაფასეთ შესაძლებლობები windows-ის განმსაზღვრელად. გამოყენება
     `--allow-implicit-provider-metadata` **მხოლოდ** მოწყობილობების ხელახლა დაკვრისას
     CI; საწარმოო წვრთნებში უნდა იყოს მითითებული ხელმოწერილი რეკლამები, რომლებიც დაეშვათ
     ქინძისთავი.
   - როდესაც manifest მიუთითებს დამატებით რეგიონებზე, გაიმეორეთ ბრძანება
     შესაბამისი პროვაიდერი მრავლდება, ასე რომ ყველა ქეშს/ალიასს აქვს შესაბამისი
     არტეფაქტის მოტანა.

2. **გამოსვლების დაარქივება.** შეინახეთ `scoreboard.json`,
   `providers.ndjson`, `fetch.json` და `chunk_receipts.ndjson` ქვეშ
   გაათავისუფლე მტკიცებულებათა საქაღალდე. ეს ფაილები აღრიცხავს თანატოლთა წონას, სცადეთ ხელახლა
   ბიუჯეტი, შეყოვნება EWMA და თითო ცალი ქვითრები, რომლებიც უნდა იყოს მმართველი პაკეტი
   შეინარჩუნეთ SF-7.

3. **განახლეთ ტელემეტრია.** ჩაწერეთ გამოსავლების იმპორტი **SoraFS Fetch-ში
   დაკვირვებადობა** დაფა (`dashboards/grafana/sorafs_fetch_observability.json`),
   ვუყურებ `torii_sorafs_fetch_duration_ms`/`_failures_total` და
   პროვაიდერის დიაპაზონის პანელები ანომალიებისთვის. დააკავშირეთ Grafana პანელის სნეპშოტები
   გამოშვების ბილეთი საანგარიშო დაფის გასწვრივ.

4. **მოწიეთ გაფრთხილების წესები.** გაუშვით `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   Prometheus გაფრთხილების ნაკრების დასადასტურებლად გამოშვების დახურვამდე. მიამაგრეთ
   promtool გამომავალი ბილეთზე, რათა DOCS-7 მიმომხილველებმა დაადასტურონ გაჩერება
   და ნელი პროვაიდერის სიგნალიზაცია რჩება შეიარაღებული.

5. **გადაიყვანეთ CI-ში.** პორტალის პინის სამუშაო პროცესი ინარჩუნებს `sorafs_fetch` ნაბიჯს უკან
   `perform_fetch_probe` შეყვანა; ჩართეთ იგი დადგმისთვის/წარმოების გაშვებისთვის ისე
   მოპოვების მტკიცებულება წარმოებულია მანიფესტის პაკეტთან ერთად სახელმძღვანელოს გარეშე
   ჩარევა. ადგილობრივ სავარჯიშოებს შეუძლიათ იგივე სკრიპტის ხელახლა გამოყენება ექსპორტით
   კარიბჭის ნიშნები და `PIN_FETCH_PROVIDERS` დაყენება მძიმით გამოყოფილი
   პროვაიდერების სია.

## პოპულარიზაცია, დაკვირვებადობა და უკან დაბრუნება

1. **პრომოცია:** შეინახეთ ცალკე დადგმისა და წარმოების მეტსახელები. დაწინაურება მიერ
   `manifest submit`-ის ხელახლა გაშვება იმავე მანიფესტთან/პაკეტით, შეცვლა
   `--alias-namespace/--alias-name` წარმოების მეტსახელის აღსანიშნავად. ეს
   თავიდან აიცილებს აღდგენას ან გადადგომას მას შემდეგ, რაც QA დაამტკიცებს დადგმის პინს.
2. **მონიტორინგი:** პინ-რეესტრის დაფის იმპორტი
   (`docs/source/grafana_sorafs_pin_registry.json`) პლუს პორტალისთვის სპეციფიკური
   ზონდები (იხ. `docs/portal/docs/devportal/observability.md`). გაფრთხილება საკონტროლო ჯამზე
   დრიფტი, წარუმატებელი ზონდები ან მტკიცებულება ხელახლა ცდის მწვერვალები.
3. **დაბრუნება:** დასაბრუნებლად, ხელახლა გაგზავნეთ წინა მანიფესტი (ან გააუქმეთ
   მიმდინარე მეტსახელი) `sorafs_cli manifest submit --alias ... --retire`-ის გამოყენებით.
   ყოველთვის შეინახეთ ბოლო ცნობილი კარგი პაკეტი და CAR შეჯამება, რათა უკან დაბრუნების მტკიცებულებები შეძლონ
   ხელახლა შეიქმნება, თუ CI ჟურნალები ბრუნავს.

## CI სამუშაო ნაკადის შაბლონი

თქვენი მილსადენი მინიმუმ უნდა:

1. Build + lint (`npm ci`, `npm run build`, საკონტროლო ჯამის გენერაცია).
2. პაკეტი (`car pack`) და გამოთვალეთ მანიფესტები.
3. მოაწერეთ ხელი სამუშაოს ფარგლებში OIDC ჟეტონის (`manifest sign`) გამოყენებით.
4. ატვირთეთ არტეფაქტები (CAR, მანიფესტი, ნაკრები, გეგმა, რეზიუმეები) აუდიტისთვის.
5. გაგზავნეთ პინის რეესტრში:
   - გაიყვანეთ მოთხოვნები → `docs-preview.sora`.
   - ტეგები / დაცული ფილიალები → წარმოების მეტსახელი ხელშეწყობა.
6. გაუშვით ზონდები + მტკიცებულების დამადასტურებელი კარიბჭე გასვლამდე.

`.github/workflows/docs-portal-sorafs-pin.yml` ხაზს უსვამს ყველა ამ ნაბიჯს
ხელით გამოშვებისთვის. სამუშაო პროცესი:

- აშენებს/ამოწმებს პორტალს,
- აწყობს კონსტრუქციას `scripts/sorafs-pin-release.sh`-ით,
- ხელს აწერს/ამოწმებს manifest-ის პაკეტს GitHub OIDC-ის გამოყენებით,
- ატვირთავს CAR/მანიფესტს/ნაკრებს/გეგმის/მტკიცებულების შეჯამებებს არტეფაქტებად და
- (სურვილისამებრ) წარუდგენს მანიფესტს + მეტსახელის სავალდებულოა, როდესაც საიდუმლოებები არსებობს.

სამუშაოს დაწყებამდე დააკონფიგურირეთ საცავის საიდუმლოებები/ცვლადები:

| სახელი | დანიშნულება |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii ჰოსტი, რომელიც ავლენს `/v1/sorafs/pin/register`-ს. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | ეპოქის იდენტიფიკატორი ჩაწერილია წარდგინებით. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | მანიფესტის წარდგენის ხელმომწერი უფლებამოსილება. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | მეტსახელის ტიპი მიბმულია მანიფესტთან, როდესაც `perform_submit` არის `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-ში დაშიფრული ალიასის დამადასტურებელი ნაკრები (არასავალდებულო; გამოტოვეთ ფსევდონიმების დაკავშირება). |
| `DOCS_ANALYTICS_*` | არსებული ანალიტიკა/გამოძიების საბოლოო წერტილები, რომლებიც ხელახლა გამოიყენება სხვა სამუშაო პროცესების მიერ. |

გააქტიურეთ სამუშაო ნაკადი Actions UI-ის მეშვეობით:

1. მიაწოდეთ `alias_label` (მაგ., `docs.sora.link`), სურვილისამებრ `proposal_alias`,
   და სურვილისამებრ `release_tag` უგულებელყოფა.
2. დატოვეთ `perform_submit` მონიშნული არტეფაქტების გენერირებისთვის Torii-ის შეხების გარეშე
   (გამოსადეგია მშრალი გაშვებისთვის) ან ჩართეთ მისი გამოქვეყნება პირდაპირ კონფიგურირებულში
   მეტსახელი.

`docs/source/sorafs_ci_templates.md` ჯერ კიდევ დოკუმენტირებულია ზოგადი CI დამხმარეებისთვის
პროექტები ამ საცავის მიღმა, მაგრამ უპირატესობა უნდა მიენიჭოს პორტალზე მუშაობის პროცესს
ყოველდღიური გამოშვებისთვის.

## საკონტროლო სია

- [ ] `npm run build`, `npm run test:*` და `npm run check:links` მწვანეა.
- [ ] `build/checksums.sha256` და `build/release.json` აღბეჭდილი არტეფაქტებში.
- [ ] CAR, გეგმა, მანიფესტი და შეჯამება გენერირებული `artifacts/`-ის ქვეშ.
- [ ] Sigstore პაკეტი + მოწყვეტილი ხელმოწერა შენახული ჟურნალებით.
- [ ] `portal.manifest.submit.summary.json` და `portal.manifest.submit.response.json`
      გადაღებული წარდგენის დროს.
- [ ] `portal.pin.report.json` (და სურვილისამებრ `portal.pin.proposal.json`)
      დაარქივებულია CAR/მანიფესტის არტეფაქტებთან ერთად.
- [ ] `proof verify` და `manifest verify-signature` ჟურნალები დაარქივებულია.
- [ ] განახლებულია Grafana დაფები + Try-It probes წარმატებით.
- [ ] უკან დაბრუნების შენიშვნები (წინა მანიფესტის ID + მეტსახელი დაიჯესტი) მიმაგრებულია
      გათავისუფლების ბილეთი.