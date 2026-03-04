---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
Название: دليل تشغيل محاكاة سعة SoraFS
Sidebar_label: Добавить комментарий
описание: تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c باستخدام светильники قابلة لإعادة الإنتاج Например Prometheus или Grafana.
---

:::примечание
Был установлен `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Он был изображен в фильме "Сфинкс" и "Территория" в Сфинксе.
:::

В 1997 году он был отправлен на базу SF-2c в 1998 году. Это так. Он был показан во время аварийного переключения, а также резких движений в светильниках, проведенных в Сан-Франциско. Это относится к `docs/examples/sorafs_capacity_simulation/`. Полезные нагрузки السعة تستخدم `sorafs_manifest_stub capacity`; Создайте `iroha app sorafs toolkit pack` для манифеста/CAR.

## 1. Просмотр артефактов в интерфейсе командной строки

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Для `run_cli.sh` для `sorafs_manifest_stub capacity` для Norito полезных данных и больших двоичных объектов в base64 и для Torii Формат JSON:

- Скарлетт Льюис Мейсон в Сан-Франциско в Нью-Йорке.
- Он был убит в 1990-х годах в Вашингтоне.
- Чтобы выполнить аварийное переключение, необходимо выполнить аварийное переключение.
- полезная нагрузка نزاع يطلب рубит بعد الانقطاع المُحاكَى.

Он был использован в программе `./artifacts` (на английском языке) وسيط). راجع ملفات `_summary.json` للحصول على سياق مقروء.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

В ответ на это:

- `capacity_simulation_report.json` - Выполняется аварийное переключение, а также выполняется аварийное переключение.
- `capacity_simulation.prom` - Текстовый файл для Prometheus (`sorafs_simulation_*`) используется для сбора текстовых файлов с помощью node-exporter и очистки заданий.

Для очистки Prometheus:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Сборщик текстовых файлов — `capacity_simulation.prom` (открывается с помощью node-exporter انسخه إلى الدليل الممرر عبر `--collector.textfile.directory`).

## 3. Установите флажок Grafana.

1. Установите Grafana, установите `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Установите флажок `Prometheus` и очистите его.
3. Сообщение в ресторане:
   - **Распределение квот (ГиБ)**
   - **Триггер аварийного переключения** Активируется *Аварийное переключение активно*.
   - **Снижение времени безотказной работы во время сбоя** Ошибка была вызвана обновлением `alpha`.
   - **Запрошенный процент слэша** в зависимости от текущего матча.

## 4. Справочная информация

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي `600` طالما بقي الإجمالي الملتزم >=600.
- `sorafs_simulation_failover_triggered` вместо `1` используется для установки `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (косая черта 15%) для `alpha`.

Установите `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` в соответствии со светильниками и используйте интерфейс CLI.