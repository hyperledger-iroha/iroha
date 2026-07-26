<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
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

ეს სახელმძღვანელო მოიცავს წარმოებაზე ორიენტირებულ განლაგებას და ოპერაციებს:

- Vue3 სტატიკური საიტი (`--template site`); და
- Vue3 SPA + API სერვისი (`--template webapp`),

Soracloud Control-plane API-ების გამოყენებით Iroha 3-ზე SCR/IVM ვარაუდებით (არა
WASM გაშვების დროზე დამოკიდებულება და არანაირი Docker დამოკიდებულება).

## 1. შექმენით შაბლონური პროექტები

საიტის სტატიკური ხარაჩო:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API ხარაჩო:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

თითოეული გამომავალი დირექტორია მოიცავს:

- `container_manifest.json`
- `service_manifest.json`
- შაბლონის წყაროს ფაილები `site/` ან `webapp/` ქვეშ

## 2. შექმენით განაცხადის არტეფაქტები

სტატიკური საიტი:

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

## 3. შეფუთეთ და გამოაქვეყნეთ წინა ნაწილის აქტივები

სტატიკური ჰოსტინგისთვის SoraFS-ის საშუალებით:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA ფრონტენტისთვის:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. განათავსეთ ცოცხალი Soracloud საკონტროლო თვითმფრინავი

განათავსეთ სტატიკური საიტის სერვისი:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

განათავსეთ SPA + API სერვისი:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

მარშრუტის შეკვრისა და გაშვების მდგომარეობის დადასტურება:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

მოსალოდნელი საკონტროლო თვითმფრინავის შემოწმება:

- `control_plane.services[].latest_revision.route_host` კომპლექტი
- `control_plane.services[].latest_revision.route_path_prefix` კომპლექტი (`/` ან `/api`)
- `control_plane.services[].active_rollout` იმყოფება განახლებისთანავე

## 5. განაახლეთ ჯანდაცვის შეზღუდვით გაშვებით

1. ამოწურვა `service_version` სერვისის მანიფესტში.
2. განახორციელეთ განახლება:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. ხელი შეუწყეთ გავრცელებას ჯანმრთელობის შემოწმების შემდეგ:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. თუ ჯანმრთელობა გაუარესებულია, შეატყობინეთ არაჯანსაღს:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

როდესაც არაჯანსაღი ანგარიშები მიაღწევს პოლიტიკის ზღურბლს, Soracloud ავტომატურად შემოდის
უბრუნდება საბაზისო რევიზიას და აღრიცხავს უკანა აუდიტის მოვლენებს.

## 6. ხელით უკან დაბრუნება და ინციდენტზე რეაგირება

წინა ვერსიაზე დაბრუნება:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

გამოიყენეთ სტატუსის გამომავალი დასადასტურებლად:

- `current_version` დაბრუნდა
- `audit_event_count` გაიზარდა
- `active_rollout` გასუფთავებულია
- `last_rollout.stage` არის `RolledBack` ავტომატური დაბრუნებისთვის

## 7. ოპერაციების ჩამონათვალი

- შეინახეთ შაბლონის მიერ გენერირებული მანიფესტები ვერსიის კონტროლის ქვეშ.
- ჩაწერეთ `governance_tx_hash` ყოველი გაშვების საფეხურზე მიკვლევადობის შესანარჩუნებლად.
- მკურნალობა `service_health`, `routing`, `resource_pressure` და
  `failed_admissions`, როგორც გასაშლელი კარიბჭის შეყვანა.
- გამოიყენეთ კანარის პროცენტები და აშკარა პოპულარიზაცია, ვიდრე პირდაპირი სრული მოჭრა
  განახლებები მომხმარებლისთვის მიმართული სერვისებისთვის.
- შეამოწმეთ სესიის/ავტორის და ხელმოწერის გადამოწმების ქცევა
  `webapp/api/server.mjs` წარმოებამდე.