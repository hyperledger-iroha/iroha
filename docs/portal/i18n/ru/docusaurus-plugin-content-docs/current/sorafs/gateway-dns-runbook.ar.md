---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Введите адрес шлюза и DNS для SoraFS.

تعكس نسخة البوابة هذا الدليل التشغيلي المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Для создания децентрализованного DNS и шлюза в Интернете
Он провел матч за сборную США в сезоне 2025–03.

## النطاق والمخرجات

- Доступ к DNS (SF-4) и шлюзу (SF-5)
  Доступны резольверы, TLS/GAR и другие устройства.
- Дэвис Миссисипи (Старый город, Колумбия, Великобритания)
  Он был создан в Колумбии.
- Написано в журнале "Лидер продаж": Убийца.
  резольверы, создание шлюза, проверка безопасности и документации Docs/DevRel.

## الأدوار والمسؤوليات

| المسار | المسؤوليات | Информационные технологии |
|--------|-------------|------------------------|
| Сетевой TL (под управлением DNS) | Он был отправлен в отдел RAD, а затем в RAD. Распознавательные резольверы. | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md`, RAD الوصفية. |
| Руководитель отдела автоматизации операций (шлюз) | Подключите TLS/ECH/GAR, установите `sorafs-gateway-probe` и включите PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, пробник JSON, файл `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | تشغيل `ci/check_sorafs_gateway_conformance.sh`, дополнительные светильники, وأرشفة حزم self-cert الخاصة بـ Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | تدوين المحاضر، تحديث предварительно прочитайте التصميم + الملاحق, ونشر ملخص الأدلة في هذا البوابة. | Проверьте `docs/source/sorafs_gateway_dns_design_*.md`. |

## المدخلات والمتطلبات المسبقة

- Дополнительная информация о программе (`docs/source/soradns/deterministic_hosts.md`)
  Дополнительные резольверы (`docs/source/soradns/resolver_attestation_directory.md`).
- Межсетевой шлюз: прямой доступ, TLS/ECH, прямой режим,
  Имеется самосертификат `docs/source/sorafs_gateway_*`.
- Сообщение: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, подключение CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Доступ: вызов GAR, поддержка ACME для DNS/TLS, поддержка PagerDuty,
  Установите резольверы Torii.

## قائمة التحقق قبل التنفيذ

1. تأكيد الحضور والأجندة بتحديث
   `docs/source/sorafs_gateway_dns_design_attendance.md` وتعميم الأجندة الحالية
   (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Тэхен Билли
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Светильники (демонстрирует наличие GAR, RAD, шлюза) и шлюза.
   Был установлен `git submodule`, который был установлен.
4. Зарегистрируйтесь на сайте (на английском языке Ed25519, на сайте ACME, в PagerDuty)
   контрольные суммы в хранилище.
5. Запустите дымовой тест в режиме онлайн (конечная точка на сервере Pushgateway, в GAR на Grafana)
   قبل التمرين.

## خطوات تمرين الأتمتة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. Внешность человека, страдающего от ярости, проявляет недовольство собой.
   В дрифте مقارنةً
   `docs/source/soradns/deterministic_hosts.md`.
2. Используйте резольверы:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Установленный в блоке управления SHA-256 блок управления.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` в случае необходимости.

### Проверка DNS- Наличие резольверов с числом ≥10 дней.
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Откройте приложение Pushgateway и проверьте идентификатор запуска NDJSON.

### تمارين أتمتة шлюз

1. Подключение к TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Проверка работоспособности (`ci/check_sorafs_gateway_conformance.sh`) с самопроверкой
   (`scripts/sorafs_gateway_self_cert.sh`) Установите флажок Norito.
3. Запустите PagerDuty/Webhook и выполните сквозное соединение.

### تجميع الأدلة

- Установите `ops/drill-log.md` для проверки состояния датчика.
- В разделе «Просмотр идентификатора запуска» используется документ «Docs/DevRel».
- ربط حزمة الأدلة في تذكرة الحوكمة قبل مراجعة الانطلاقة.

## إدارة الجلسة وتسليم الأدلة

- **الخط الزمني للمشرف:**
  - T-24 h — Управление программой التذكير + لقطة الأجندة/الحضور في `#nexus-steering`.
  - T-2 h — Сеть Networking TL работает в GAR и использует `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - Т-15 м — используется системой автоматизации операций, когда зонды компании DAF запускают идентификатор `artifacts/sorafs_gateway_dns/current`.
  - أثناء المكالمة — Джон Уилсон и Уилсон Нэнси Мэнсон Документы/DevRel опубликованы в разделе «Документы/DevRel».
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (отсутствует в комплекте поставки)
  Он был убит в 1980-х годах. تضمّن قائمة الحضور والقرارات وبنود العمل
  Вы можете сделать это в любое время.
- **Просмотр:** Загрузите файл `runbook_bundle/` в PDF-файл.
  Он был создан на базе SHA-256 в режиме "+" в режиме реального времени".
  Он был создан для `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة الأدلة (начало 2025 г.)

آخر الآرتيفاكتات المرتبطة بالخارطة والمحاضر محفوظة في
`s3://sora-governance/sorafs/gateway_dns/`. الهاشات أدناه تعكس
В манифесте المعتمد (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Файл архива: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`.
  - PDF-файл: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`.
- **Отправлено — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(описано: `gateway_dns_minutes_20250303.pdf` — ستضيف Docs/DevRel для SHA-256 в формате PDF в формате PDF.)_

## مواد ذات صلة

- [Для шлюза шлюза](./operations-playbook.md)
- [خطة مراقبة SoraFS](./observability-plan.md)
- [Открытый DNS-шлюз](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)