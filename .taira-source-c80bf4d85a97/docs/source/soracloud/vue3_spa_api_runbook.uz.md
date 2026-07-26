<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
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

Ushbu ish kitobi ishlab chiqarishga yo'naltirilgan joylashtirish va operatsiyalarni o'z ichiga oladi:

- Vue3 statik sayt (`--template site`); va
- Vue3 SPA + API xizmati (`--template webapp`),

SCR/IVM taxminlari bilan Iroha 3 da Soracloud boshqaruv tekisligi API laridan foydalanish (yo'q
WASM ish vaqtiga bog'liqlik va Docker bog'liqligi yo'q).

## 1. Shablon loyihalarini yarating

Statik sayt iskala:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API iskala:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Har bir chiqish katalogiga quyidagilar kiradi:

- `container_manifest.json`
- `service_manifest.json`
- `site/` yoki `webapp/` ostidagi shablon manba fayllari

## 2. Ilova artefaktlarini yarating

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

## 3. Frontend aktivlarini paketlash va nashr qilish

SoraFS orqali statik xosting uchun:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA frontend uchun:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Jonli Soracloud boshqaruv samolyotiga joylashtiring

Statik sayt xizmatini o'rnatish:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API xizmatini o'rnatish:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Marshrutni ulash va ishga tushirish holatini tasdiqlang:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Kutilayotgan nazorat tekisligi tekshiruvlari:

- `control_plane.services[].latest_revision.route_host` to'plami
- `control_plane.services[].latest_revision.route_path_prefix` to'plami (`/` yoki `/api`)
- `control_plane.services[].active_rollout` yangilanishdan so'ng darhol mavjud

## 5. Sog'liqni saqlash bilan himoyalangan dastur bilan yangilang

1. Xizmat manifestida `service_version` bump.
2. Yangilashni ishga tushiring:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Sog'liqni saqlash tekshiruvidan so'ng tarqatishni targ'ib qiling:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Agar sog'lig'ingiz yomonlashsa, nosog'lom haqida xabar bering:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Nosog'lom hisobotlar siyosat chegarasiga yetganda, Soracloud avtomatik ravishda ishga tushadi
bazaviy qayta ko'rib chiqishga qaytish va orqaga qaytish audit hodisalarini qayd qiladi.

## 6. Qo'lda orqaga qaytarish va hodisaga javob berish

Oldingi versiyaga qaytish:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Tasdiqlash uchun holat chiqishidan foydalaning:

- `current_version` qaytarildi
- `audit_event_count` oshirildi
- `active_rollout` tozalandi
- `last_rollout.stage` - avtomatik orqaga qaytarish uchun `RolledBack`

## 7. Operatsiyalarni tekshirish ro'yxati

- Shablonda yaratilgan manifestlarni versiya nazorati ostida saqlang.
- `governance_tx_hash` ni kuzatuvni saqlab qolish uchun har bir ishga tushirish bosqichi uchun yozib oling.
- `service_health`, `routing`, `resource_pressure` va
  `failed_admissions` kirish eshiklari sifatida.
- To'g'ridan-to'g'ri to'liq kesishdan ko'ra, kanareyka foizlari va aniq reklamadan foydalaning
  foydalanuvchiga qaratilgan xizmatlar uchun yangilanishlar.
- Seans/auth va imzoni tekshirish xatti-harakatlarini tasdiqlang
  Ishlab chiqarishdan oldin `webapp/api/server.mjs`.