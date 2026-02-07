---
lang: ru
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Руководство по миграции производства FASTPQ

В этом модуле Runbook описывается, как проверить производственный прувер FASTPQ Stage6.
Детерминированный серверный заполнитель был удален в рамках этого плана миграции.
Он дополняет поэтапный план `docs/source/fastpq_plan.md` и предполагает, что вы уже отслеживаете
статус рабочей области в `status.md`.

## Аудитория и охват
- Операторы валидаторов развертывают производственное средство проверки в промежуточной среде или среде основной сети.
- Разработчики релизов создают двоичные файлы или контейнеры, которые будут поставляться вместе с серверной частью производства.
- Группы SRE/наблюдения подключают новые сигналы телеметрии и оповещения.

Выходит за рамки: Kotodama разработка контракта и IVM изменения ABI (см. `docs/source/nexus.md` для
модель исполнения).

## Матрица функций
| Путь | Грузовые функции для включения | Результат | Когда использовать |
| ---- | ----------------------- | ------ | ----------- |
| Доказательство продукции (по умолчанию) | _нет_ | Серверная часть FASTPQ Stage6 с планировщиком FFT/LDE и конвейером DEEP-FRI.【crates/fastpq_prover/src/backend.rs:1144】 | По умолчанию для всех производственных двоичных файлов. |
| Дополнительное ускорение графического процессора | `fastpq_prover/fastpq-gpu` | Включает ядра CUDA/Metal с автоматическим возвратом ЦП.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Хосты с поддерживаемыми ускорителями. |

## Процедура сборки
1. **Сборка только для процессора**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Серверная часть производства компилируется по умолчанию; никаких дополнительных функций не требуется.

2. **Сборка с поддержкой графического процессора (необязательно)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   Для поддержки графического процессора требуется набор инструментов SM80+ CUDA с `nvcc`, доступный во время сборки.【crates/fastpq_prover/Cargo.toml:11】

3. **Самотестирование**
   ```bash
   cargo test -p fastpq_prover
   ```
   Запускайте эту сборку один раз для каждой сборки, чтобы подтвердить путь к Stage6 перед упаковкой.

### Подготовка цепочки инструментов Metal (macOS)
1. Перед сборкой установите инструменты командной строки Metal: `xcode-select --install` (если инструменты CLI отсутствуют) и `xcodebuild -downloadComponent MetalToolchain`, чтобы получить набор инструментов графического процессора. Сценарий сборки вызывает `xcrun metal`/`xcrun metallib` напрямую и быстро завершится сбоем, если двоичные файлы отсутствуют.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Чтобы проверить конвейер перед CI, вы можете локально отразить сценарий сборки:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Когда это удастся, сборка выдаст `FASTPQ_METAL_LIB=<path>`; среда выполнения считывает это значение для детерминированной загрузки Metallib.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Установите `FASTPQ_SKIP_GPU_BUILD=1` при кросс-компиляции без набора инструментов Metal; сборка выводит предупреждение, и планировщик остается на пути ЦП.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Узлы автоматически переключаются на ЦП, если Metal недоступен (отсутствует платформа, неподдерживаемый графический процессор или пустой `FASTPQ_METAL_LIB`); сценарий сборки очищает переменную окружения, и планировщик регистрирует переход на более раннюю версию.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### Контрольный список выпуска (этап 6)
Сохраняйте билет выпуска FASTPQ заблокированным до тех пор, пока все приведенные ниже элементы не будут заполнены и прикреплены.

1. **Показатели проверки за доли секунды** — проверьте только что захваченный `fastpq_metal_bench_*.json` и
   подтвердите запись `benchmarks.operations`, где `operation = "lde"` (и зеркальный
   Образец `report.operations`) сообщает `gpu_mean_ms ≤ 950` для рабочей нагрузки в 20 000 строк (дополненные 32 768 строк).
   ряды). Съемки за пределами потолка требуют повторных запусков, прежде чем можно будет подписать контрольный список.
