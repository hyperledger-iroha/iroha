---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
Название: البدء السريع لـ Lane الافتراضي (NX-5)
Sidebar_label: Нажмите на переулок.
Описание: Создается для резервного варианта Lane الافتراضي в Nexus для Torii и SDK дляlane_id На переулках.
---

:::примечание
Был установлен `docs/source/quickstart/default_lane.md`. Он сказал, что в действительности он был убит в фильме "Самоооооооооооо".
:::

# البدء السريع لـ Lane الافتراضي (NX-5)

> **Добавлено сообщение:** NX-5 — переулок Пейдж-Лейн в окрестностях города. Вызов резервного варианта `nexus.routing_policy.default_lane` при вызове REST/gRPC для Torii. В SDK используется `lane_id`, созданный в соответствии с каноническими правилами. В качестве резервного варианта для `/status` используется резервный вариант `/status`. Смолил в Лос-Анджелесе.

## المتطلبات المسبقة

- Установите Sora/Nexus на `irohad` (на `irohad --sora --config ...`).
- وصول إلى مستودع الإعدادات حتى تتمكن من تعديل أقسام `nexus.*`.
- `iroha_cli` находится в центре внимания.
- `curl`/`jq` (также в футляре) для `/status` или Torii.

## 1. Доступ к полосе движения и пространству данных

Доступны полосы движения и пространства данных, а также пространство для хранения данных. Вызов (в формате `defaults/nexus/config.toml`) для полос движения и псевдонимов пространства данных Ответ:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Он был отправлен в США по `index`. Пространство данных в формате 64-й версии. На улице Кейси-Лейн-Лейнс.

## 2. ضبط افتراضات التوجيه والتجاوزات الاختيارية

قسم `nexus.routing_policy` на переулке Лейн, штат Нью-Йорк, США. Да, это так. Для этого используйте планировщик `default_lane` и `default_dataspace`. Маршрутизатор используется для `crates/iroha_core/src/queue/router.rs` и используется для подключения к Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

В Лейнс-Лейнс-Лейнс-Сити, штат Калифорния, в Нью-Йорке. Он находится на Уилл-лейн, а затем на Уилл-лейн, где находится город-республиканец. Используйте SDK для проверки.

## 3. إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Вы можете сделать это в ближайшее время. تظهر أي أخطاء تحقق (فهارس مفقودة, псевдонимы مكررة, معرفات dataspace غير صالحة) قبل بدء сплетни.

## 4. تأكيد حالة حوكمة переулок

Откройте онлайн-интерфейс, откройте интерфейс CLI в переулке الافتراضي مختوم (манифест محمّل) وجاهز للحركة. Обратите внимание на переулок:

```bash
iroha_cli app nexus lane-report --summary
```

Пример вывода:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Он находится в переулке `sealed`, в списке runbook, который находится в переулке. علم `--fail-on-sealed` для CI.

## 5. Введите код Torii.

Установите `/status` и установите планировщик в переулок. `curl`/`jq` находится в районе переулка. Ответ на вопрос:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Пример вывода:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Запустите планировщик по полосе `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Он был назначен псевдонимом TEU и объявил о своем манифесте. На экране появится сообщение Grafana о включении полосы пропускания.## 6. اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` и ящик, созданный Rust, для `lane_id`, созданный для `--lane-id` / `LaneSelector`. Откройте маршрутизатор очереди `default_lane`. Установите флажок `--lane-id`/`--dataspace-id`, расположенный в переулке.
- **JS/Swift/Android.** Загрузить SDK с помощью `laneId`/`lane_id`. Установите флажок `/status`. Тэхен Сэнсэйр в фильме "Бонн" постановка и продюсирование لإعادة تهيئة طارئة.
- **Тесты конвейера/SSE.** Проверьте наличие `tx_lane_id == <u32>` (`docs/source/pipeline.md`). اشترك في `/v2/pipeline/events/transactions` بهذا الشرط لإثبات أن الكتابات المرسلة بدون Lane صريح تصل تحت معرف переулок الاحتياطي.

## 7. Информационный бюллетень

- `/status` используется для `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases` в Alertmanager в приложении Alertmanager. манифест полосы движения. Там вы найдете информацию о devnets.
- Запустите планировщик для создания дорожек (`dashboards/grafana/nexus_lanes.json`) и укажите псевдоним/слизень в списке. Его псевдоним был назван в честь Куры, когда он был главой государства. Он был установлен (в версии NX-1).
- Нажмите на полосы движения, чтобы откатить назад. Создайте хеш в манифесте и создайте базу данных в Runbook, чтобы получить доступ к базе данных. Сделайте это в ближайшее время.