---
lang: ru
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

# План репетиции хаоса и отказов Connect (IOS3 / IOS7)

Этот плейбук описывает повторяемые хаос-дриллы, которые закрывают действие дорожной карты _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). Используйте его вместе с ранбуком превью Connect (`docs/runbooks/connect_session_preview_runbook.md`) при кросс-SDK демо.

## Цели и критерии успеха
- Прогонять общую политику retry/back-off Connect, лимиты офлайн-очередей и экспорт телеметрии под контролируемыми сбоями без изменения продакшн-кода.
- Собирать детерминированные артефакты (вывод `iroha connect queue inspect`, снимки метрик `connect.*`, логи SDK Swift/Android/JS), чтобы governance могла аудировать каждый drill.
- Подтвердить, что кошельки и dApp уважают изменения конфигурации (drift манифеста, ротация соли, сбои attestation), показывая каноническую категорию `ConnectError` и редактируемо-безопасные события телеметрии.

## Предварительные условия
1. **Подготовка окружения**
   - Запустите demo Torii стек: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Запустите хотя бы один sample SDK (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Инструментация**
   - Включите диагностику SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` в Swift; аналоги `ConnectQueueJournal` + `ConnectQueueJournalTests`
     в Android/JS).
   - Убедитесь, что CLI `iroha connect queue inspect --sid <sid> --metrics` находит
     путь очереди, который создает SDK (`~/.iroha/connect/<sid>/state.json` и
     `metrics.ndjson`).
   - Подключите экспортеры телеметрии, чтобы следующие серии были видны в
     Grafana и через `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Папки доказательств** - создайте `artifacts/connect-chaos/<date>/` и сохраните:
   - сырые логи (`*.log`), снимки метрик (`*.json`), экспорты дашбордов
     (`*.png`), вывод CLI и PagerDuty IDs.

## Матрица сценариев

| ID | Отказ | Шаги инъекции | Ожидаемые сигналы | Доказательства |
|----|-------|---------------|-------------------|----------------|
| C1 | Отключение WebSocket и реконнект | Оберните `/v2/connect/ws` прокси (например, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) или временно заблокируйте сервис (`kubectl scale deploy/torii --replicas=0` на <=60 s). Заставьте кошелек продолжать отправлять фреймы, чтобы офлайн-очереди заполнились. | `connect.reconnects_total` растет, `connect.resume_latency_ms` дает пик, но остается <1 s p95, очереди переходят в `state=Draining` через `ConnectQueueStateTracker`. SDKs эмитят `ConnectError.Transport.reconnecting` один раз и затем продолжают. | - Вывод `iroha connect queue inspect --sid <sid>` с ненулевым `resume_attempts_total`.<br>- Аннотация на дашборде для окна outage.<br>- Лог-выдержка с reconnect + drain. |
| C2 | Переполнение офлайн-очереди / истечение TTL | Измените sample, чтобы снизить лимиты очереди (Swift: инстанцируйте `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` внутри `ConnectSessionDiagnostics`; Android/JS используют эквивалентные конструкторы). Приостановите кошелек на >=2x `retentionInterval`, пока dApp продолжает ставить запросы в очередь. | `connect.queue_dropped_total{reason="overflow"}` и `{reason="ttl"}` растут, `connect.queue_depth` выходит на новый лимит, SDKs показывают `ConnectError.QueueOverflow(limit: 4)` (или `.QueueExpired`). `iroha connect queue inspect` показывает `state=Overflow` с `warn/drop` отметками на 100%. | - Скриншот счетчиков метрик.<br>- JSON CLI с overflow.<br>- Лог Swift/Android со строкой `ConnectError`. |
| C3 | Drift манифеста / отказ admission | Подмените манифест Connect, который отдается кошелькам (например, измените sample в `docs/connect_swift_ios.md` или запустите Torii с `--connect-manifest-path`, указывающим на копию, где `chain_id` или `permissions` отличаются). Пусть dApp запросит одобрение и убедитесь, что кошелек отклоняет по политике. | Torii возвращает `HTTP 409` для `/v2/connect/session` с `manifest_mismatch`, SDKs эмитят `ConnectError.Authorization.manifestMismatch(manifestVersion)`, телеметрия повышает `connect.manifest_mismatch_total`, очереди остаются пустыми (`state=Idle`). | - Лог Torii с детектом mismatch.<br>- Скриншот SDK с ошибкой.<br>- Снимок метрик, подтверждающий отсутствие queued frames. |
| C4 | Ротация ключа / повышение версии соли | Поверните соль или AEAD ключ Connect в середине сессии. В dev-стеке перезапустите Torii с `CONNECT_SALT_VERSION=$((old+1))` (как в Android тесте соли в `docs/source/sdk/android/telemetry_schema_diff.md`). Держите кошелек офлайн до завершения ротации, затем возобновите. | Первая попытка resume падает с `ConnectError.Authorization.invalidSalt`, очереди очищаются (dApp сбрасывает кешированные фреймы с причиной `salt_version_mismatch`), телеметрия эмитит `android.telemetry.redaction.salt_version` (Android) и `swift.connect.session_event{event="salt_rotation"}`. Вторая сессия после обновления SID проходит успешно. | - Аннотация на дашборде с эпохой соли до/после.<br>- Логи с invalid-salt и последующим успехом.<br>- Вывод `iroha connect queue inspect` с `state=Stalled`, затем `state=Active`. |
| C5 | Сбой attestation / StrongBox | На Android кошельках настройте `ConnectApproval` так, чтобы включать `attachments[]` + StrongBox attestation. Используйте harness attestation (`scripts/android_keystore_attestation.sh` с `--inject-failure strongbox-simulated`) или подмените JSON attestation до передачи в dApp. | dApp отклоняет approval с `ConnectError.Authorization.invalidAttestation`, Torii логирует причину сбоя, экспортеры увеличивают `connect.attestation_failed_total`, а очередь удаляет плохую запись. dApps Swift/JS логируют ошибку, сохраняя сессию активной. | - Лог harness с ID инъекции отказа.<br>- Лог ошибки SDK + снимок счетчика телеметрии.<br>- Доказательство удаления плохого фрейма (`recordsRemoved > 0`). |

## Детали сценариев

### C1 - Отключение WebSocket и реконнект
1. Оберните Torii прокси (toxiproxy, Envoy или `kubectl port-forward`), чтобы
   можно было переключать доступность без остановки всего узла.
2. Запустите отключение на 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Наблюдайте дашборды телеметрии и `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. Снимите состояние очереди сразу после отключения:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Успех = один реконнект, ограниченный рост очереди и автоматический drain
   после восстановления прокси.

### C2 - Переполнение офлайн-очереди / истечение TTL
1. Уменьшите пороги очереди в локальных сборках:
   - Swift: обновите инициализатор `ConnectQueueJournal` в sample
     (например, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     чтобы передать `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: передайте эквивалентную конфигурацию при создании `ConnectQueueJournal`.
2. Приостановите кошелек (background симулятора или авиарежим) на >=60 s,
   пока dApp вызывает `ConnectClient.requestSignature(...)`.
3. Используйте `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) или JS helper
   для экспорта bundle доказательств (`state.json`, `journal/*.to`, `metrics.ndjson`).
4. Успех = счетчики overflow растут, SDK показывает `ConnectError.QueueOverflow`
   один раз, и очередь восстанавливается после возврата кошелька.

### C3 - Drift манифеста / отказ admission
1. Сделайте копию admission manifest, например:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Запустите Torii с `--connect-manifest-path /tmp/manifest_drift.json` (или
   обновите docker compose/k8s config для drill).
3. Попробуйте начать сессию с кошелька; ожидайте HTTP 409.
4. Соберите логи Torii + SDK и `connect.manifest_mismatch_total` с дашборда телеметрии.
5. Успех = отказ без роста очереди, и кошелек показывает ошибку общей таксономии
   (`ConnectError.Authorization.manifestMismatch`).

### C4 - Ротация ключа / смена соли
1. Запишите текущую версию соли из телеметрии:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Перезапустите Torii с новой солью (`CONNECT_SALT_VERSION=$((OLD+1))` или обновите
   config map). Держите кошелек офлайн до завершения перезапуска.
3. Возобновите кошелек; первая попытка должна упасть с invalid-salt
   и `connect.queue_dropped_total{reason="salt_version_mismatch"}` растет.
4. Принудительно удалите кешированные фреймы, удалив директорию сессии
   (`rm -rf ~/.iroha/connect/<sid>` или очистка кеша по платформе), затем
   перезапустите сессию с новыми токенами.
5. Успех = телеметрия показывает смену соли, invalid-resume логируется
   один раз, и следующая сессия проходит без ручного вмешательства.

### C5 - Сбой attestation / StrongBox
1. Сгенерируйте attestation bundle через `scripts/android_keystore_attestation.sh`
   (укажите `--inject-failure strongbox-simulated`, чтобы испортить подпись).
2. Пусть кошелек прикрепит этот bundle через API `ConnectApproval`; dApp
   должна проверить и отклонить payload.
3. Проверьте телеметрию (`connect.attestation_failed_total`, инцидентные метрики
   Swift/Android) и убедитесь, что очередь удалила плохую запись.
4. Успех = отказ локализован на плохом approval, очереди остаются здоровыми,
   и log attestation сохранен вместе с доказательствами drill.

## Чеклист доказательств
- exports `artifacts/connect-chaos/<date>/c*_metrics.json` из
  `scripts/swift_status_export.py telemetry`.
- вывод CLI (`c*_queue.txt`) из `iroha connect queue inspect`.
- логи SDK + Torii с временными метками и хешами SID.
- скриншоты дашбордов с аннотациями для каждого сценария.
- PagerDuty / incident IDs, если срабатывали Sev 1/2.

Выполнение полной матрицы раз в квартал закрывает гейт дорожной карты и
показывает, что реализации Connect на Swift/Android/JS детерминированно
реагируют на самые рискованные режимы отказа.
