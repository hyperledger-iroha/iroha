---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: развертывание разработчика
Название: ملاحظات نشر SoraFS
Sidebar_label: Открыть
описание: قائمة لترقية خط أنابيب SoraFS в CI إلى الإنتاج.
---

:::примечание
Создан на `docs/source/sorafs/developer/deployment.md`. Он был убит в 1980-х годах в Нью-Йорке.
:::

# ملاحظات النشر

Код SoraFS находится в центре внимания CI в CI. Он сказал, что это не так. Он был убит в 1990-х годах в Нью-Йорке и в Нью-Йорке.

## قبل التشغيل

- **Можно использовать** — Скачивает и использует чанкера والمانيفستات تشير إلى نفس ثلثية `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — Рэйчел Уинстон, псевдоним المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Получить информацию о программе** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (псевдоним Тэхёна, إخفاقات النسخ المتماثل).

## إعدادات البيئة

- Для запуска приложения CLI (`POST /v2/sorafs/proof/stream`) используйте CLI. إصدار ملخصات التليمترية.
- Установите `sorafs_alias_cache` и откройте интерфейс командной строки `iroha_config`. Ошибка (`sorafs_cli manifest submit --alias-*`).
- Он сказал, что он (Джон Бэнхан اعتماد Torii) عبر مدير أسرار آمن.
- Для получения дополнительной информации (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) установите флажок Prometheus/OTel الخاصة بك.

## استراتيجية الإطلاق

1. **Синий/зеленый**
   - Установите `manifest submit --summary-out` для проверки подлинности.
   - راقب `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق القدرات مبكرًا.
2. **Получить в подарок**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ Он был свидетелем того, как в 2007 году он был в Уэльсе, а затем в Сан-Франциско.
   - يجب يكون `proof verify` جزءًا من اختبار Smoke بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لا يزال يطابق дайджест المانيفست.
3. **Получить информацию**
   - Установите `docs/examples/sorafs_proof_streaming_dashboard.json` или Grafana.
   - Задайте диапазон фрагментов (`docs/source/sorafs/runbooks/pin_registry_ops.md`).
4. **Полный выбор**
   - اتبع خطوات الإطلاق المرحلي في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق, وأرشِف Табло آرتيفاكتات/التليمترية لأغراض التدقيق.

## التعامل مع الحوادث

- Обновление для `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` позволяет получить поток-токен.
  - `dispute_revocation_runbook.md` عند وقوع نزاعات النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى العقدة.
  - `multi_source_rollout.md` لتجاوزات المُنسِّق, وإدراج الأقران في القائمة السوداء, والإطلاق Хорошо.
- Зарегистрируйтесь в GovernanceLog в GovernanceLog и выберите PoR-трекер. Он сказал, что Стив Сейлор находится в центре внимания.

## الخطوات التالية

- ادمج أتمتة المُنسِّق (`sorafs_car::multi_fetch`) (СФ-6б).
- Поддержка PDP/PoTR для SF-13/SF-14; Откройте интерфейс командной строки и выполните следующие действия: تلك الأدلة.

В 2007 году в Нью-Йорке, США, в Нью-Йорке, Нью-Йорк, США, США, США, США, США, США, США, США, США, США, США, США, США и США. Ссылка на приложение SoraFS جاهزة للإنتاج بعملية قابلة للتكرار وقابلة للرصد.