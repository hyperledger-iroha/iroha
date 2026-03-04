---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pq-ratchet-runbook
כותרת: Учебная тревога PQ Ratchet SoraNet
sidebar_label: Runbook PQ Ratchet
תיאור: Шаги חזרות כוננות для повышения или понижения стадийной PQ מדיניות אנונימיות с детерминированной telemetry валидацией.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_ratchet_runbook.md`. אם יש לך מסמכים נוספים, לא ניתן למצוא את המסמכים.
:::

## Назначение

Этот runbook описывает последовательность תרגיל אש למדיניות אנונימיות פוסט-קוונטי (PQ) SoraNet. מפעילים отрабатывают как קידום (שלב A -> שלב B -> שלב C), так и הורדה מבוקרת обратно к Stage B/A при падении supply PQ. מקדחה валидирует ווי טלמטריה (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) ושיתוף חפצים ליומן חזרות של תקריות.

## דרישות מוקדמות

- Самый свежий `sorafs_orchestrator` בינארי с יכולת שקלול (התחייב равен или позже תרגיל ייחוס из `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к Prometheus/Grafana מחסנית, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- תמונת מצב של ספריית המשמר של Номинальный. Получите и проверьте копию до תרגיל:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ספריית המקור Если публикует только JSON, перекодируйте его в Norito בינארי через `soranet-directory build` перед запуском עוזרי סיבוב.

- צור מטא נתונים וחפצי אמנות קדם-שלביים, המנפיק באמצעות помощью CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- שינוי חלון одобрен כוננות командами רשתות וצפייה.

## שלבי קידום

1. **ביקורת שלב**

   שלב Зафиксируйте стартовый:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   מבצע Перед ожидайте `anon-guard-pq`.

2. **קידום בשלב ב' (רוב PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 דקות למניפסטים.
   - В Grafana (לוח מחוונים `SoraNet PQ Ratchet Drill`) убедитесь, что панель "אירועי מדיניות" показывает `outcome=met` ל-Prometheus.
   - צילום מסך או JSON панели и приложите к יומן אירועים.

3. **קידום בשלב ג' (קפדנית PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Проверьте, что היסטוגרמה `sorafs_orchestrator_pq_ratio_*` стремится к 1.0.
   - Убедитесь, что brownout counter остается плоским; иначе выполните шаги הורדה.

## תרגיל ירידה בדרגה / חום

1. **Индуцируйте синтетический дефицит PQ**

   Отключите PQ relays в playground среде, обрезав guard directory до классических ערכים, затем перезагрузите מטמון מתזמר:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **לא בלוחמת טלמטריה**

   - לוח מחוונים: панель "Brownout Rate" подскакивает выше 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщить `anonymity_outcome="brownout"` с `anonymity_reason="missing_majority_pq"`.

3. **הורדה עד שלב ב' / שלב א'**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Если supply PQ все еще недостаточен, понизьте до `anon-guard-pq`. Drill завершен, когда דלפקי בראון стабилизируются и מבצעים можно повторно применить.

4. **מדריך המשמר של Восстановление**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## טלמטריה וחפצי אמנות- **לוח מחוונים:** `dashboards/grafana/soranet_pq_ratchet.json`
- **התראות Prometheus:** убедитесь, что brownout alert `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (<5% в люмбоим 10-мон).
- **יומן תקריות:** הצג קטעי טלמטריה ותקשורת ב-`docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **לכידה חתומה:** используйте `cargo xtask soranet-rollout-capture`, чтобы скопировать יומן תרגיל ולוח תוצאות ב-`artifacts/soranet_pq_rollout/<timestamp>/`, вычислить BLAKE3 תקצירים и сопировать сопировать `rollout_capture.json`.

דוגמה:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Приложите сгенерированные metadata и חתימה к пакету ממשל.

## חזרה לאחור

מקדחה רגילה PQ, בוצעה בשלב א', התקנת Networking TL ו-Prilожите собранныествествет של מדדי מעקב מדדי שמירה של מדריך תקריות. Используйте ранее захваченный ייצוא ספריית השומר, чтобы восстановить нормальный сервис.

:::tip כיסוי רגרסיה
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` предоставляет אימות סינטטי, מקדחה которая поддерживает этот.
:::