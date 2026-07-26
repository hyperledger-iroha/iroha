---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# دليل تشغيل قابلية الملاحظة لمرساة Taikai

تعكس نسخة البوابة هذا الدليل المعتمد في
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
استخدمه عند تدريب مراسي routing-manifest (TRM) الخاصة بـ SN13-C حتى يتمكن مشغلو
SoraFS/SoraNet من ربط آرتيفاكتات spool وتليمترية Prometheus وأدلة الحوكمة دون
مغادرة معاينة البوابة.

## النطاق والمالكون

- **البرنامج:** SN13-C — manifests الخاصة بـ Taikai ومراسي SoraNS.
- **المالكون:** Media Platform WG وDA Program وNetworking TL وDocs/DevRel.
- **الهدف:** توفير دليل حتمي لتنبيهات Sev 1/Sev 2، والتحقق من التليمترية، وجمع
  الأدلة بينما تتحرك routing manifests الخاصة بـ Taikai عبر aliases.

## البدء السريع (Sev 1/Sev 2)

1. **التقاط آرتيفاكتات spool** — انسخ أحدث الملفات
   `taikai-anchor-request-*.json` و`taikai-trm-state-*.json` و
   `taikai-lineage-*.json` من
   `config.da_ingest.manifest_store_dir/taikai/` قبل إعادة تشغيل العمال.
2. **تفريغ تليمترية `/status`** — سجّل مصفوفة
   `telemetry.taikai_alias_rotations` لإثبات نافذة المانيفست النشطة:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **فحص لوحات المتابعة والتنبيهات** — حمّل
   `dashboards/grafana/taikai_viewer.json` (مرشحات cluster + stream) ودوّن ما إذا
   كانت أي قواعد في
   `dashboards/alerts/taikai_viewer_rules.yml` قد أطلقت (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`، وأحداث صحة PoR الخاصة بـ SoraFS).
4. **فحص Prometheus** — نفّذ الاستعلامات في قسم "مرجع المقاييس" لتأكيد أن
   latencies/‏drift الخاصة بالـ ingest وعدّادات دوران alias تعمل كما ينبغي.
   صعّد إذا توقّف `taikai_trm_alias_rotations_total` عبر عدة نوافذ أو إذا زادت
   عدّادات الأخطاء.

## مرجع المقاييس

| المقياس | الغرض |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | مخطط latency لعمليات ingest بنمط CMAF لكل cluster/stream (الهدف: p95 < 750 ms، p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Drift للـ live-edge بين encoder وعمال المرساة (تنبيه عند p99 > 1.5 s لمدة 10 دقائق). |
| `taikai_ingest_segment_errors_total{reason}` | عدّادات الأخطاء حسب السبب (`decode`, `manifest_mismatch`, `lineage_replay`, ...). أي زيادة تُطلق `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | يزداد عند قبول `/v1/da/ingest` لـ TRM جديد لاسم مستعار؛ استخدم `rate()` للتحقق من وتيرة الدوران. |
| `/status → telemetry.taikai_alias_rotations[]` | لقطة JSON تتضمن `window_start_sequence` و`window_end_sequence` و`manifest_digest_hex` و`rotations_total` والطوابع الزمنية لحزم الأدلة. |
| `taikai_viewer_*` (rebuffer, عمر دوران CEK, صحة PQ, التنبيهات) | مؤشرات KPI للعارض لضمان بقاء دوران CEK ودوائر PQ سليمة أثناء المراسي. |

### مقتطفات PromQL

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## لوحات المتابعة والتنبيهات

- **لوحة Grafana للعارض:** `dashboards/grafana/taikai_viewer.json` — latencies p95/p99،
  drift للـ live-edge، أخطاء المقاطع، عمر دوران CEK، وتنبيهات العارض.
- **لوحة Grafana للكاش:** `dashboards/grafana/taikai_cache.json` — ترقيات hot/warm/cold
  ورفض QoS عند دوران نوافذ alias.
- **قواعد Alertmanager:** `dashboards/alerts/taikai_viewer_rules.yml` — تنبيهات drift،
  تحذيرات فشل ingest، تأخر دوران CEK، وعقوبات/فترات تبريد صحة PoR لـ SoraFS. تأكد
  من إعداد مستلمين لكل cluster إنتاجي.

## قائمة التحقق لحزمة الأدلة

- آرتيفاكتات spool (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- شغّل `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  لإصدار جرد JSON موقّع للأغلفة المعلّقة/المسلّمة ونسخ ملفات request/SSM/TRM/lineage
  إلى حزمة التمرين. المسار الافتراضي لـ spool هو `storage/da_manifests/taikai` من `torii.toml`.
- لقطة `/status` التي تغطي `telemetry.taikai_alias_rotations`.
- صادرات Prometheus (JSON/CSV) للمقاييس أعلاه خلال نافذة الحادثة.
- لقطات Grafana مع ظهور المرشحات.
- معرفات Alertmanager التي تشير إلى حالات الإطلاق ذات الصلة.
- رابط إلى `docs/examples/taikai_anchor_lineage_packet.md` الذي يصف حزمة الأدلة المعتمدة.

## عكس لوحات المتابعة وإيقاع التدريبات

تلبية متطلب SN13-C تعني إثبات أن لوحات Taikai viewer/cache منعكسة داخل البوابة
**و** أن تدريب الأدلة للمرساة يعمل بإيقاع متوقع.

1. **انعكاس البوابة.** عند تغيّر `dashboards/grafana/taikai_viewer.json` أو
   `dashboards/grafana/taikai_cache.json`، لخّص الفروقات في
   `sorafs/taikai-monitoring-dashboards` (هذه البوابة) وسجّل checksums JSON في
   وصف PR الخاص بالبوابة. أبرز اللوحات/العتبات الجديدة لربطها بمجلد Grafana المُدار.
2. **تدريب شهري.**
   - نفّذ التدريب في أول ثلاثاء من كل شهر عند 15:00 UTC حتى تصل الأدلة قبل
     اجتماع حوكمة SN13.
   - التقط آرتيفاكتات spool وتليمترية `/status` ولقطات Grafana داخل
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - سجّل التنفيذ عبر
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **المراجعة والنشر.** خلال 48 ساعة، راجع التنبيهات/الإيجابيات الكاذبة مع
   DA Program وNetOps، وثّق المتابعات في drill log، واربط رفع حزمة الحوكمة من
   `docs/source/sorafs/runbooks-index.md`.

إذا تأخرت لوحات المتابعة أو التدريبات، فلن تتمكن SN13-C من الخروج من 🈺؛ حافظ على
تحديث هذا القسم عند تغيير الإيقاع أو توقعات الأدلة.

## أوامر مفيدة

```bash
# لقطة تليمترية دوران alias إلى دليل آرتيفاكتات
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# سرد إدخالات spool لاسم مستعار/حدث محدد
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# فحص أسباب عدم تطابق TRM من سجل spool
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

حافظ على تزامن نسخة البوابة هذه مع الدليل المعتمد عند تغيّر تليمترية مراسي
Taikai أو لوحات المتابعة أو متطلبات أدلة الحوكمة.
