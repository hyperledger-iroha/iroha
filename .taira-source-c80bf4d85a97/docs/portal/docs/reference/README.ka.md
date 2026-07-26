---
lang: ka
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-12-29T18:16:35.156432+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

ეს განყოფილება აერთიანებს მასალას „წაიკითხე როგორც სპეციფიკაცია“ Iroha-ისთვის. ეს გვერდები რჩება სტაბილურად მაშინაც კი, როცა
სახელმძღვანელოები და გაკვეთილები ვითარდება.

## ხელმისაწვდომია დღეს

- **Norito კოდეკის მიმოხილვა** – `reference/norito-codec.md` ბმულებს პირდაპირ ავტორიტეტულთან
  `norito.md` სპეციფიკაცია პორტალის ცხრილის შევსებისას.
- **Torii OpenAPI** – `/reference/torii-openapi` გადმოსცემს უახლეს Torii REST სპეციფიკაციას გამოყენებით
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v1/mcp`.
  რედოკ. განაახლეთ სპეციფიკაცია `npm run sync-openapi -- --version=current --latest`-ით (დაამატეთ
  `--mirror=<label>` სნეპშოტის დამატებით ისტორიულ ვერსიებში კოპირებისთვის).
- **კონფიგურაციის ცხრილები** - სრული პარამეტრის კატალოგი ინახება
  `docs/source/references/configuration.md`. სანამ პორტალი არ გაგზავნის ავტომატურ იმპორტს, მიუთითეთ ეს
  Markdown ფაილი ზუსტი ნაგულისხმევი და გარემოს უგულებელყოფისთვის.
- **Docs versioning** – navbar-ის ვერსიის ჩამოსაშლელი მენიუ ასახავს გაყინულ კადრებს, რომლებიც შექმნილია
  `npm run docs:version -- <label>`, რაც გაადვილებს მითითებების შედარებას გამოშვებებში.

## მალე

- **Torii REST მითითება** – OpenAPI განმარტებები სინქრონიზებული იქნება ამ სექციაში მეშვეობით
  `docs/portal/scripts/sync-openapi.mjs` მილსადენის ჩართვის შემდეგ.
- **CLI ბრძანების ინდექსი** – გენერირებული ბრძანების მატრიცა (`crates/iroha_cli/src/commands`)
  აქ დაჯდება კანონიკურ მაგალითებთან ერთად.
- **IVM ABI ცხრილები** – მაჩვენებლის ტიპის და syscall მატრიცები (შენახულია `crates/ivm/docs`-ში)
  გადაიცემა პორტალში, როგორც კი დოკუმენტის გენერირების სამუშაო გაფორმდება.

## ამ ინდექსის მიმდინარეობის შენარჩუნება

როდესაც დაემატება ახალი საცნობარო მასალა - გენერირებული API დოკუმენტები, კოდეკის სპეციფიკაციები, კონფიგურაციის მატრიცები - ადგილი
გვერდი `docs/portal/docs/reference/`-ის ქვეშ და დააკავშირეთ იგი ზემოთ. თუ გვერდი ავტოგენერირდება, გაითვალისწინეთ
სკრიპტის სინქრონიზაცია, რათა მომხმარებლებმა იცოდნენ, როგორ განაახლონ იგი. ეს ინახავს საცნობარო ხეს გამოსადეგი მანამ, სანამ
სრულად ავტოგენერირებული სანავიგაციო მიწები.
