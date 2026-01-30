---
lang: ja
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ru
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Спецификация раннера модерации ИИ
summary: Детерминированный дизайн комитета модерации для поставки Министерства информации (MINFO-1).
---

# Спецификация раннера модерации ИИ

Эта спецификация закрывает документационную часть **MINFO-1 — Establish AI moderation baseline**. Она определяет детерминированный контракт исполнения сервиса модерации Министерства информации, чтобы каждый gateway запускал идентичные пайплайны перед апелляциями и потоками прозрачности (SFM-4/SFM-4b). Всё поведение, описанное здесь, является нормативным, если не отмечено как информационное.

## 1. Цели и охват
- Обеспечить воспроизводимый комитет модерации, который оценивает контент шлюза (объекты, манифесты, метаданные, аудио) с использованием разнородных моделей.
- Гарантировать детерминированное исполнение между операторами: фиксированный opset, токенизация с сидом, ограниченная точность и версионированные артефакты.
- Выпускать артефакты, готовые к аудиту: манифесты, scorecards, доказательства калибровки и дайджесты прозрачности, пригодные для публикации в DAG управления.
- Экспортировать телеметрию, чтобы SRE могли обнаруживать дрейф, ложные срабатывания и простои без сбора сырых пользовательских данных.

## 2. Контракт детерминированного исполнения
- **Runtime:** ONNX Runtime 1.19.x (CPU backend), собранный с отключённым AVX2 и флагом `--enable-extended-minimal-build`, чтобы зафиксировать набор опкодов. Рантаймы CUDA/Metal прямо запрещены в продакшене.
- **Opset:** `opset=17`. Модели, нацеленные на более новые opset, должны быть понижены и валидированы до допуска.
- **Производная сидов:** каждая оценка выводит seed RNG из `BLAKE3(content_digest || manifest_id || run_nonce)`, где `run_nonce` берётся из манифеста, утверждённого управлением. Сиды питают все стохастические компоненты (beam search, переключатели dropout), чтобы результаты были воспроизводимы бит-в-бит.
- **Параллелизм:** один worker на модель. Конкурентность координирует оркестратор раннера, чтобы избежать гонок на разделяемом состоянии. Библиотеки BLAS работают в однопоточном режиме.
- **Численная часть:** накопление FP16 запрещено. Используйте FP32 промежуточные значения и ограничивайте выходы до четырёх знаков после запятой перед агрегацией.

## 3. Состав комитета
Базовый комитет включает три семейства моделей. Управление может добавлять модели, но минимальный кворум должен сохраняться.

| Семейство | Базовая модель | Назначение |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14 (safety fine-tuned) | Выявляет визуальную контрабанду, насилие, индикаторы CSAM. |
| Multimodal | LLaVA-1.6 34B Safety | Захватывает взаимодействия текста и изображений, контекстные сигналы, харассмент. |
| Perceptual | ансамбль pHash + aHash + NeuralHash-lite | Быстрое выявление почти-двойников и recall известного вредоносного контента. |

Каждая запись модели задаёт:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 OCI-образа)
- `weights_digest` (BLAKE3-256 ONNX или объединённого safetensors blob)
- `opset` (должен быть `17`)
- `weight` (вес комитета, по умолчанию `1.0`)
- `critical_labels` (набор меток, которые сразу вызывают `Escalate`)
- `max_eval_ms` (ограничитель для детерминированных watchdog)

## 4. Манифесты и результаты Norito

### 4.1 Манифест комитета
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 Результат оценки
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

Раннер ДОЛЖЕН выпускать детерминированный `AiModerationDigestV1` (BLAKE3 по сериализованному результату) для журналов прозрачности и добавлять результаты в ledger модерации, когда вердикт не `pass`.

### 4.3 Манифест adversarial corpus

Операторы gateway теперь ingest’ят сопутствующий манифест, который перечисляет “семейства” perceptual hash/embedding, полученные из прогонов калибровки:

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

