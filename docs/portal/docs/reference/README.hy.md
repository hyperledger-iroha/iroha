---
lang: hy
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

Այս բաժինը միավորում է Iroha-ի «կարդալ այն որպես բնութագրիչ» նյութը: Այս էջերը մնում են կայուն նույնիսկ այն ժամանակ, երբ
ուղեցույցները և ձեռնարկները զարգանում են:

## Հասանելի է այսօր

- **Norito կոդեկի ակնարկ** – `reference/norito-codec.md` հղումներ ուղղակիորեն դեպի հեղինակավոր
  `norito.md` ճշգրտում, մինչ պորտալի աղյուսակը համալրվում է:
- **Torii OpenAPI** – `/reference/torii-openapi`-ը ներկայացնում է վերջին Torii REST հատկորոշումը` օգտագործելով
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v1/mcp`.
  Վերագր. Վերականգնեք բնութագրերը `npm run sync-openapi -- --version=current --latest`-ով (ավել
  `--mirror=<label>` պատկերը պատճենելու լրացուցիչ պատմական տարբերակներում):
- **Կազմաձևման աղյուսակներ** – Պարամետրերի ամբողջական կատալոգը պահվում է
  `docs/source/references/configuration.md`. Քանի դեռ պորտալը չի ուղարկել ավտոմատ ներմուծում, նշեք դրան
  Markdown ֆայլ ճշգրիտ լռելյայնությունների և շրջակա միջավայրի անտեսումների համար:
- **Փաստաթղթերի տարբերակում** – Navbar տարբերակի բացվող ցանկը բացահայտում է սառեցված նկարները, որոնք ստեղծվել են
  `npm run docs:version -- <label>`, ինչը հեշտացնում է ուղեցույցների համեմատությունը բոլոր թողարկումներում:

## Շուտով

- **Torii REST հղում** – OpenAPI սահմանումները կհամաժամացվեն այս բաժնի մեջ
  `docs/portal/scripts/sync-openapi.mjs`, երբ խողովակաշարը միացված է:
- **CLI հրամանի ինդեքս ** – Ստեղծված հրամանի մատրիցա (հայելային `crates/iroha_cli/src/commands`)
  այստեղ կհանդիպի կանոնական օրինակների կողքին:
- **IVM ABI աղյուսակներ** – Ցուցանիշի տիպի և syscall մատրիցները (պահպանվում են `crates/ivm/docs`-ի ներքո)
  կներկայացվի պորտալում, երբ փաստաթղթի ստեղծման աշխատանքը միացվի:

## Այս ցուցանիշի ընթացիկ պահում

Երբ ավելացվում է նոր տեղեկատու նյութ՝ ստեղծված API փաստաթղթեր, կոդեկի ակնարկներ, կազմաձևման մատրիցաներ.
էջը `docs/portal/docs/reference/` տակ և կապեք այն վերևում: Եթե էջը ավտոմատ կերպով ստեղծվում է, նշեք
համաժամեցրեք սկրիպտը, որպեսզի մասնակիցները իմանան, թե ինչպես թարմացնել այն: Սա պահպանում է հղման ծառը օգտակար մինչև
լիովին ինքնագեներացված նավիգացիոն հողեր:
