---
lang: ar
direction: rtl
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sorafs_capacity_simulation/README.md -->

# عدة محاكاة سعة SoraFS

تحتوي هذه الدليل على artefacts قابلة لاعادة الانتاج لمحاكاة سوق السعة SF-2c. تقوم العدة بتمرين تفاوض الحصص والتعامل مع failover واصلاحات slashing باستخدام مساعدات CLI للانتاج ونص تحليل خفيف.

## المتطلبات المسبقة

- سلسلة ادوات Rust قادرة على تشغيل `cargo run` لاعضاء workspace.
- Python 3.10+ (المكتبة القياسية فقط).

## البدء السريع

```bash
# 1. توليد artefacts CLI قياسية
./run_cli.sh ./artifacts

# 2. تجميع النتائج واصدار مقاييس Prometheus
./analyze.py --artifacts ./artifacts
```

سكربت `run_cli.sh` يستدعي `sorafs_manifest_stub capacity` لبناء:

- تصريحات مزودين حتمية لمجموعة fixtures لتفاوض الحصص.
- ترتيب تكرار يطابق سيناريو التفاوض.
- لقطات تليمترى لنافذة الفشل.
- payload نزاع يلتقط طلب slashing.

يقوم السكربت بكتابة Norito bytes (`*.to`) و payloads base64 (`*.b64`) و Torii request
bodies وملخصات مقروءة (`*_summary.json`) تحت دليل artefact المختار.

يقوم `analyze.py` باستهلاك الملخصات المولدة، وينتج تقريرا مجمعا
(`capacity_simulation_report.json`)، ويصدر ملف نصي Prometheus
(`capacity_simulation.prom`) يتضمن:

- مقاييس `sorafs_simulation_quota_*` التي تصف السعة المتفاوض عليها وحصة التخصيص لكل مزود.
- مقاييس `sorafs_simulation_failover_*` التي تبرز فروقات التوقف ومزود الاستبدال المختار.
- `sorafs_simulation_slash_requested` التي تسجل نسبة المعالجة المستخرجة من payload النزاع.

استورد حزمة Grafana في `dashboards/grafana/sorafs_capacity_simulation.json`
ووجهها الى مصدر Prometheus يسحب الملف النصي المولد (مثلا عبر textfile collector في
node-exporter). يشرح دليل التشغيل في
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` سير العمل الكامل،
بما في ذلك نصائح تهيئة Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` — مواصفات تصريح المزود وترتيب التكرار.
- `scenarios/failover/` — نوافذ التليمترى لانقطاع الاساس ورفع failover.
- `scenarios/slashing/` — مواصفات نزاع تشير الى نفس ترتيب التكرار.

يتم التحقق من هذه fixtures في `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
لضمان بقائها متزامنة مع مخطط CLI.

</div>
