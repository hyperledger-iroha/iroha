---
lang: ru
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

# Ранбук превью-сессий Connect (IOS7 / JS4)

Этот ранбук описывает сквозную процедуру подготовки, проверки и завершения превью-сессий Connect, необходимых по вехам **IOS7** и **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Следуйте этим шагам каждый раз, когда вы демонстрируете strawman Connect (`docs/source/connect_architecture_strawman.md`), проверяете хуки очереди/телеметрии, обещанные в дорожных планах SDK, или собираете доказательства для `status.md`.

## 1. Чеклист перед стартом

| Пункт | Детали | Ссылки |
|------|---------|------------|
| Endpoint Torii + политика Connect | Подтвердите базовый URL Torii, `chain_id` и политику Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Сохраните JSON-снимок в тикете ранбука. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Версии фикстуры и bridge | Зафиксируйте хэш фикстуры Norito и билд bridge, который будете использовать (Swift требует `NoritoBridge.xcframework`, JS требует `@iroha/iroha-js` >= версии, в которой появился `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Дашборды телеметрии | Убедитесь, что дашборды с `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` и т.д. доступны (board Grafana `Android/Swift Connect` + экспортированные снимки Prometheus). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Папки доказательств | Выберите место вроде `docs/source/status/swift_weekly_digest.md` (еженедельный дайджест) и `docs/source/sdk/swift/connect_risk_tracker.md` (трекер рисков). Сохраняйте логи, скриншоты метрик и подтверждения в `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Запуск превью-сессии

1. **Проверьте политику и квоты.** Вызовите:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Считайте прогон неуспешным, если `queue_max` или TTL отличаются от запланированной конфигурации.
2. **Сформируйте детерминированные SID/URI.** Хелпер `bootstrapConnectPreviewSession` из `@iroha/iroha-js` связывает генерацию SID/URI с регистрацией сессии в Torii; используйте его даже когда Swift ведет слой WebSocket.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - Установите `register: false` для dry-run сценариев QR/deep-link.
   - Сохраните возвращенные `sidBase64Url`, URL deeplink и blob `tokens` в папку доказательств; ревью governance ожидает эти артефакты.
3. **Распределите секреты.** Передайте deeplink URI оператору кошелька (Swift dApp sample, Android wallet или QA harness). Никогда не вставляйте сырые токены в чат; используйте зашифрованное хранилище, описанное в enablement packet.

## 3. Ведение сессии

1. **Откройте WebSocket.** Клиенты Swift обычно используют:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   См. `docs/connect_swift_integration.md` для дополнительной настройки (imports bridge, адаптеры конкуренции).
2. **Потоки согласования и подписи.** DApps вызывают `ConnectSession.requestSignature(...)`, а кошельки отвечают через `approveSession` / `reject`. Каждое одобрение должно логировать хешированный alias + permissions, чтобы соответствовать хартии governance Connect.
3. **Проверьте очередь и возобновление.** Переключайте сеть или приостанавливайте кошелек, чтобы убедиться, что ограниченная очередь и хуки replay фиксируют события. SDK JS/Android эмитят `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` при сбросе кадров; Swift должен наблюдать то же самое, когда появится каркас очереди IOS7 (`docs/source/connect_architecture_strawman.md`). После как минимум одного реконнекта выполните
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (или передайте каталог экспорта, возвращенный `ConnectSessionDiagnostics`) и приложите таблицу/JSON к тикету ранбука. CLI читает ту же пару `state.json` / `metrics.ndjson`, которую создает `ConnectQueueStateTracker`, поэтому ревьюеры governance могут отслеживать доказательства drill без специальных инструментов.

## 4. Телеметрия и наблюдаемость

- **Метрики для сбора:**
  - `connect.queue_depth{direction}` gauge (должен оставаться ниже лимита политики).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (ненулевой только во время инъекции отказов).
  - `connect.resume_latency_ms` histogram (зафиксируйте p95 после принудительного реконнекта).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-specific exports `swift.connect.session_event` и `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Дашборды:** Обновите закладки доски Connect с аннотациями. Приложите скриншоты (или JSON exports) в папку доказательств вместе с OTLP/Prometheus снимками, полученными через CLI экспортера телеметрии.
- **Оповещения:** Если срабатывают пороги Sev 1/2 (см. `docs/source/android_support_playbook.md` раздел 5), позовите SDK Program Lead и зафиксируйте ID инцидента PagerDuty в тикете ранбука перед продолжением.

## 5. Очистка и откат

1. **Удалите подготовленные сессии.** Всегда удаляйте превью-сессии, чтобы алерты глубины очереди оставались значимыми:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Для прогонов только Swift вызовите тот же endpoint через helper Rust/CLI.
2. **Очистите журналы.** Удалите любые сохраненные журналы очереди (`ApplicationSupport/ConnectQueue/<sid>.to`, хранилища IndexedDB и т.д.), чтобы следующий прогон стартовал чисто. Зафиксируйте хэш файла перед удалением, если нужно отлаживать проблему replay.
3. **Зафиксируйте заметки по инциденту.** Сводка прогона в:
   - `docs/source/status/swift_weekly_digest.md` (блок deltas),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (очистите или понизьте CR-2 после готовности телеметрии),
   - changelog SDK JS или рецепт, если было подтверждено новое поведение.
4. **Эскалация сбоев:**
   - Переполнение очереди без инъекций отказов => заведите баг на SDK, чья политика расходится с Torii.
   - Ошибки возобновления => приложите снимки `connect.queue_depth` + `connect.resume_latency_ms` к отчету об инциденте.
   - Несоответствия governance (повторное использование токенов, превышение TTL) => эскалируйте SDK Program Lead и отметьте это в `roadmap.md` при следующем обновлении.

## 6. Чеклист доказательств

| Артефакт | Место |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| Подтверждение очистки (Torii delete, journal wipe) | `.../cleanup.log` |

Заполнение этого чеклиста закрывает критерий выхода "docs/runbooks updated" для IOS7/JS4 и дает ревьюерам governance детерминированный след для каждой превью-сессии Connect.