2. **Подписанный манифест** — Запустить
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   поэтому билет выпуска содержит как манифест, так и его отдельную подпись.
   (`artifacts/fastpq_bench_manifest.sig`). Рецензенты проверяют пару дайджест/подпись перед
   продвижение выпуска.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Манифест матрицы (построенный
   через `scripts/fastpq/capture_matrix.sh`) уже кодирует пол строки из 20 тысяч и
   отладка регрессии.
3. **Вложения с доказательствами** – загрузите JSON теста Metal, стандартный журнал (или трассировку инструментов),
   Выходные данные манифеста CUDA/Metal и отдельная подпись к билету выпуска. Запись в контрольном списке
   должен ссылаться на все артефакты плюс отпечаток открытого ключа, используемый для подписи, поэтому последующие аудиты
   можно повторить этап проверки.【artifacts/fastpq_benchmarks/README.md:65】### Рабочий процесс проверки металла
1. После сборки с поддержкой графического процессора подтвердите, что `FASTPQ_METAL_LIB` указывает на `.metallib` (`echo $FASTPQ_METAL_LIB`), чтобы среда выполнения могла загрузить его детерминированно.【crates/fastpq_prover/build.rs:188】
2. Запустите пакет контроля четности с включенными полосами графического процессора:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Серверная часть будет использовать ядра Metal и регистрировать детерминированный резервный процессор в случае сбоя обнаружения.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Создайте эталонный образец для информационных панелей:\
   найдите скомпилированную библиотеку Metal (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   экспортируйте его через `FASTPQ_METAL_LIB` и запустите\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Канонический профиль `fastpq-lane-balanced` теперь дополняет каждый захват до 32 768 строк (2¹⁵), поэтому JSON содержит как `rows`, так и `padded_rows` вместе с задержкой Metal LDE; перезапустите захват, если `zero_fill` или настройки очереди выводят LDE графического процессора за пределы целевого значения 950 мс (<1 с) на хостах серии AppleM. Архивируйте полученный JSON/журнал вместе с другими свидетельствами выпуска; ночной рабочий процесс macOS выполняет тот же запуск и загружает свои артефакты для сравнения.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Если вам нужна телеметрия только Poseidon (например, для записи трассировки приборов), добавьте `--operation poseidon_hash_columns` к приведенной выше команде; стенд по-прежнему будет учитывать `FASTPQ_GPU=gpu`, выдавать `metal_dispatch_queue.poseidon` и включать новый блок `poseidon_profiles`, поэтому пакет выпуска явно документирует узкое место Poseidon.
  Доказательства теперь включают `zero_fill.{bytes,ms,queue_delta}` плюс `kernel_profiles` (для каждого ядра).
  загруженность, расчетные ГБ/с и статистику продолжительности), поэтому эффективность графического процессора можно построить без
  повторная обработка необработанных трассировок и блок `twiddle_cache` (попадания/промахи + `before_ms`/`after_ms`), который
  доказывает, что кэшированные загрузки Twiddle действуют. `--trace-dir` перезапускает жгут под
  `xcrun xctrace record` и
  хранит файл `.trace` с отметкой времени вместе с JSON; вы все еще можете предоставить индивидуальный заказ
  `--trace-output` (с дополнительным `--trace-template` / `--trace-seconds`) при захвате в
  пользовательское местоположение/шаблон. JSON записывает `metal_trace_{template,seconds,output}` для аудита.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】После каждого захвата запускайте `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`, чтобы публикация содержала метаданные хоста (теперь включая `metadata.metal_trace`) для пакета платы/предупреждений Grafana (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). Отчет теперь содержит объект `speedup` для каждой операции (`speedup.ratio`, `speedup.delta_ms`), оболочка поднимает `zero_fill_hotspots` (байты, задержка, полученные ГБ/с и дельта-счетчики очереди Metal), сглаживает `kernel_profiles` в `benchmarks.kernel_summary`, сохраняет блок `twiddle_cache` нетронутым, копирует новый блок/сводку `post_tile_dispatches`, чтобы рецензенты могли доказать, что многопроходное ядро работало во время захвата, и теперь суммирует данные микробенча Poseidon в `benchmarks.poseidon_microbench`, чтобы информационные панели могли указать задержку скаляра по сравнению с задержкой по умолчанию без повторной обработки. сырой отчет. Шлюз манифеста считывает один и тот же блок и отклоняет пакеты свидетельств графического процессора, в которых он отсутствует, вынуждая операторов обновлять захваты всякий раз, когда путь после тайлинга пропускается или неправильно настроен.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Ядро Poseidon2 Metal имеет те же ручки: `FASTPQ_METAL_POSEIDON_LANES` (32–256, степени двойки) и `FASTPQ_METAL_POSEIDON_BATCH` (1–32 состояния на полосу) позволяют закрепить ширину запуска и работу на каждой полосе без перестройки; хост передает эти значения через `PoseidonArgs` перед каждой отправкой. По умолчанию среда выполнения проверяет `MTLDevice::{is_low_power,is_headless,location}`, чтобы сместить дискретные графические процессоры в сторону запуска на уровне VRAM (`256×24`, когда сообщается о ≥48 ГБ, `256×20` при 32 ГБ, `256×16` в противном случае), в то время как маломощные SoC остаются на `256×8` (и более старые части 128/64 полос придерживаются 8/6 состояний на полосу), поэтому большинству операторов никогда не нужно устанавливать переменные окружения вручную. `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` и генерирует блок `poseidon_microbench`, который записывает оба профиля запуска, а также измеренное ускорение по сравнению со скалярной полосой, чтобы пакеты релизов могли доказать, что новое ядро действительно сокращает `poseidon_hash_columns`, и он включает блок `poseidon_pipeline`, поэтому данные Stage7 фиксируют ручки глубины/перекрытия фрагментов наряду с новыми уровнями занятости. Оставьте env неустановленным для обычных запусков; жгут автоматически управляет повторным выполнением, регистрирует сбои, если захват дочерних элементов не может быть запущен, и немедленно завершает работу, когда установлен `FASTPQ_GPU=gpu`, но серверная часть графического процессора недоступна, поэтому тихие резервные варианты ЦП никогда не проникают в производительность. артефакты.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Оболочка отклоняет захваты Poseidon, в которых отсутствуют дельта `metal_dispatch_queue.poseidon`, общие счетчики `column_staging` или блоки свидетельств `poseidon_profiles`/`poseidon_microbench`, поэтому операторы должны обновить любой захват, который не может доказать перекрытие промежуточной подготовки или скалярное значение по умолчанию. ускорение.【scripts/fastpq/wrap_benchmark.py:732】 Если вам нужен отдельный JSON для информационных панелей или дельт CI, запустите `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; помощник принимает как упакованные артефакты, так и необработанные захваты `fastpq_metal_bench*.json`, выдавая `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` со стандартными/скалярными таймингами, настройкой метаданных и записанным ускорением.【scripts/fastpq/export_poseidon_microbench.py:1】
  Завершите выполнение, выполнив `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`, чтобы контрольный список выпуска Stage6 обеспечил соблюдение максимального уровня LDE `<1 s` и выдал подписанный пакет манифеста/дайджеста, который поставляется с билетом на выпуск.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Перед развертыванием проверьте телеметрию: сверните конечную точку Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) и проверьте журналы `telemetry::fastpq.execution_mode` на наличие непредвиденных `resolved="cpu"`. записи.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Задокументируйте резервный путь ЦП, намеренно задав его (`FASTPQ_GPU=cpu` или `zk.fastpq.execution_mode = "cpu"`), чтобы плейбуки SRE оставались согласованными с детерминированным поведением.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Дополнительная настройка: по умолчанию хост выбирает 16 полос для коротких трасс, 32 для средних и 64/128 один раз `log_len ≥ 10/14`, приземляясь на 256, когда `log_len ≥ 18`, и теперь он сохраняет плитку общей памяти на пяти этапах для небольших трасс, четырех один раз `log_len ≥ 12` и 14.12.16 этапы для `log_len ≥ 18/20/22` перед началом работы над ядром после тайлинга. Экспортируйте `FASTPQ_METAL_FFT_LANES` (степень двойки между 8 и 256) и/или `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) перед выполнением описанных выше шагов, чтобы переопределить эту эвристику. Размеры пакетов столбцов FFT/IFFT и LDE зависят от разрешенной ширины группы потоков (≈2048 логических потоков на отправку, ограничено 32 столбцами и теперь увеличивается до 32 → 16 → 8 → 4 → 2 → 1 по мере роста домена), в то время как путь LDE по-прежнему применяет ограничения домена; установите `FASTPQ_METAL_FFT_COLUMNS` (1–32), чтобы закрепить детерминированный размер пакета БПФ, и `FASTPQ_METAL_LDE_COLUMNS` (1–32), чтобы применить то же переопределение к диспетчеру LDE, когда вам нужно побитовое сравнение между хостами. Глубина тайла LDE также отражает эвристику БПФ — трассировки с `log₂ ≥ 18/20/22` выполняют только 12/10/8 стадий общей памяти перед передачей широких бабочек ядру после тайлинга — и вы можете переопределить это ограничение с помощью `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). Среда выполнения пропускает все значения через аргументы ядра Metal, ограничивает неподдерживаемые переопределения и протоколирует разрешенные значения, поэтому эксперименты остаются воспроизводимыми без пересборки Metallib; эталонный JSON отображает как разрешенную настройку, так и бюджет нулевого заполнения хоста (`zero_fill.{bytes,ms,queue_delta}`), полученный с помощью статистики LDE, поэтому дельты очереди привязаны непосредственно к каждому захвату, и теперь добавляет блок `column_staging` (пакеты сглаживаются, Flatten_ms, wait_ms, wait_ratio), чтобы рецензенты могли проверить перекрытие хоста/устройства, вызванное конвейером с двойной буферизацией. Когда графический процессор отказывается сообщать телеметрию о заполнении нулями, система теперь синтезирует детерминированное время из очистки буфера на стороне хоста и вводит его в блок `zero_fill`, поэтому доказательства выпуска никогда не отправляются без поле.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Распределение нескольких очередей происходит автоматически на дискретных компьютерах Mac: когда `Device::is_low_power()` возвращает false или металлическое устройство сообщает о слоте/внешнем расположении, хост создает два экземпляра `MTLCommandQueue`, разветвляется только после того, как рабочая нагрузка несет ≥16 столбцов (масштабируется разветвлением), и выполняет циклический перебор пакетов столбцов по очередям, поэтому длинные трассировки удерживают обе линии графического процессора занятыми без ущерба для детерминизма. Переопределите политику с помощью `FASTPQ_METAL_QUEUE_FANOUT` (1–4 очереди) и `FASTPQ_METAL_COLUMN_THRESHOLD` (минимальное общее количество столбцов перед разветвлением), когда вам требуется воспроизводимый захват данных на разных машинах; тесты на четность вызывают эти переопределения, поэтому компьютеры Mac с несколькими графическими процессорами остаются закрытыми, а разрешенное разветвление / пороговое значение регистрируется рядом с глубиной очереди телеметрия.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Доказательства в архив
| Артефакт | Захват | Заметки |
|----------|---------|-------|
| Комплект `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` и `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, за которыми следуют `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` и `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Доказывает, что Metal CLI/toolchain был установлен и создал детерминированную библиотеку для этого коммита.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Снимок окружающей среды | `echo $FASTPQ_METAL_LIB` после сборки; сохраните абсолютный путь с вашим билетом на выпуск. | Пустой вывод означает, что Metal отключен; запись ценностных документов о том, что полосы графического процессора остаются доступными на отгрузочном артефакте.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| Журнал четности графического процессора | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` и заархивируйте фрагмент, содержащий `backend="metal"` или предупреждение о переходе на более раннюю версию. | Демонстрирует, что ядра запускаются (или детерминированно откатываются) перед продвижением сборки.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Контрольный результат | И18НИ00000129Х; оберните и подпишите через `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | Обернутые записи JSON `speedup.ratio`, `speedup.delta_ms`, настройка БПФ, дополненные строки (32 768), расширенные `zero_fill`/`kernel_profiles`, сплющенные `kernel_summary`, проверенные `metal_dispatch_queue.poseidon`/`poseidon_profiles` блокируется (при использовании `--operation poseidon_hash_columns`) и метаданные трассировки, поэтому среднее значение LDE графического процессора остается ≤950 мс, а Poseidon остается <1 с; сохраните как пакет, так и сгенерированную подпись `.json.asc` вместе с билетом на выпуск, чтобы панели мониторинга и аудиторы могли проверить артефакт без повторного запуска. рабочие нагрузки.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Манифест скамейки | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Проверяет оба артефакта графического процессора, завершается сбоем, если среднее значение LDE превышает потолок `<1 s`, записывает дайджесты BLAKE3/SHA-256 и выдает подписанный манифест, поэтому контрольный список выпуска не может продвигаться без проверки. metrics.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| Пакет CUDA | Запустите `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` на лабораторном хосте SM80, оберните/подпишите JSON в `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (используйте `--label device_class=xeon-rtx-sm80`, чтобы панели мониторинга выбрали правильный класс), добавьте путь к `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` и сохраните пару `.json`/`.asc` с Металлический артефакт перед регенерацией манифеста. Зарегистрированный `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` иллюстрирует точный формат пакета, ожидаемый аудиторами.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| Доказательство телеметрии | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` плюс журнал `telemetry::fastpq.execution_mode`, создаваемый при запуске. | Подтверждает, что Prometheus/OTEL предоставляет `device_class="<matrix>", backend="metal"` (или журнал перехода на более раннюю версию) перед включением трафика.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 || Принудительное сверление процессора | Запустите короткий пакет с `FASTPQ_GPU=cpu` или `zk.fastpq.execution_mode = "cpu"` и запишите журнал перехода на более раннюю версию. | Обеспечивает соответствие модулей Runbook SRE детерминированному резервному пути на случай, если в середине выпуска потребуется откат.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Захват трассировки (необязательно) | Повторите проверку четности с помощью `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` и сохраните созданную трассировку отправки. | Сохраняет данные о занятости/группе потоков для последующих проверок профилирования без повторного запуска тестов.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Многоязычные файлы `fastpq_plan.*` ссылаются на этот контрольный список, поэтому операторы подготовки и производства используют один и тот же путь доказательств.【docs/source/fastpq_plan.md:1】

## Воспроизводимые сборки
Используйте рабочий процесс закрепленного контейнера для создания воспроизводимых артефактов Stage6:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Вспомогательный сценарий создает образ цепочки инструментов `rust:1.88.0-slim-bookworm` (и `nvidia/cuda:12.2.2-devel-ubuntu22.04` для графического процессора), запускает сборку внутри контейнера и записывает `manifest.json`, `sha256s.txt` и скомпилированные двоичные файлы в целевой выходной файл. каталог.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Переопределения среды:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – закрепить явную базу/тег Rust.
- `FASTPQ_CUDA_IMAGE` — замените базу CUDA при создании артефактов графического процессора.
- `FASTPQ_CONTAINER_RUNTIME` – принудительно установить определенное время выполнения; по умолчанию `auto` пытается `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – разделенный запятыми порядок предпочтений для автоматического обнаружения во время выполнения (по умолчанию `docker,podman,nerdctl`).

