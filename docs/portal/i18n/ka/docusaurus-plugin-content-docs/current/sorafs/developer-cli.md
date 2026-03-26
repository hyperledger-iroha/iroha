---
id: developer-cli
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

კონსოლიდირებული `sorafs_cli` ზედაპირი (მოწოდებული `sorafs_car` ყუთით
`cli` ფუნქცია ჩართულია) ასახავს SoraFS მოსამზადებლად საჭირო ყველა ნაბიჯს
არტეფაქტები. გამოიყენეთ ეს კულინარიული წიგნი, რათა პირდაპირ გადახვიდეთ საერთო სამუშაო პროცესებზე; დააწყვილეთ იგი
მანიფესტის მილსადენის და ორკესტრის წიგნები ოპერატიული კონტექსტისთვის.

## პაკეტის დატვირთვები

გამოიყენეთ `car pack` დეტერმინისტული მანქანის არქივებისა და ნაწილაკების გეგმების შესაქმნელად. The
ბრძანება ავტომატურად ირჩევს SF-1 ცუნკერს, თუ სახელური არ არის მოწოდებული.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- ნაგულისხმევი ჭურჭლის სახელური: `sorafs.sf1@1.0.0`.
- დირექტორიაში შეყვანები შესრულებულია ლექსიკოგრაფიული თანმიმდევრობით, რათა შემოწმებული ჯამები დარჩეს სტაბილური
  პლატფორმების გასწვრივ.
- JSON-ის შეჯამება მოიცავს დატვირთვის შეჯამებებს, თითო ნაწილზე მეტამონაცემებს და root-ს
  CID აღიარებულია რეესტრისა და ორკესტრის მიერ.

## კონსტრუქციის მანიფესტი

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` პარამეტრების რუკა პირდაპირ `PinPolicy` ველებზე
  `sorafs_manifest::ManifestBuilder`.
- მიაწოდეთ `--chunk-plan`, როდესაც გსურთ CLI-მ SHA3 ნაწილის ხელახლა გამოთვლა
  დაიჯესტი წარდგენამდე; წინააღმდეგ შემთხვევაში ის ხელახლა იყენებს მასში ჩაშენებულ დაიჯესტს
  შეჯამება.
- JSON გამომავალი ასახავს Norito დატვირთვას პირდაპირი განსხვავებების დროს
  მიმოხილვები.

## ნიშანი ვლინდება დიდი ხნის გასაღებების გარეშე

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- იღებს შიდა ნიშნებს, გარემოს ცვლადებს ან ფაილზე დაფუძნებულ წყაროებს.
- ამატებს წარმოშობის მეტამონაცემებს (`token_source`, `token_hash_hex`, ნაწილის დაიჯესტი)
  დაუმუშავებელი JWT-ის შენარჩუნების გარეშე, თუ `--include-token=true`.
- კარგად მუშაობს CI-ში: დააკავშირეთ GitHub Actions OIDC პარამეტრით
  `--identity-token-provider=github-actions`.

## გაგზავნეთ მანიფესტები Torii-ზე

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- ასრულებს Norito დეკოდირებას ალიასის მტკიცებულებებისთვის და ადასტურებს, რომ ისინი ემთხვევა
  მანიფესტი დაიჯესტი Torii-ზე გამოქვეყნებამდე.
- ხელახლა გამოთვლის SHA3 ნაწილს გეგმიდან, რათა თავიდან აიცილოს შეუსაბამობის შეტევები.
- პასუხების შეჯამებები აღწერს HTTP სტატუსს, სათაურებს და რეესტრის დატვირთვას
  მოგვიანებით აუდიტი.

## გადაამოწმეთ მანქანის შინაარსი და მტკიცებულებები

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- აღადგენს PoR ხეს და ადარებს დატვირთვის შეჯამებას მანიფესტის შეჯამებასთან.
- ასახავს რაოდენობას და იდენტიფიკატორებს, რომლებიც საჭიროა რეპლიკაციის მტკიცებულებების წარდგენისას
  მმართველობისკენ.

## ნაკადის საწინააღმდეგო ტელემეტრია

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- გამოსცემს NDJSON ელემენტებს თითოეული სტრიმინგირებული მტკიცებულებისთვის (გამორთეთ ხელახალი დაკვრა
  `--emit-events=false`).
- აერთიანებს წარმატებების/წარუმატებლობის რაოდენობას, ლატენტურ ჰისტოგრამებს და შერჩეულ წარუმატებლობებს
  JSON-ის შეჯამება, რათა საინფორმაციო დაფებმა შეძლონ შედეგების დახატვა ჟურნალების ამოღების გარეშე.
- გადის ნულიდან, როდესაც კარიბჭე იტყობინება წარუმატებლობის ან ადგილობრივი PoR დადასტურების შესახებ
  (`--por-root-hex`-ის მეშვეობით) უარყოფს მტკიცებულებებს. დაარეგულირეთ ზღურბლები
  `--max-failures` და `--max-verification-failures` რეპეტიციისთვის.
- მხარს უჭერს PoR დღეს; PDP და PoTR ხელახლა იყენებენ ერთსა და იმავე კონვერტს ერთხელ SF-13/SF-14
  მიწა.
- `--governance-evidence-dir` წერს შეჯამებას, მეტამონაცემებს (დროის ნიშანს,
  CLI ვერსია, კარიბჭის URL, manifest digest) და manifest-ის ასლი
  მოწოდებული დირექტორია, რათა მმართველობის პაკეტებმა შეძლონ მტკიცებულების ნაკადის დაარქივება
  მტკიცებულება გაშვების გამეორების გარეშე.

## დამატებითი მითითებები

- `docs/source/sorafs_cli.md` — ამომწურავი დროშის დოკუმენტაცია.
- `docs/source/sorafs_proof_streaming.md` — მტკიცებულების ტელემეტრიის სქემა და Grafana
  დაფის შაბლონი.
- `docs/source/sorafs/manifest_pipeline.md` — ღრმა ჩაყვინთვა ნატეხზე, მანიფესტი
  შემადგენლობა და მანქანების მართვა.