Схема находится в `crates/iroha_data_model/src/sorafs/moderation.rs` и валидируется через `AdversarialCorpusManifestV1::validate()`. Манифест позволяет загрузчику denylist заполнять `perceptual_family`, которые блокируют целые кластеры почти-двойников вместо отдельных байтов. Исполняемый фикстур (`docs/examples/ai_moderation_perceptual_registry_202602.json`) демонстрирует ожидаемый layout и напрямую питает примерный gateway denylist.

## 5. Исполнительный пайплайн
1. Загрузить `AiModerationManifestV1` из governance DAG. Отклонить, если `runner_hash` или `runtime_version` не совпадают с развёрнутым бинарём.
2. Получить артефакты моделей по OCI digest, проверяя digests перед загрузкой.
3. Сформировать батчи оценки по типу контента; порядок должен быть `(content_digest, manifest_id)` для детерминированной агрегации.
4. Запустить каждую модель с производной сид‑строкой. Для perceptual hash объединять ансамбль по большинству голосов → score в `[0,1]`.
5. Агрегировать оценки в `combined_score` по взвешенному усечённому отношению:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Сформировать `ModerationVerdictV1`:
   - `escalate`, если срабатывает любая `critical_labels` или `combined ≥ thresholds.escalate`.
   - `quarantine`, если выше `thresholds.quarantine`, но ниже `escalate`.
   - `pass` иначе.
7. Сохранить `AiModerationResultV1` и поставить downstream‑процессы в очередь:
   - сервис карантина (если вердикт escalates/quarantines)
   - writer журнала прозрачности (`ModerationLedgerV1`)
   - экспортёр телеметрии

