---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска Gateway ו-DNS SoraFS

Эта копия в портале отражает канонический ранбук в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Она фиксирует операционные מעקות בטיחות עבור воркстрима מבוזר DNS & Gateway,
чтобы лиды по נטוורקינג, ops и документации могли отрепетировать стек
בעיטת הפתיחה автоматизации перед 2025-03.

## Область и התוצרים

- בדוק את ה-DNS (SF-4) ושער (SF-5) через репетицию детерминированной
  деривации хостов, релизов каталога resolvers, автоматизации TLS/GAR ו
  сбора доказательств.
- Держать kickoff-артефакты (סדר יום, הזמנה, מעקב נוכחות, תמונת מצב
  телеметрии GAR) синхронизированными с последними назначениями בעלים.
- Подготовить аудируемый חבילה артефактов לסוקרי ממשל: הערות שחרור
  רזולוצי קטאלוג, גישודי שערים, רתמת התאמה ותקשורת
  Docs/DevRel.

## Роли и ответственности

| זרם עבודה | Ответственности | Требуемые артефакты |
|------------|----------------|------------------------|
| רשת TL (מחסנית DNS) | שלח את הטלפונים החכמים, מצא את הגירסאות של ספריית RAD, פתח את פתרונות הטלפונים. | `artifacts/soradns_directory/<ts>/`, הבדלים ל-`docs/source/soradns/deterministic_hosts.md`, מטא נתונים של RAD. |
| Ops Automation Lead (שער) | Выполнять מקדחות автоматизации TLS/ECH/GAR, запускать `sorafs-gateway-probe`, обновлять ווים PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, בדיקה JSON, ספייס ב-`ops/drill-log.md`. |
| QA Guild & Tooling WG | Запускать `ci/check_sorafs_gateway_conformance.sh`, אביזרי курировать, архивировать Norito חבילות אישור עצמי. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | Фиксировать דקות, обновлять קריאה מוקדמת של עיצוב + נספחים, публиковать סיכום ראיות в портале. | Обновленные `docs/source/sorafs_gateway_dns_design_*.md` והערות השקה. |

## Входы и דרישות מוקדמות

- Спецификация детерминированных хостов (`docs/source/soradns/deterministic_hosts.md`) וכן
  אישור כרטיס לפותרים (`docs/source/soradns/resolver_attestation_directory.md`).
- שער Артефакты: מדריך למפעיל, עוזרי אוטומציה של TLS/ECH,
  הדרכה במצב ישיר וזרימת עבודה באישור עצמי под `docs/source/sorafs_gateway_*`.
- כלי עבודה: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  עוזרי `scripts/sorafs_gateway_self_cert.sh` и CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- סודות: ключ релиза GAR, אישורי DNS/TLS ACME, מפתח ניתוב PagerDuty,
  אסימון אישור Torii לשליפה פותרים.

## רשימת בדיקה לפני טיסה

1. Подтвердите участников и agenda, обновив
   `docs/source/sorafs_gateway_dns_design_attendance.md` ו разослав текущую
   סדר יום (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовьте корни артефактов, например
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` и
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. שפר את התקנים (מניפסטים של GAR, הוכחות RAD, שער התאמה לצרורות) וכן
   убедитесь, что состояние `git submodule` соответствует последнему תג חזרות.
4. צור סודות (מפתח שחרור Ed25519, קובץ חשבון ACME, אסימון PagerDuty) וכן
   соответствие סכומים בכספת.
5. צור יעדי טלמטריה לבדיקת עשן (נקודת קצה של Pushgateway, לוח GAR Grafana)
   תרגיל перед.

## Шаги репетиции автоматизации

### Детерминированная карта хостов и release каталога RAD1. Запустите helper детерминированной деривации хостов на предложенном наборе
   manifests и подтвердите отсутствие סחף относительно
   `docs/source/soradns/deterministic_hosts.md`.
2. רזולורי צרור קטאלוגאים:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зафиксируйте напечатанный ID каталога, SHA-256 ו-выходные пути внутри
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и в דקות בעיטה.

### Захват телеметрии DNS

- יומני שקיפות של פותר זנב в течение ≥10 минут с
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспортируйте метрики Pushgateway ו- архивируйте NDJSON צילומי מצב рядом с
  מזהה הפעלה של директориями.

### מקדחות автоматизации שער

1. בדיקת TLS/ECH בדיקה:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. הצג רתמת התאמה (`ci/check_sorafs_gateway_conformance.sh`) וכן
   אישור עצמי עוזר (`scripts/sorafs_gateway_self_cert.sh`) для обновления
   חבילת אישורים Norito.
3. Зафиксируйте PagerDuty/Webhook события, чтобы подтвердить מקצה לקצה работу
   пути автоматизации.

### Упаковка доказательств

- התקן את `ops/drill-log.md` עם חותמות זמן, בדיקות ובדיקות גיבוב.
- Сохраните артефакты в директориях ריצה מזהה ותקציר מנהלים
  в דקות Docs/DevRel.
- Сошлитесь на חבילת ראיות в governance тикете до סקירת פתיחה.

## Модерация сессии и передача доказательств

- **ציר הזמן модератора:**
  - T-24 שעות — ניהול תוכנית публикует напоминание + תמונת מצב סדר יום/נוכחות в `#nexus-steering`.
  - T-2 h — Networking TL обновляет תמונת מצב телеметрии GAR и фиксирует deltas в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation проверяет готовность probes и записывает активный run ID в `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор делится этим ранбуком и назначает סופר חי; Docs/DevRel фиксируют פריטי פעולה по ходу.
- **דקות Шаблон:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (также отражен в חבילת פורטל)
  и коммитьте заполненный экземпляр на каждую сессию. Включите список участников,
  решения, פריטי פעולה, גיבוב ראיות и открытые риски.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из החזרה,
  приложите отрендеренный דקות PDF, запишите SHA-256 hashes в דקות + סדר יום,
  затем уведомите סוקר ממשל כינוי после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (בעיטת מרץ 2025)

Последние חזרות/לחיות артефакты, упомянутые в מפת דרכים и דקות, лежат в bucket
`s3://sora-governance/sorafs/gateway_dns/`. Хэши ниже отражают канонический
מניפסט (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **ריצה יבשה — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - חבילת Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF של דקות: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **סדנה חיה — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(העלאה בהמתנה: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавит SHA-256 после попадания PDF в bundle.)_

## Связанные материалы

- [ספר פעולות הפעלה לשער](./operations-playbook.md)
- [План наблюдаемости SoraFS](./observability-plan.md)
- [Трекер децентрализованного DNS ו-gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)