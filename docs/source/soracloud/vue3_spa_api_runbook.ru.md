<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
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

Этот модуль Runbook охватывает ориентированное на производство развертывание и операции для:

— статический сайт Vue3 (`--template site`); и
- сервис Vue3 SPA + API (`--template webapp`),

использование API-интерфейсов плоскости управления Soracloud на Iroha 3 с предположениями SCR/IVM (нет
Зависимость времени выполнения WASM и отсутствие зависимости Docker).

## 1. Создание шаблонных проектов

Статический каркас сайта:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Структура SPA + API:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Каждый выходной каталог включает в себя:

- `container_manifest.json`
- `service_manifest.json`
- исходные файлы шаблонов под `site/` или `webapp/`.

## 2. Создание артефактов приложения

Статический сайт:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA-интерфейс + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Упакуйте и опубликуйте ресурсы внешнего интерфейса

Для статического хостинга через SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

Для интерфейса SPA:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Развертывание в рабочей плоскости управления Soracloud

Разверните службу статического сайта:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Развертывание службы SPA + API:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Проверьте привязку маршрута и состояние развертывания:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Ожидаемые проверки плоскости управления:

- Комплект `control_plane.services[].latest_revision.route_host`
- Комплект `control_plane.services[].latest_revision.route_path_prefix` (`/` или `/api`)
- `control_plane.services[].active_rollout` присутствует сразу после обновления

## 5. Обновление с контролируемым развертыванием

1. Добавьте `service_version` в манифест службы.
2. Запустите обновление:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Содействие внедрению после проверок работоспособности:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Если здоровье подвело, сообщите о нездоровье:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Когда неработоспособные отчеты достигают порога политики, Soracloud автоматически выполняет
вернуться к базовой версии и записать события аудита отката.

## 6. Ручной откат и реагирование на инциденты

Откат к предыдущей версии:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Используйте вывод состояния, чтобы подтвердить:

- `current_version` восстановлен
- `audit_event_count` увеличен
- `active_rollout` очищен
- `last_rollout.stage` — `RolledBack` для автоматического отката

## 7. Контрольный список операций

- Храните манифесты, созданные на основе шаблонов, под контролем версий.
- Записывайте `governance_tx_hash` для каждого шага развертывания, чтобы сохранить отслеживаемость.
- Обработайте `service_health`, `routing`, `resource_pressure` и
  `failed_admissions` в качестве входов раскатных ворот.
- Используйте канареечные проценты и явное продвижение, а не прямое полное сокращение.
  обновления для сервисов, ориентированных на пользователя.
- Проверка поведения сеанса/аутентификации и проверки подписи в
  `webapp/api/server.mjs` до производства.