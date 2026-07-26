<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API Runbook

Այս գրքույկը ներառում է արտադրության վրա հիմնված տեղակայումը և գործողությունները հետևյալի համար.

- Vue3 ստատիկ կայք (`--template site`); և
- Vue3 SPA + API ծառայություն (`--template webapp`),

օգտագործելով Soracloud կառավարման ինքնաթիռի API-ներ Iroha 3-ում SCR/IVM ենթադրություններով (ոչ
WASM գործարկման կախվածություն և առանց Docker կախվածություն):

## 1. Ստեղծեք կաղապարային նախագծեր

Կայքի ստատիկ փայտամած.

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API փայտամած:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Յուրաքանչյուր ելքային գրացուցակ ներառում է.

- `container_manifest.json`
- `service_manifest.json`
- կաղապարի սկզբնաղբյուր ֆայլեր `site/` կամ `webapp/` տակ

## 2. Կառուցեք հավելվածի արտեֆակտներ

Ստատիկ կայք.

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA frontend + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Փաթեթավորեք և հրապարակեք ճակատային ակտիվները

SoraFS-ի միջոցով ստատիկ հոստինգի համար.

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA ճակատի համար.

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Տեղադրեք կենդանի Soracloud կառավարման ինքնաթիռ

Տեղադրեք ստատիկ կայքի ծառայություն.

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Տեղադրեք SPA + API ծառայություն.

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Վավերացնել երթուղու կապակցման և թողարկման վիճակը.

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Սպասվող կառավարման ինքնաթիռի ստուգումներ.

- `control_plane.services[].latest_revision.route_host` հավաքածու
- `control_plane.services[].latest_revision.route_path_prefix` հավաքածու (`/` կամ `/api`)
- `control_plane.services[].active_rollout` առկա է թարմացումից անմիջապես հետո

## 5. Թարմացրեք առողջապահական ծրագրով

1. Զարկեք `service_version` ծառայության մանիֆեստում:
2. Գործարկել արդիականացումը.

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Խթանել շրջանառությունը առողջական ստուգումներից հետո.

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Եթե առողջությունը վատանում է, ապա տեղեկացրեք անառողջ.```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Երբ անառողջ հաշվետվությունները հասնում են քաղաքականության շեմին, Soracloud-ը ավտոմատ կերպով անցնում է
վերադառնալ ելակետային վերանայմանը և գրանցել հետադարձ աուդիտի իրադարձությունները:

## 6. Ձեռքով հետ վերադարձ և միջադեպի արձագանք

Վերադարձ դեպի նախորդ տարբերակ.

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Օգտագործեք կարգավիճակի ելքը հաստատելու համար.

- `current_version` հետ է վերադարձվել
- `audit_event_count` ավելացված
- `active_rollout` մաքրված է
- `last_rollout.stage`-ը `RolledBack` է ավտոմատ հետադարձման համար

## 7. Գործառնությունների ստուգաթերթ

- Կաղապարի կողմից ստեղծված մանիֆեստները պահեք տարբերակի հսկողության տակ:
- Գրանցեք `governance_tx_hash` յուրաքանչյուր թողարկման քայլի համար՝ հետագծելիությունը պահպանելու համար:
- Վերաբերվեք `service_health`, `routing`, `resource_pressure` և
  `failed_admissions` որպես բացվող դարպասի մուտքեր:
- Օգտագործեք դեղձանիկների տոկոսները և բացահայտ առաջխաղացումը, այլ ոչ թե ուղղակի ամբողջական կտրումը
  օգտատերերի համար նախատեսված ծառայությունների բարելավումներ:
- Վավերացրեք նստաշրջանի/հավաստագրման և ստորագրության ստուգման վարքագիծը
  `webapp/api/server.mjs` մինչև արտադրությունը: