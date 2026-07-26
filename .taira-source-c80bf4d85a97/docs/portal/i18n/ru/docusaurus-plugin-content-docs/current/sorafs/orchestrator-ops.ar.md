---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: оркестратор-опс
Название: دليل تشغيل مُنسِّق SoraFS
Sidebar_label: دليل تشغيل المُنسِّق
описание: Дэниел Тэйлор, персонаж фильма "Лейнер" Нынче.
---

:::примечание
Был установлен `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Он был убит в фильме "Сфинкс" в фильме "Сфинкс". القديمة بالكامل.
:::

Он был создан в SRE в 2017 году в 2017 году. المصادر. Он сыграл в фильме "Страна" в фильме "Солнце". Он сказал, что это не так.

> **راجع أيضًا:** يركّز [دليل إطلاق متعدد المصادر](./multi-source-rollout.md) على موجات الإطلاق Он был убит в 2007 году в Нью-Йорке. ارجع إليه لتنسيق الحوكمة / بيئة الاختبار المرحلية Pينما تستخدم هذا المستند لعمليات المُنسِّق اليومية.

## 1. قائمة التحقق قبل التنفيذ

1. **День рождения **
   - Установите флажок (`ProviderAdvertV1`) для установки защитного кожуха.
   - Установите флажок (`plan.json`) для получения дополнительной информации.
2. **Добавлено лишнее слово**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Создан для `artifacts/scoreboard.json` и установлен в `eligible`.
   - Создание изображения JSON Дэвида Бэтмена в формате JSON. Затем нанесите куски на куски и нарежьте их.
3. **Зарегистрированные светильники تجريبي باستخدام** — новые светильники, установленные на `docs/examples/sorafs_ci_sample/`. Он был создан в 1980-х годах в 1980-х годах в 1980-х годах. إنتاج.

## 2. إجراء الإطلاق المرحلي

1. **Диапазон исчезновения (≤2 месяца)**
   - أعد بناء لوحة النتائج وشغّل باستخدام `--max-peers=2` لتقييد المُنسِّق بمجموعة فرعية صغيرة.
   - Рэй:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - تابع عندما تبقى معدلات إعادة المحاولة أقل من 1% لجلب كامل للمانيفست ولا Он был в Миссисипи.
2. **Отличный вариант (50% от общего количества)**
   - Создан `--max-peers` и установлен в приложении.
   - احتفظ بكل تشغيل عبر `--provider-metrics-out` и `--chunk-receipts-out`. Срок действия ≥7 дней.
3. **إطلاق كامل**
   - أزل `--max-peers` (أو اضبطه على العدد الكامل للمزوّدين المؤهلين).
   - Для просмотра изображений в формате JSON: используйте файлы JSON. Нажмите здесь, чтобы получить больше информации.
   - حدّث لوحات المتابعة لعرض `sorafs_orchestrator_fetch_duration_ms` p95/p99, чтобы получить дополнительную информацию. المنطقة.

## 3. حظر وتعزيز النظراء

Откройте интерфейс командной строки и нажмите кнопку «Добавить» в интерфейсе командной строки. تحديثات الحوكمة.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` будет установлен в режиме онлайн-обновления.
- `--boost-provider=<alias>=<weight>` يرفع المُزوّد ​​في المُجدول. Он сказал: Хорошо.
- Скачивание в режиме реального времени в формате JSON Люка в обложке журнала В 2008 году он был открыт для посещения.

Нападающий "Старый игрок" нанес штрафной удар Кроме того, вы можете получить доступ к интерфейсу командной строки.

## 4. تشخيص الإخفاقات

Сказал, что принес:1. Как выполнить процедуру проверки:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Установите `session.summary.json` для проверки подлинности:
   - `no providers were supplied` → تحقّق من مسارات المزوّدين والإعلانات.
   - `retry budget exhausted ...` → زد `--retry-budget` и нажмите кнопку .
   - `no compatible providers available ...` → دقّق بيانات قدرات النطاق للمزوّد المخالف.
3. Установите флажок `sorafs_orchestrator_provider_failures_total` и установите его на место.
4. Выполните выборку для получения изображения `--scoreboard-json`, чтобы получить доступ к файлу. Он сказал Пьеру Дэвису.

## 5. Уведомление

لمُنسِّق:

1. Установите флажок `--max-peers=1` (отключено от источника питания). أعد العملاء إلى مسار принести الأحادي المصدر القديم.
2. Установите флажок `--boost-provider` в режиме ожидания.
3. Он сказал: Он принесет тебе.

Он был выбран в честь Дня независимости США и его семьи В 2017 году он был назначен президентом США. Это было сделано для того, чтобы сделать это.