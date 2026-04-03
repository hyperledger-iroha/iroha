<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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

Bu runbook aşağıdakılar üçün istehsal yönümlü yerləşdirmə və əməliyyatları əhatə edir:

- Vue3 statik saytı (`--template site`); və
- Vue3 SPA + API xidməti (`--template webapp`),

SCR/IVM fərziyyələri ilə Iroha 3-də Soracloud idarəetmə təyyarəsi API-lərindən istifadə (yox
WASM iş vaxtından asılılıq və Docker asılılığı yoxdur).

## 1. Şablon layihələr yaradın

Statik sayt iskele:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API iskele:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Hər bir çıxış kataloquna aşağıdakılar daxildir:

- `container_manifest.json`
- `service_manifest.json`
- `site/` və ya `webapp/` altında şablon mənbə faylları

## 2. Tətbiq artefaktları yaradın

Statik sayt:

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

## 3. Frontend aktivlərini paketləyin və dərc edin

SoraFS vasitəsilə statik hostinq üçün:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA cəbhəsi üçün:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Canlı Soracloud idarəetmə təyyarəsi üçün yerləşdirin

Statik sayt xidmətini yerləşdirin:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API xidmətini yerləşdirin:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Marşrutun bağlanması və buraxılış vəziyyətini təsdiqləyin:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Gözlənilən idarəetmə təyyarəsi yoxlamaları:

- `control_plane.services[].latest_revision.route_host` dəsti
- `control_plane.services[].latest_revision.route_path_prefix` dəsti (`/` və ya `/api`)
- `control_plane.services[].active_rollout` təkmilləşdirmədən dərhal sonra təqdim olunur

## 5. Sağlamlıq nəzarəti ilə təkmilləşdirin

1. Xidmət manifestində `service_version` işarəsini vurun.
2. Təkmilləşdirməni işə salın:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Sağlamlıq yoxlamalarından sonra tətbiqi təşviq edin:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Səhhətiniz pis olarsa, sağlam olduğunuzu bildirin:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Qeyri-sağlam hesabatlar siyasət həddinə çatdıqda, Soracloud avtomatik olaraq yuvarlanır
ilkin təftişə qayıdın və audit hadisələrini geri qaytarın.

## 6. Əllə geriyə qaytarma və insidentlərə cavab

Əvvəlki versiyaya geri qayıt:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Təsdiq etmək üçün status çıxışından istifadə edin:

- `current_version` geri qaytarıldı
- `audit_event_count` artırıldı
- `active_rollout` təmizləndi
- `last_rollout.stage` avtomatik geri qaytarma üçün `RolledBack`-dir

## 7. Əməliyyatlara nəzarət siyahısı

- Şablonla yaradılan manifestləri versiya nəzarəti altında saxlayın.
- İzləmə qabiliyyətini qorumaq üçün hər buraxılış addımı üçün `governance_tx_hash` yazın.
- `service_health`, `routing`, `resource_pressure` və
  `failed_admissions` buraxılış qapısı girişləri kimi.
- Birbaşa tam kəsmə əvəzinə kanareyka faizlərindən və açıq təşviqdən istifadə edin
  istifadəçi ilə bağlı xidmətlər üçün təkmilləşdirmələr.
- Sessiya/auth və imza doğrulama davranışını təsdiqləyin
  İstehsaldan əvvəl `webapp/api/server.mjs`.