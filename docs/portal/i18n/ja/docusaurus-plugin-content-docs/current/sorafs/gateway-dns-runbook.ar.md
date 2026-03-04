---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ゲートウェイ وDNS في SoraFS

عكس نسخة البوابة هذا الدليل التشغيلي المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
分散型 DNS およびゲートウェイ 分散型 DNS およびゲートウェイ
2025-03 年 2025 年 3 月 2025 年 3 月に開催されました。

## いいえ

- DNS (SF-4) とゲートウェイ (SF-5) の接続。
  TLS/GAR とリゾルバー。
- إبقاء مدخلات الانطلاقة (الأجندة، الدعوة، متعقب الحضور، لقطة تليمترية GAR)
  重要な情報を入力してください。
- إنتاج حزمة آرتيفاكتات قابلة للتدقيق لمراجعي الحوكمة: ملاحظات إصدار دليل
  リゾルバー、ゲートウェイ、ドキュメント/DevRel。

## いいえ

|名前: और देखें और देखें
|----------|---------------|--------------------------|
|ネットワーク TL (DNS) |解決策は、RAD のリゾルバーです。 | `artifacts/soradns_directory/<ts>/`، فروق `docs/source/soradns/deterministic_hosts.md`، وبيانات RAD الوصفية。 |
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR と `sorafs-gateway-probe` が PagerDuty をサポートします。 | `artifacts/sorafs_gateway_probe/<ts>/`JSON プローブ `ops/drill-log.md`。 |
| QA ギルドおよびツーリング WG | `ci/check_sorafs_gateway_conformance.sh` の備品は、Norito の自己証明書です。 | `artifacts/sorafs_gateway_conformance/<ts>/`×`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel | دوين المحاضر، تحديث 先読み التصميم + الملاحق، ونشر ملخص الأدلة في هذا البوابة。 | `docs/source/sorafs_gateway_dns_design_*.md` を確認してください。 |

## دخلات والمتطلبات المسبقة

- مواصفة المضيفات الحتمية (`docs/source/soradns/deterministic_hosts.md`) وبنية
  リゾルバー (`docs/source/soradns/resolver_attestation_directory.md`)。
- ゲートウェイ: TLS/ECH ダイレクト モード
  自己証明書 `docs/source/sorafs_gateway_*`。
- 番号: `cargo xtask soradns-directory-release`
  `cargo xtask sorafs-gateway-probe`Ì `scripts/telemetry/run_soradns_transparency_tail.sh`Ì
  `scripts/sorafs_gateway_self_cert.sh`، وأدوات CI المساعدة
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- メッセージ: GAR メッセージ、ACME メッセージ、DNS/TLS メッセージ、PagerDuty メッセージ
  Torii リゾルバー。

## قائمة التحقق قبل التنفيذ

1. 最高のパフォーマンス
   `docs/source/sorafs_gateway_dns_design_attendance.md` وتعميم الأجندة الحالية
   (`docs/source/sorafs_gateway_dns_design_agenda.md`)。
2. 重要な情報
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. フィクスチャ (マニフェスト الخاصة بـ GAR، أدلة RAD، حزم توافق ゲートウェイ)
   重要なのは `git submodule` です。
4. التحقق من الأسرار (مفتاح إصدار Ed25519، ملف حساب ACME، رمز PagerDuty) ومطابقة
   チェックサムはボールトに保存されます。
5. スモークテスト لأهداف التليمترية (エンドポイント الخاص بـ Pushgateway ، لوحة GAR في Grafana)
   ああ、それは。

## خطوات تمرين الأتمتة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. マニフェストをマニフェストする
   ドリフト マシン ドリフト マシン
   `docs/source/soradns/deterministic_hosts.md`。
2. リゾルバー:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. تدوين معرّف الدليل المطبوع وSHA-256 ومسارات الإخراج داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` وفي محاضر الانطلاقة。

### DNS を使用する

- テストリゾルバー ≥10 個のリゾルバー
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway の実行 ID を確認します。

### ゲートウェイ

1. TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 自己証明書 (`ci/check_sorafs_gateway_conformance.sh`) 自己証明書
   (`scripts/sorafs_gateway_self_cert.sh`) は Norito です。
3. PagerDuty/Webhook はエンドツーエンドで機能します。

### 評価

- تحديث `ops/drill-log.md` بالطوابع الزمنية والمشاركين وهاشات プローブ。
- 実行 ID を確認し、Docs/DevRel を確認します。
- ログインしてください。

## إدارة الجلسة وتسليم الأدلة

- ** 名前:**
  - T-24 h — プログラム管理 التذكير + الأجندة/الحضور في `#nexus-steering`。
  - T-2 h — يقوم Networking TL بتحديث لقطة تليمترية GAR وتسجيل الفروقات في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`。
  - T-15 m — Ops Automation のプローブは、ID を実行します。`artifacts/sorafs_gateway_dns/current`。
  - أثناء المكالمة — يشارك المشرف هذا الدليل ويُعيّن ناسخًا مباشرًا؛ Docs/DevRel を参照してください。
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ومنعكس في バンドル البوابة)
  重要な問題は、次のとおりです。重要な問題は、次のとおりです。
  هاشات الأدلة والمخاطر المفتوحة。
- **رفع الأدلة:** اضغط مجلد `runbook_bundle/` الخاص بالتمرين، أرفق PDF المحاضر
  SHA-256 في المحاضر + الأجندة، ثم نبّه اسم المراجعين
  `s3://sora-governance/sorafs/gateway_dns/<date>/` を参照してください。

## لقطة الأدلة (انطلاقة مارس 2025)

آخر الآرتيفاكتات المرتبطة بالخارطة والمحاضر محفوظة في
`s3://sora-governance/sorafs/gateway_dns/`。 और देखें
マニフェスト (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - タールボール: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF 番号: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة مباشرة — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(رفع متوقع: `gateway_dns_minutes_20250303.pdf` — ستضيف Docs/DevRel قيمة SHA-256 عند توفر PDF في الحزمة.)_

## عرض المزيد

- [دليل تشغيل عمليات ゲートウェイ](./operations-playbook.md)
- [خطة مراقبة SoraFS](./observability-plan.md)
- [DNS ゲートウェイ](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)