## 6. Калибровка и оценка
- **Datasets:** базовая калибровка использует смешанный корпус, утверждённый policy‑командой. Ссылка хранится в `calibration_dataset`.
- **Метрики:** вычислять Brier score, Expected Calibration Error (ECE) и AUROC по каждой модели и по комбинированному вердикту. Ежемесячная перекалибровка ДОЛЖНА удерживать `Brier ≤ 0.18` и `ECE ≤ 0.05`. Результаты сохраняются в дереве отчётов SoraFS (например, [февральская калибровка 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **График:** ежемесячная перекалибровка (первый понедельник). Допускается экстренная перекалибровка при срабатывании алертов дрейфа.
- **Процесс:** прогнать детерминированный pipeline оценки на калибровочном наборе, пересчитать `thresholds`, обновить манифест и подготовить изменения к голосованию управления.

## 7. Packaging и деплой
- Сборка OCI образов через `docker buildx bake -f docker/ai_moderation.hcl`.
- Образы включают:
  - зафиксированное окружение Python (`poetry.lock`) или Rust бинарь `Cargo.lock`;
  - каталог `models/` с хэшированными ONNX‑весами;
  - входную точку `run_moderation.py` (или эквивалент на Rust), которая отдаёт HTTP/gRPC API.
- Публиковать артефакты в `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- Бинарь раннера поставляется как часть crate `sorafs_ai_runner`. Пайплайн сборки вшивает хэш манифеста в бинарь (доступен через `/v1/info`).

## 8. Телеметрия и наблюдаемость
- Метрики Prometheus:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Логи: JSON‑строки с `request_id`, `manifest_id`, `verdict` и digest сохранённого результата. Сырые оценки редактируются до двух знаков после запятой в логах.
- Дашборды хранятся в `dashboards/grafana/ministry_moderation_overview.json` (публикуются вместе с первым отчётом о калибровке).
- Пороги алертов:
  - отсутствие инжеста (`moderation_requests_total` не растёт 10 минут);
  - обнаружение дрейфа (средний delta score модели >20% относительно 7‑дневной скользящей средней);
  - backlog ложных срабатываний (очередь карантина > 50 элементов более 30 минут).

## 9. Управление и контроль изменений
- Манифесты требуют двойной подписи: член совета Министерства + лидер SRE модерации. Подписи записываются в `AiModerationManifestV1.governance_signature`.
- Изменения проходят через `ModerationManifestChangeProposalV1` в Torii. Хэши заносятся в governance DAG; деплой блокируется до принятия предложения.
- Бинарь раннера вшивает `runner_hash`; CI отклоняет деплой при расхождении хэшей.
- Прозрачность: еженедельный `ModerationScorecardV1`, суммирующий объём, микс вердиктов и исходы апелляций. Публикуется на портале парламента Sora.

## 10. Безопасность и приватность
- Digest’ы контента используют BLAKE3. Сырые payload’ы никогда не сохраняются вне карантина.
- Доступ к карантину требует Just‑In‑Time одобрений; все доступы логируются.
- Раннер изолирует недоверенный контент, ограничивая память 512 MiB и wall‑clock 120 s.
- Дифференциальная приватность здесь НЕ применяется; gateways полагаются на карантин + аудит‑процессы. Политики редактирования следуют плану соответствия gateway (`docs/source/sorafs_gateway_compliance_plan.md`; копия в портале ожидается).

## 11. Публикация калибровки (2026-02)
- **Манифест:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  фиксирует governance‑подписанный `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), ссылку на датасет
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, хэш раннера
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` и
  пороги калибровки 2026‑02 (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  плюс читаемый отчёт
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  фиксируют Brier, ECE, AUROC и микс вердиктов по каждой модели. Комбинированные метрики достигли целей (`Brier = 0.126`, `ECE = 0.034`).
- **Дашборды и алерты:** `dashboards/grafana/ministry_moderation_overview.json`
  и `dashboards/alerts/ministry_moderation_rules.yml` (с регрессионными тестами в
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) обеспечивают мониторинг инжеста/латентности/дрейфа, требуемый для запуска.

## 12. Схема воспроизводимости и валидатор (MINFO-1b)
- Канонические типы Norito теперь находятся рядом с остальной схемой SoraFS в
  `crates/iroha_data_model/src/sorafs/moderation.rs`. Структуры
  `ModerationReproManifestV1`/`ModerationReproBodyV1` фиксируют UUID манифеста, хэш раннера, digests моделей, набор порогов и seed‑материал.
  `ModerationReproManifestV1::validate` обеспечивает версию схемы
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), гарантирует, что каждый манифест содержит как минимум одну модель и одного подписанта, и проверяет каждую `SignatureOf<ModerationReproBodyV1>` перед возвратом машиночитаемого резюме.
- Операторы могут вызвать общий валидатор через
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (реализовано в `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). CLI
  принимает как JSON‑артефакты, опубликованные в
  `docs/examples/ai_moderation_calibration_manifest_202602.json`, так и сырой Norito‑формат и выводит количество моделей/подписей и timestamp манифеста после успешной проверки.
- Gateways и автоматизация используют тот же helper, чтобы детерминированно отклонять манифесты воспроизводимости при дрейфе схемы, отсутствии digests или ошибках подписей.
- Бандлы adversarial corpus следуют тому же паттерну:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  разбирает `AdversarialCorpusManifestV1`, проверяет версию схемы и отклоняет манифесты, которые не содержат семьи, варианты или метаданные отпечатков. Успешные запуски выводят timestamp выпуска, метку когорты и количество семейств/вариантов, чтобы операторы могли зафиксировать доказательства перед обновлением записей denylist gateway, описанных в Разделе 4.3.

## 13. Открытые follow-ups
- Ежемесячные окна перекалибровки после 2026-03-02 продолжают следовать процедуре из Раздела 6; публикуйте `ai-moderation-calibration-<YYYYMM>.md` вместе с обновлёнными бандлами manifest/scorecard в дереве отчётов SoraFS.
- MINFO-1b и MINFO-1c (валидаторы манифестов воспроизводимости и реестр adversarial corpus) продолжают отслеживаться отдельно в roadmap.
