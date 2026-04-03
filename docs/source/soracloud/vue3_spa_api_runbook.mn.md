<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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

Энэхүү runbook нь үйлдвэрлэлд чиглэсэн байршуулалт, үйл ажиллагааг хамардаг:

- Vue3 статик сайт (`--template site`); болон
- Vue3 SPA + API үйлчилгээ (`--template webapp`),

SCR/IVM таамаглал бүхий Iroha 3 дээр Soracloud хяналтын хавтгай API-г ашиглах (үгүй)
WASM ажиллах цагийн хамаарал, Docker хамаарал байхгүй).

## 1. Загвар төсөл үүсгэх

Статик сайтын шат:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API шат:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Гаралтын лавлах бүр нь:

- `container_manifest.json`
- `service_manifest.json`
- `site/` эсвэл `webapp/` доорх загвар эх файлууд

## 2. Хэрэглээний олдворуудыг бүтээх

Статик сайт:

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

## 3. Урд талын хөрөнгийг багцлах, нийтлэх

SoraFS-ээр дамжуулан статик байршуулахын тулд:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA урд талын хувьд:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Soracloud-ийн удирдлагын онгоцонд байршуулах

Статик сайтын үйлчилгээг байрлуулах:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API үйлчилгээг ашиглах:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Маршрутыг холбох болон нэвтрүүлэх төлөвийг баталгаажуулах:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Хяналтын онгоцны хүлээгдэж буй шалгалтууд:

- `control_plane.services[].latest_revision.route_host` багц
- `control_plane.services[].latest_revision.route_path_prefix` багц (`/` эсвэл `/api`)
- `control_plane.services[].active_rollout` шинэчлэгдсэний дараа шууд гарч ирнэ

## 5. Эрүүл мэндийн хамгаалалттай хувилбараар сайжруул

1. Үйлчилгээний манифест дотор `service_version` цохино.
2. Шинэчлэлтийг эхлүүлэх:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Эрүүл мэндийн үзлэгт хамрагдсаны дараа нэвтрүүлэх ажлыг дэмжих:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Хэрэв эрүүл мэнд муудаж байвал эрүүл бус гэж мэдэгдээрэй:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Эрүүл бус тайлан бодлогын босго хэмжээнд хүрэхэд Soracloud автоматаар эргэлддэг
үндсэн засвар руу буцах ба аудитын үйл явдлыг буцаах.

## 6. Гараар буцаах болон ослын хариу арга хэмжээ

Өмнөх хувилбар руу буцах:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Баталгаажуулахын тулд төлөвийн гаралтыг ашиглана уу:

- `current_version` буцаагдсан
- `audit_event_count` нэмэгдсэн
- `active_rollout` арилсан
- `last_rollout.stage` нь автоматаар буцаахад зориулагдсан `RolledBack`

## 7. Үйл ажиллагааны хяналтын хуудас

- Загвараар үүсгэгдсэн манифестуудыг хувилбарын хяналтанд байлга.
- Мөрдөх чадварыг хадгалахын тулд нэвтрүүлэх алхам бүрд `governance_tx_hash` тэмдэглэнэ үү.
- `service_health`, `routing`, `resource_pressure`, болон
  `failed_admissions` оролтыг оруулах.
- Канарын хувь хэмжээ, шууд сурталчилгаа гэхээсээ илүүтэйгээр сурталчилгааг ашиглаарай
  хэрэглэгчдэд чиглэсэн үйлчилгээний шинэчлэлтүүд.
- Сесс/auth болон гарын үсэг баталгаажуулах үйлдлийг баталгаажуулах
  Үйлдвэрлэлийн өмнө `webapp/api/server.mjs`.