## Обновления конфигурации
1. Установите режим выполнения во время выполнения в вашем TOML:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Значение анализируется через `FastpqExecutionMode` и передается в серверную часть при запуске.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. При необходимости переопределите при запуске:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI переопределяет разрешенную конфигурацию до загрузки узла.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Разработчики могут временно принудительно обнаруживать, не затрагивая конфигурации, путем экспорта
   `FASTPQ_GPU={auto,cpu,gpu}` перед запуском двоичного файла; переопределение регистрируется и конвейер
   по-прежнему отображается разрешенный режим.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Контрольный список проверки
1. **Журналы запуска**
   - Ожидайте `FASTPQ execution mode resolved` от цели `telemetry::fastpq.execution_mode` с
     Ярлыки `requested`, `resolved` и `backend`.【crates/fastpq_prover/src/backend.rs:208】
   - При автоматическом обнаружении графического процессора вторичный журнал от `fastpq::planner` сообщает о последней полосе.
   - Поверхность металлических хостов `backend="metal"` при успешной загрузке Metallib; если компиляция или загрузка завершаются неудачей, сценарий сборки выдает предупреждение, очищает `FASTPQ_METAL_LIB`, а планировщик записывает `GPU acceleration unavailable`, прежде чем оставаться включенным. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Метрики Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Счетчик увеличивается через `record_fastpq_execution_mode` (теперь помечен как
   `{device_class,chip_family,gpu_kind}`) всякий раз, когда узел разрешает свое выполнение
   режим.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Для металлического покрытия подтвердите
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     увеличивается вместе с вашими панелями развертывания.【crates/iroha_telemetry/src/metrics.rs:5397】
   - узлы macOS, скомпилированные с `irohad --features fastpq-gpu`, дополнительно предоставляют
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     и
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}`, поэтому панели управления Stage7
     может отслеживать рабочий цикл и запас очереди на основе данных Prometheus в режиме реального времени.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Экспорт телеметрии**
   - сборки OTEL излучают `fastpq.execution_mode_resolutions_total` с теми же метками; убедитесь, что ваш
     информационные панели или оповещения отслеживают неожиданный `resolved="cpu"`, когда графические процессоры должны быть активны.

4. **Доказательство/проверка здравомыслия**
   - Запустите небольшую партию через `iroha_cli` или интеграционный жгут и подтвердите проверку доказательств на
     одноранговый узел скомпилирован с теми же параметрами.

## Устранение неполадок
- **Разрешенный режим сохраняет процессор на хостах с графическим процессором** — убедитесь, что двоичный файл был собран с помощью
  `fastpq_prover/fastpq-gpu`, библиотеки CUDA находятся на пути загрузчика, а `FASTPQ_GPU` не принудительно
  `cpu`.
- **Metal недоступен на Apple Silicon** — убедитесь, что инструменты CLI установлены (`xcode-select --install`), повторно запустите `xcodebuild -downloadComponent MetalToolchain` и убедитесь, что сборка создала непустой путь `FASTPQ_METAL_LIB`; пустое или отсутствующее значение автоматически отключает серверную часть.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **Ошибки `Unknown parameter`** — убедитесь, что и проверяющий, и проверяющий используют один и тот же канонический каталог.
  испущенный `fastpq_isi`; не соответствует поверхности как `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Неожиданный отказ ЦП** — проверьте `cargo tree -p fastpq_prover --features` и
  убедитесь, что `fastpq_prover/fastpq-gpu` присутствует в сборках графического процессора; убедитесь, что библиотеки `nvcc`/CUDA находятся в пути поиска.
