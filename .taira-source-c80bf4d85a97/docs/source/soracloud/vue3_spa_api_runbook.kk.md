<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
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

Бұл жұмыс кітабы өндіріске бағытталған орналастыру мен операцияларды қамтиды:

- Vue3 статикалық сайты (`--template site`); және
- Vue3 SPA + API қызметі (`--template webapp`),

SCR/IVM болжамдарымен Iroha 3 жүйесінде Soracloud басқару жазықтығы API интерфейстерін пайдалану (жоқ
WASM орындау уақытына тәуелділік және Docker тәуелділігі жоқ).

## 1. Үлгі жобаларын жасаңыз

Сайттың статикалық тірегі:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API тіректері:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Әрбір шығыс каталогы мыналарды қамтиды:

- `container_manifest.json`
- `service_manifest.json`
- `site/` немесе `webapp/` астында үлгінің бастапқы файлдары

## 2. Қолданба артефактілерін құрастырыңыз

Статикалық сайт:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA интерфейсі + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Frontend активтерін жинақтау және жариялау

SoraFS арқылы статикалық хостинг үшін:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA интерфейсі үшін:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Тікелей Soracloud басқару ұшағына орналастырыңыз

Статикалық торап қызметін орналастыру:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API қызметін қолдану:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Маршрутты байланыстыру және шығару күйін тексеру:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Басқару жазықтығының күтілетін тексерулері:

- `control_plane.services[].latest_revision.route_host` жинағы
- `control_plane.services[].latest_revision.route_path_prefix` жинағы (`/` немесе `/api`)
- `control_plane.services[].active_rollout` жаңартудан кейін бірден пайда болады

## 5. Денсаулықты қорғауға арналған шығарылыммен жаңартыңыз

1. Қызмет манифестінде `service_version` соққысы.
2. Жаңартуды іске қосыңыз:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Денсаулықты тексеруден кейін таратуды алға жылжытыңыз:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Денсаулығы нашар болса, сау емес деп хабарлаңыз:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Дұрыс емес есептер саясат шегіне жеткенде, Soracloud автоматты түрде айналады
бастапқы тексеруге оралу және кері аудит оқиғаларын жазады.

## 6. Қолмен кері қайтару және оқиғаға жауап беру

Алдыңғы нұсқаға қайтару:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Растау үшін күй шығысын пайдаланыңыз:

- `current_version` қайтарылды
- `audit_event_count` ұлғайтылған
- `active_rollout` тазартылды
- `last_rollout.stage` автоматты кері қайтаруға арналған `RolledBack`

## 7. Операцияларды тексеру парағы

- Үлгі арқылы жасалған манифесттерді нұсқа бақылауында ұстаңыз.
- Бақылау мүмкіндігін сақтау үшін әрбір шығару қадамы үшін `governance_tx_hash` жазыңыз.
- `service_health`, `routing`, `resource_pressure` және
  `failed_admissions` шығару қақпасының кірістері ретінде.
- Тікелей толық кесілгеннен гөрі канарей пайыздарын және нақты жарнаманы пайдаланыңыз
  пайдаланушыға бағытталған қызметтер үшін жаңартулар.
- Сеанс/аутентификация және қолтаңбаны тексеру әрекетін растаңыз
  Өндіріске дейін `webapp/api/server.mjs`.