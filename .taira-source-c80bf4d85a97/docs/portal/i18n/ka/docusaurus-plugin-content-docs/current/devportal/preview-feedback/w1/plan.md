---
id: preview-feedback-w1-plan
lang: ka
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| ნივთი | დეტალები |
| --- | --- |
| ტალღა | W1 — პარტნიორები და Torii ინტეგრატორები |
| სამიზნე ფანჯარა | Q2 2025 კვირა 3 |
| არტეფაქტის ტეგი (დაგეგმილი) | `preview-2025-04-12` |
| ტრეკერის საკითხი | `DOCS-SORA-Preview-W1` |

## მიზნები

1. დაიცავით პარტნიორის გადახედვის პირობების სამართლებრივი + მმართველობის დამტკიცებები.
2. დადგმეთ Try it proxy და ტელემეტრიის სნეპშოტები, რომლებიც გამოიყენება მოწვევის პაკეტში.
3. განაახლეთ საკონტროლო ჯამით დამოწმებული წინასწარი გადახედვის არტეფაქტი და გამოძიების შედეგები.
4. დაასრულეთ პარტნიორების სია + მოთხოვნის შაბლონები მოწვევის გაგზავნამდე.

## დავალების დაშლა

| ID | ამოცანა | მფლობელი | ვადა | სტატუსი | შენიშვნები |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | მიიღეთ კანონიერი დამტკიცება წინასწარი გადახედვის პირობების დამატებაზე | Docs/DevRel წამყვანი → Legal | 2025-04-05 | ✅ დასრულებული | იურიდიული ბილეთი `DOCS-SORA-Preview-W1-Legal` გაფორმებულია 2025-04-05; PDF მიმაგრებულია ტრეკერზე. |
| W1-P2 | გადაიღეთ სცადეთ პროქსის დადგმის ფანჯარა (2025-04-10) და დაადასტურეთ პროქსის ჯანმრთელობა | Docs/DevRel + Ops | 2025-04-06 | ✅ დასრულებული | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` შესრულებული 2025-04-06; CLI ტრანსკრიპტი + `.env.tryit-proxy.bak` დაარქივებულია. |
| W1-P3 | შექმენით წინასწარი გადახედვის არტეფაქტი (`preview-2025-04-12`), გაუშვით `scripts/preview_verify.sh` + `npm run probe:portal`, არქივის აღმწერი/შემოწმების ჯამები | პორტალი TL | 2025-04-08 | ✅ დასრულებული | `artifacts/docs_preview/W1/preview-2025-04-12/`-ში შენახული არტეფაქტი + ვერიფიკაციის ჟურნალები; ზონდის გამომავალი მიმაგრებულია ტრეკერზე. |
| W1-P4 | პარტნიორის მიღების ფორმების გადახედვა (`DOCS-SORA-Preview-REQ-P01…P08`), დაადასტურეთ კონტაქტები + NDA | მმართველობითი კავშირი | 2025-04-07 | ✅ დასრულებული | რვავე მოთხოვნა დამტკიცდა (ბოლო ორი გაწმენდილი 2025-04-11); დამტკიცებები დაკავშირებულია ტრეკერში. |
| W1-P5 | მოწვევის ასლის პროექტი (`docs/examples/docs_preview_invite_template.md`-ზე დაყრდნობით), კომპლექტი `<preview_tag>` და `<request_ticket>` თითოეული პარტნიორისთვის | Docs/DevRel წამყვანი | 2025-04-08 | ✅ დასრულებული | მოწვევის მონახაზი გაიგზავნა 2025-04-12 15:00UTC არტეფაქტის ბმულებთან ერთად. |

## ფრენის წინ საკონტროლო სია

> რჩევა: გაუშვით `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` 1-5 ნაბიჯების ავტომატურად შესასრულებლად (აშენება, საკონტროლო ჯამის დადასტურება, პორტალის გამოკვლევა, ბმულის შემოწმება და სცადეთ პროქსის განახლება). სკრიპტი ჩაწერს JSON ჟურნალს, რომელიც შეგიძლიათ დაურთოთ ტრეკერის პრობლემას.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`-ით) `build/checksums.sha256` და `build/release.json`-ის რეგენერაციისთვის.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` და დაარქივეთ `build/link-report.json` აღწერის გვერდით.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ან მიაწოდეთ შესაბამისი სამიზნე `--tryit-target`-ის მეშვეობით); ჩააბარეთ განახლებული `.env.tryit-proxy` და შეინახეთ `.bak` უკან დასაბრუნებლად.
6. განაახლეთ W1 ტრეკერის საკითხი ჟურნალის ბილიკებით (აღმწერის შემოწმების ჯამი, გამოძიების გამომავალი, სცადეთ პროქსის შეცვლა, Grafana სნეპშოტები).

## მტკიცებულებათა ჩამონათვალი

- [x] ხელმოწერილი იურიდიული დამტკიცება (PDF ან ბილეთის ბმული) მიმაგრებული `DOCS-SORA-Preview-W1`-ზე.
- [x] Grafana ეკრანის ანაბეჭდები `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] `preview-2025-04-12` აღმწერი + საკონტროლო ჯამის ჟურნალი, რომელიც ინახება `artifacts/docs_preview/W1/`-ში.
- [x] მოიწვიეთ ჩამონათვალის ცხრილი `invite_sent_at` დროის შტამპებით დასახლებული (იხილეთ ტრეკერის W1 ჟურნალი).
- [x] გამოხმაურების არტეფაქტები ასახულია [`preview-feedback/w1/log.md`]-ში (./log.md) თითო მწკრივით თითო პარტნიორთან (განახლებულია 2025-04-26 სიაში/ტელემეტრია/გამოშვების მონაცემებით).

განაახლეთ ეს გეგმა, როგორც ამოცანები პროგრესირებს; ტრეკერი მიუთითებს მას საგზაო რუქის შესანარჩუნებლად
აუდიტორული.

## უკუკავშირის სამუშაო პროცესი

1. თითოეული მიმომხილველისთვის, დააკოპირეთ შაბლონი
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   შეავსეთ მეტამონაცემები და შეინახეთ დასრულებული ასლი ქვემოთ
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. შეაჯამეთ მოწვევები, ტელემეტრიის საგუშაგოები და ღია საკითხები პირდაპირ ჟურნალში
   [`preview-feedback/w1/log.md`](./log.md), რათა მმართველობის მიმომხილველებმა შეძლონ მთელი ტალღის გამეორება
   საცავიდან გაუსვლელად.
3. როდესაც ცოდნის შემოწმების ან კვლევის ექსპორტი მოდის, მიამაგრეთ ისინი ჟურნალში მითითებული არტეფაქტის გზაზე
   და დააკავშირეთ ტრეკერის საკითხი.