- **Отсутствует счетчик телеметрии** — убедитесь, что узел был запущен с `--features telemetry` (по умолчанию).
  и что экспорт OTEL (если он включен) включает конвейер метрик.【crates/iroha_telemetry/src/metrics.rs:8887】

## Резервная процедура
Детерминированный заполнитель был удален. Если регрессия требует отката,
повторно развернуть ранее заведомо исправные артефакты выпуска и провести исследование перед повторным выпуском Stage6.
двоичные файлы. Задокументируйте решение по управлению изменениями и убедитесь, что переход вперед завершается только после
регрессия понятна.

3. Отслеживайте телеметрию, чтобы убедиться, что `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` соответствует ожидаемому
   выполнение заполнителя.

## Базовый уровень оборудования
| Профиль | процессор | графический процессор | Заметки |
| ------- | --- | --- | ----- |
| Ссылка (Stage6) | AMD EPYC7B12 (32 ядра), 256 ГБ ОЗУ | NVIDIA A10040GB (CUDA12.2) | Синтетические пакеты из 20 000 строк должны выполняться в течение ≤1000 мс.【docs/source/fastpq_plan.md:131】 |
| только для процессора | ≥32 физических ядер, AVX2 | – | Ожидайте ~0,9–1,2 с для 20 000 строк; оставьте `execution_mode = "cpu"` для детерминизма. |## Регрессионные тесты
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (на хостах с графическим процессором)
- Дополнительная проверка золотых приспособлений:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Задокументируйте любые отклонения от этого контрольного списка в своей рабочей книге и обновите `status.md` после
Окно миграции завершается.