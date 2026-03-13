---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
title: Реестр контактов عمليات
sidebar_label: Реестр контактов عمليات
описание: Создан реестр контактов для SoraFS в соответствии с SLA للتكرار.
---

:::примечание
تعكس `docs/source/sorafs/runbooks/pin_registry_ops.md`. Он был создан в честь Сфинкса.
:::

## نظرة عامة

Для этого необходимо создать реестр контактов SoraFS. (SLA) Установите `iroha_torii` и установите Prometheus, чтобы установить `torii_sorafs_*`. Введите Torii в реестр реестра на 30-е число дней после завершения регистрации. Он был создан в 2007 году в рамках проекта `/v2/sorafs/pin/*`. Установите флажок (`docs/source/grafana_sorafs_pin_registry.json`) для установки Grafana. Это было сделано для того, чтобы сделать это.

## مرجع المقاييس

| المقياس | Этикетки | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Человек проявляет ярость в глазах окружающих. |
| `torii_sorafs_registry_aliases_total` | — | Псевдонимы можно указать в файле манифеста в реестре. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Он был убит в 2007 году. |
| `torii_sorafs_replication_backlog_total` | — | Манометр установлен на `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Соответствие SLA: `met` для получения сертификата `missed`. Установите + `pending` для проверки работоспособности. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Хейн Уилсон (Ради Уинстон Уилсон). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Он был главой государства (المهلة ناقص ايبوك الاصدار). |

Датчики Дэниела Сэнсэя и снимок Стоуна Ллаха, Ларри Уилсон и Сан-Франциско в Лос-Анджелесе. Загрузите `1m`.

## Grafana

Создайте файл JSON для создания файла, созданного в формате JSON. Он был убит Кейном Стоуном и его коллегой по работе.

1. **Информационные манифесты** – `torii_sorafs_registry_manifests_total` (مجموعة حسب `status`).
2. **псевдоним اتجاه كتالوج** — `torii_sorafs_registry_aliases_total`.
3. **Полный доступ к сети** – `torii_sorafs_registry_orders_total` (отображение `status`).
4. **Отставание в работе над обновлением** – `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}`.
5. **Согласование соглашения об уровне обслуживания** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Воспроизведение видеозаписи** – для `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Установите флажок Grafana для установки `min_over_time`. Сообщение от Сэнсэля:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Время ожидания (продолжительность 1 час)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه- **Соглашение об уровне обслуживания  0**
  - Сообщение: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`.
  - Сообщение: В настоящее время наблюдается необратимый отток поставщиков услуг.
- **p95 للاكتمال > متوسط هامش المهلة**
  - Код: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`.
  - Информация: Информация о поставщиках услуг в режиме онлайн. В اعادة الاسناد.

### امثلة قواعد Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "هبوط SLA لتكرار SoraFS تحت الهدف"
          description: "ظلت نسبة نجاح SLA اقل من 95% لمدة 15 دقيقة."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "تراكم تكرار SoraFS فوق العتبة"
          description: "تجاوزت اوامر التكرار المعلقة ميزانية التراكم المضبوطة."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "انتهاء اوامر تكرار SoraFS"
          description: "انتهى امر تكرار واحد على الاقل في الخمس دقائق الماضية."
```

## سير عمل الفرز

1. **Первый день**
   - Соглашение об уровне обслуживания должно быть предоставлено поставщикам услуг (например, PoR и поставщикам услуг). المتاخر).
   - اذا زاد التراكم مع اخفاقات مستقرة, افحص القبول (`/v2/sorafs/pin/*`) لتاكيد проявляется Да, это правда.
2. **Обращение к поставщикам услуг**
   - `iroha app sorafs providers list` используется в качестве источника питания для проверки.
   - Датчики `torii_sorafs_capacity_*` установлены в GiB в соответствии с PoR.
3. **Полный выбор**
   - Для установки `sorafs_manifest_stub capacity replication-order` необходимо установить блок управления (`stat="avg"`) в течение 5 дней. Проверьте (открытый манифест/CAR يستخدم `iroha app sorafs toolkit pack`).
   - Используйте псевдонимы تفتقر لربط манифеста (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **Получить информацию**
   - Ссылка на приложение SoraFS в разделе "Дайджесты".
   - Он сказал, что Сэнсэй Уинстон в Лос-Анджелесе.

## خطة الاطلاق

Найдите имя пользователя и имя кэша псевдонима в разделе:1. **Получить информацию**
   - Задайте `torii.sorafs_alias_cache` и `iroha_config` (пользователь -> фактический) Задайте значения TTL для следующих значений: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Установите флажок `docs/source/sorafs_alias_policy.md`.
   - Добавление SDK и возможность использования привязок Rust/NAPI/Python (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` для привязок Rust/NAPI/Python). Он был отправлен в Лондон.
2. **Пробный прогон и постановка**
   - Он выступил с постановкой спектакля "Старый мир".
   - شغّل `cargo xtask sorafs-pin-fixtures` для светильников الكنسية للـ alias, ما زالت تفك التشفير وتقوم بعملية туда и обратно; Он выступил с дрейфом в Нью-Йорке и в Нью-Йорке.
   - Конечные точки `/v2/sorafs/pin/{digest}` и `/v2/sorafs/aliases` могут быть заменены новыми, обновленными окнами, истекшими и жесткими истекшими сроками действия. Для HTTP-заголовков (`Sora-Proof-Status`, `Retry-After`, `Warning`) и JSON-файла.
3. **Вечеринка в ресторане**
   - اطرح الاعدادات الجديدة في نافذة التغيير القياسية. Установите Torii для получения шлюзов/установки SDK в нужном месте. Сделайте это в ресторане.
   - Установите `docs/source/grafana_sorafs_pin_registry.json` или Grafana (необходимо установить кэш-память) Псевдоним в честь НОК.
4. **Вечернее сообщение**
   - Установите `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` через 30 дней. Он был создан на базе `error`/`expired`. Это означает, что у вас есть псевдонимы поставщиков услуг.
   - Добавление в пакет SDK для создания встроенного программного обеспечения (установленные SDK для загрузки). Срок годности истек). Он был убит в 2007 году.
5. **Резервный вариант**
   - Он Тэстер, псевдоним: Нэнси Тэхен, его отец, Миссисипи. `refresh_window` и `positive_ttl` в режиме ожидания. `hard_expiry` был отправлен в центральную часть города.
   - عد الى الاعداد السابق باستعادة snapshot السابق من `iroha_config` اذا استمرت التليمتري في اظهار اعداد `error` был создан псевдонимом افتح حادثة لتتبع تاخير توليد.

## مواد ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين, تكمل هذا الدليل.