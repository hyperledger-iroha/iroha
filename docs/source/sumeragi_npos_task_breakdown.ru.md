---
lang: ru
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

## Разбивка задач Sumeragi + NPoS

Эта заметка расширяет дорожную карту Фазы A до небольших инженерных задач, чтобы мы могли поэтапно закрывать оставшуюся работу по Sumeragi/NPoS. Обозначения статуса следуют конвенции: `✅` выполнено, `⚙️` в работе, `⬜` не начато и `🧪` нужны тесты.

### A2 - Принятие сообщений на уровне wire
- ✅ Вынести типы Norito `Proposal`/`Vote`/`Qc` в `BlockMessage` и прогнать round-trip encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ Закрыть старые фреймы `BlockSigned/BlockCommitted`; переключатель миграции был выставлен в `false` до вывода из эксплуатации.
- ✅ Удалить миграционный knob, переключавший старые сообщения блоков; режим Vote/commit certificate теперь единственный wire-путь.
- ✅ Обновить роутеры Torii, команды CLI и потребителей телеметрии, чтобы предпочитать JSON-снимки `/v2/sumeragi/*` старым фреймам блоков.
- ✅ Интеграционное покрытие упражняет эндпоинты `/v2/sumeragi/*` исключительно через пайплайн Vote/commit certificate (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ Удалить старые фреймы после достижения функционального паритета и тестов совместимости.

### План удаления фреймов
1. ✅ Мультиузловые soak-тесты шли 72 h на телеметрийном и CI harness; снимки Torii показали стабильную пропускную способность proposer и формирование commit certificate без регрессий.
2. ✅ Интеграционные тесты теперь работают только через путь Vote/commit certificate (`sumeragi_vote_qc_commit.rs`), гарантируя достижение консенсуса смешанными пирами без старых фреймов.
3. ✅ Операторская документация и CLI help больше не упоминают прежний wire-путь; руководство по troubleshooting теперь указывает на телеметрию Vote/commit certificate.
4. ✅ Удалены прежние варианты сообщений, счетчики телеметрии и кеши ожидающих commit; матрица совместимости теперь отражает только поверхность Vote/commit certificate.

### A3 - Усиление движка и pacemaker
- ✅ Инварианты Lock/Highestcommit certificate закреплены в `handle_message` (см. `block_created_header_sanity`).
- ✅ Отслеживание доступности данных валидирует хеш payload RBC при записи доставки (`Actor::ensure_block_matches_rbc_payload`), чтобы несовпадающие сессии не считались доставленными.
- ✅ Требование Precommitcommit certificate (`require_precommit_qc`) включено в конфиги по умолчанию и добавлены негативные тесты (по умолчанию теперь `true`; тесты покрывают пути с gate и opt-out).
- ✅ Вместо эвристик redundant-send на уровне view внедрены контроллеры pacemaker на базе EMA (`aggregator_retry_deadline` теперь зависит от live EMA и задает дедлайны redundant send).
- ✅ Сборка предложений блокируется по backpressure очереди (`BackpressureGate` теперь останавливает pacemaker при насыщении очереди и пишет deferrals в status/telemetry).
- ✅ Голоса availability отправляются после валидации предложения, когда требуется DA (без ожидания локального RBC `DELIVER`), а evidence availability отслеживается через `availability evidence` как доказательство безопасности, пока commit продолжается без ожидания. Это избегает циклических ожиданий между доставкой payload и голосованием.
- ✅ Покрытие restart/liveness теперь включает восстановление RBC после cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) и возобновление pacemaker после downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- ✅ Добавлены детерминированные regression-тесты restart/view-change, покрывающие сходимость lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - Пайплайн коллекторов и случайности
- ✅ Детерминированные хелперы ротации коллекторов живут в `collectors.rs`.
- ✅ GA-A4.1 - Выбор коллекторов на базе PRF теперь записывает детерминированные seeds и height/view в `/status` и телеметрию; VRF refresh hooks распространяют контекст после commits и reveals. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md` (закрыт).
- ✅ GA-A4.2 - Показать телеметрию участия в reveal + команды CLI для инспекции и обновить Norito manifests. Owners: `@telemetry-ops`, `@torii-sdk`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`.
- ✅ GA-A4.3 - Зафиксировать восстановление после late-reveal и тесты эпох без участия в `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), проверяя телеметрию очистки штрафов. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - Совместная переконфигурация и evidence
- ✅ Скаффолдинг evidence, сохранение в WSV и Norito roundtrip теперь покрывают double-vote, invalid proposal, invalid commit certificate и варианты double exec с детерминированным дедупом и pruning горизонта (`sumeragi::evidence`).
- ✅ GA-A5.1 - Активирована joint-consensus конфигурация (старый набор коммитит, новый активируется на следующем блоке) с целевым интеграционным покрытием.
- ✅ GA-A5.2 - Обновлены governance docs и CLI потоки для slashing/jailing, вместе с mdBook синхронизацией для фиксации defaults и формулировок evidence horizon.
- ✅ GA-A5.3 - Негативные evidence-тесты (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) плюс fuzz fixtures добавлены и запускаются nightly, чтобы защищать валидацию Norito roundtrip.

### A6 - Инструменты, документация, валидация
- ✅ Телеметрия/отчетность RBC внедрены; DA отчет генерирует реальные метрики (включая счетчики eviction).
- ✅ GA-A6.1 - Happy-path тест NPoS с VRF и 4 peers теперь запускается в CI с порогами pacemaker/RBC через `integration_tests/tests/sumeragi_npos_happy_path.rs`. Owners: `@qa-consensus`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`.
- ✅ GA-A6.2 - Зафиксировать baseline производительности NPoS (блоки по 1 s, k=3) и опубликовать в `status.md`/операторских docs с воспроизводимыми seeds harness + матрицей железа. Owners: `@performance-lab`, `@telemetry-ops`. Report: `docs/source/generated/sumeragi_baseline_report.md`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`. Живой прогон записан на Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) по команде из `scripts/run_sumeragi_baseline.py`.
- ✅ GA-A6.3 - Руководства по troubleshooting для RBC/pacemaker/backpressure инструментации опубликованы (`docs/source/telemetry.md:523`); корреляция логов теперь выполняется `scripts/sumeragi_backpressure_log_scraper.py`, чтобы операторы могли получать пары pacemaker deferral/missing-availability без ручного grep. Owners: `@operator-docs`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`.
- ✅ Добавлены сценарии производительности RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), покрытие redundant fan-out (`npos_redundant_send_retries_update_metrics`) и bounded-jitter harness (`npos_pacemaker_jitter_within_band`), чтобы A6 suite проверяла deferrals soft-limit store, детерминированные drops chunks, телеметрию redundant-send и диапазоны jitter pacemaker под нагрузкой. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### Ближайшие шаги
1. ✅ Bounded-jitter harness упражняет метрики jitter pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ Усилить assertions deferral RBC в `npos_queue_backpressure_triggers_metrics`, подавая детерминированное давление на store RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ Расширить soak `/v2/sumeragi/telemetry`, чтобы покрыть длинные epochs и adversarial collectors, сравнивая snapshots с счетчиками Prometheus по нескольким heights. Покрыто `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

Ведение этого списка здесь позволяет держать `roadmap.md` сфокусированным на вехах и при этом дает команде живой чеклист. Обновляйте пункты (и отмечайте завершение) по мере выхода изменений.
