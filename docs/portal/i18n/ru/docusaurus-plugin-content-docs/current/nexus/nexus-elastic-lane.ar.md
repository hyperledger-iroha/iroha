---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Elastic-Lane
титул: تجهيز Lane المرن (NX-7)
Sidebar_label: تجهيز Lane المرن
описание: Запуск начальной загрузки демонстрирует развертывание линии Nexus.
---

:::примечание
Был установлен `docs/source/nexus_elastic_lane.md`. Он был убит в 2007 году в Колумбийском университете.
:::

# مجموعة ادوات تجهيز Lane المرن (NX-7)

> **Обратный доступ:** NX-7 – переулок ادوات تجهيز المرن  
> **الحالة:** الادوات مكتملة - تولد Manifes, مقتطفات الكتالوج، حمولات Norito, اختبارات курить,
> Пакетный пакет, созданный в соответствии со слотом + манифесты, доступные для просмотра. اختبارات حمل المدققين
> Дэн Сэрри Сити.

هذا الدليل يوجه الذي يقوم بأتمتة Вы можете манифестировать развертывание полосы движения и пространства данных. Названия переулков для Nexus (на английском языке) в عدة Он был убит в 1980-х годах в Нью-Йорке.

## 1. Настройки

1. Создать псевдоним لـlane и dataspace, чтобы сохранить доступ к нему (`f`) Усадебное поселение.
2. Создание пространства имен (معرفات الحسابات) для создания пространств имен.
3. Позвоните по телефону, чтобы узнать больше о вещах.
4. На экране отображается полоса пропускания (انظر `nexus.registry.manifest_directory` и `cache_directory`).
5. Дэйв Лайн в Нью-Йорке/Миниатюра PagerDuty использует переулка, когда он работает в режиме реального времени. Пройдите по переулку в городе.

## 2. Переулок артефактов Сейла

Сообщение от президента США:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Уважаемый:

- `--lane-id` соответствует индексу исходного кода для `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` в пространстве данных Dataspace (пространство данных id الخاص بالlane) عند الحذف).
- `--validator` для `--validators-file`.
- `--route-instruction` / `--route-account` تصدر قواعد توجيه جاهزة للصق.
- `--metadata key=value` (او `--telemetry-contact/channel/runbook`) Откройте для себя Runbook, чтобы просмотреть его. صحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` позволяет выполнить обновление во время выполнения манифеста, который находится в переулке, который находится в процессе выполнения.
- `--encode-space-directory` на `cargo xtask space-directory encode`. Зарегистрируйтесь на `--space-directory-out` и нажмите на `.to` для проверки. الافتراضي.

Сохранение артефактов داخل `--output-dir` (создание и исчезновение артефактов), Кодировка اختياري عند تفعيل:

1. `<slug>.manifest.json` — манифест для определения кворума и пространств имен для обновления среды выполнения перехватчика.
2. `<slug>.catalog.toml` — TOML используется для `[[nexus.lane_catalog]]` и `[[nexus.dataspace_catalog]]` в автономном режиме. Используется для `fault_tolerance` в пространстве данных, подключенном к полосе-реле (`3f+1`).
3. `<slug>.summary.json` — развертывание развертывания с помощью пули (slug والقطاعات والبيانات) Установите `cargo xtask space-directory encode` (например, `space_directory_encode.command`). Воспользуйтесь JSON, чтобы пройти адаптацию.
4. `<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`; Установите `iroha app space-directory manifest publish` или Torii.

Создайте `--dry-run` для JSON/экрана, чтобы просмотреть файлы и `--force`. Найдите артефакты.## 3. تطبيق التغييرات

1. Создайте манифест JSON в формате `nexus.registry.manifest_directory` (каталог кэша и реестр и удаленный доступ). В фильме "Капитан Кейн" проявляется синдром исчезновения в напряжении.
2. Установите флажок `config/config.toml` (или `config.d/*.toml`). Для `nexus.lane_count` используется `lane_id + 1` для `nexus.routing_policy.rules`, для `nexus.routing_policy.rules`. На переулке الجديد.
3. Создайте (зарегистрируйтесь `--encode-space-directory`) манифест в каталоге Space Directory в разделе «Сводка» (`space_directory_encode.command`). Для полезной нагрузки `.manifest.to` используется Torii. ارسله عبر `iroha app space-directory manifest publish`.
4. Введите `irohad --sora --config path/to/config.toml --trace-config` и выполните трассировку при развертывании. Его можно использовать для слизняков / кура Кура.
5. Сделайте это в соответствии с описанием манифеста/отображения. Сводная информация в формате JSON для редактирования файлов.

## 4. Бандажная связка

В манифесте Манифеста и оверлее Лос-Анджелеса в Нью-Йорке в Нью-Йорке переулки переулков. Проверьте конфигурации здесь и сейчас. Созданный упаковщик ينسخ манифестирует оверлейное наложение الختياري لكاتالوج الحوكمة لـ. `nexus.registry.cache_directory`, а также архив tarball в автономном режиме:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

المخرجات:

1. `manifests/<slug>.manifest.json` — установите флажок `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - ضعها في `nexus.registry.cache_directory`. В качестве `--module` было установлено защитное устройство, в котором он находится. Для (NX-2) используется наложение, установленное на `config.toml`.
3. `summary.json` — хеш-коды и наложение хеш-кодов.
4. Добавлен `registry_bundle.tar.*` — добавлен SCP в S3 и обнаружены артефакты.

Дэниел Билли (او الارشيف) Лил Миссури, Уинстон, хосты, معزولة, وانسخ, манифесты + оверлей Установите флажок Torii.

## 5. اختبارات курить للمدققين

بعد اعادة تشغيل Torii, شغّل مساعد Smoke الجديد للتحقق من ان Lane يبلّغ `manifest_ready=true`, В 2008 году были закрыты переулки, а в Нью-Йорке запечатаны. يجب ان تعرض Lanes التي تتطلب манифестирует قيمة `manifest_path` غير فارغة؛ В качестве примера, приведенного в сообщении, опубликованном на сайте NX-7 в манифесте:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`--insecure` был установлен самоподписанным. Переулок Кейна, переулок, опечатанный и запечатанный, находится в центре города. المقاييس/القياس عن القيم المتوقعة. Установите `--min-block-height` и `--max-finality-lag` и `--max-settlement-backlog` и `--max-headroom-events`, чтобы перейти к переулку (ارتفاع) الكتلة/النهائية/backlog/headroom) ضمن حدود التشغيل, واربطها مع `--max-slot-p95` / `--max-slot-p99` (مع) `--min-slot-samples`) Закройте слот для NX-18.

Устройство с воздушным зазором (CI) для подключения к Torii для подключения к сети Для конечной точки:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Фикстуры `fixtures/nexus/lanes/` для артефактов и файлов начальной загрузки lint манифесты Он сказал Дэну Сеймерсу. Для CI используются `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`) для дыма. NX-7 используется для создания полезной нагрузки и создания дайджестов/наложений для набора пакетов. لاعادة الانتاج.