---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: خطة تنفيذ Pin Registry في SoraFS
Sidebar_label: ピン レジストリ
説明: SF-4 レジストリ Torii 。
---

:::note ノート
評価は `docs/source/sorafs/pin_registry_plan.md` です。最高のパフォーマンスを見せてください。
:::

# 認証ピン レジストリ SoraFS (SF-4)

SF-4 の Pin レジストリとマニフェストの管理
API を Torii にピン留めしてください。
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain،
試合は、試合の試合結果を表します。

## ああ

1. **レジストリ**: Norito マニフェストのエイリアス
   ログインしてください。
2. **評価: CRUD ピン (`ReplicationOrder`、`Precommit`、
   `Completion`、エビクション)。
3. **واجهة الخدمة**: gRPC/REST レジストリ تستهلكها Torii وSDKs،
   ありがとうございます。
4. **フィクスチャ**: CLI の ومتجهات اختبار ووثائق تحافظ على تزامن
   マニフェスト、エイリアス、封筒など。
5.***: مقاييس وتنبيهات وrunbooks レジストリ。

## いいえ

### السجلات الاساسية (Norito)

|ああ |ああ |ああ |
|----------|----------|----------|
| `PinRecordV1` |マニフェストを表示します。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` |エイリアス -> CID マニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |マニフェストを表示します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |ありがとうございます。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ありがとうございます。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

セキュリティ: `crates/sorafs_manifest/src/pin_registry.rs` セキュリティ Norito セキュリティ Rust
هساعدات التحقق التي تدعم هذه السجلات.ツールとマニフェスト
(チャンカー レジストリとピン ポリシー ゲーティングのルックアップ) Torii وCLI
不変条件。

意味:
- 回答 Norito 回答 `crates/sorafs_manifest/src/pin_registry.rs`。
- バージョン (Rust + SDK バージョン) Norito。
- 評価 (`sorafs_architecture_rfc.md`) 評価。

## いいえ

|ああ | और देखें और देखें
|----------|------|----------|
|レジストリ (スレッド/sqlite/オフチェーン) スマート コントラクト。 |コアインフラ/スマートコントラクトチーム |ハッシュ化する必要があります。 |
|バージョン: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ | `ManifestValidator` を確認してください。別名 يمر الان عبر `RegisterPinManifest` (DTO من Torii) بينما يبقى `bind_alias` المخصص مخططا لتحديثاتああ。 |
|別名: فرض التعاقب (マニフェスト A -> B) 別名。 |ガバナンス評議会 / コアインフラ |別名 وحدود الاحتفاظ وفحوصات اعتماد/سحب السلف موجودة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات ودفاتر التكرار ما زالت مفتوحة。 |
|メッセージ: `ManifestPolicyV1` من config/حالة الحوكمة؛ありがとうございます。 |ガバナンス評議会 | CLI を使用してください。 |
|バージョン: Norito バージョン (`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`)。 |可観測性 | عريف مخطط الاحداث + التسجيل。 |

回答:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض)。
- اختبارات خصائص لسلسلة التعاقب (بدون دورات، عصور متصاعدة)。
- ファズ للتحقق عبر توليد は、عشوائية (مقيدة) を明示します。

## واجهة الخدمة (تكامل Torii/SDK)

|ああ、ああ | और देखें
|--------|--------|------|
| Torii | 「`/v2/sorafs/pin` (送信)」「`/v2/sorafs/pin/{cid}` (検索)」「`/v2/sorafs/aliases` (リスト/バインド)」「`/v2/sorafs/replication` (注文/領収書)」。翻訳: 翻訳: + 翻訳: |ネットワーキング TL / コア インフラ |
|認証済み | تضمين ارتفاع/هاش レジストリ في الاستجابات؛ Norito の SDK です。 |コアインフラ |
| CLI | `sorafs_manifest_stub` と CLI の `sorafs_pin` は、`pin submit`、`alias bind`、`order issue`、`registry export` です。 |ツーリングWG |
| SDK |バインディング (Rust/Go/TS) Norito؛ありがとうございます。 | SDK チーム |意味:
- キャッシュ/ETag を取得します。
- レート制限 / 認証 Torii。

## 試合日程とCI

- フィクスチャ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/オーダー `cargo run -p iroha_core --example gen_pin_snapshot`。
- CI: `ci/check_sorafs_fixtures.sh` 試合の試合結果CI です。
- バージョン (`crates/iroha_core/tests/pin_registry.rs`) バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン処理チャンカーを処理します。 عليها مسبقا/مسحوبة/ذاتية الاشارة)؛ `register_manifest_rejects_*` は、`register_manifest_rejects_*` を意味します。
- エイリアス エイリアス エイリアス `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON は、 للاحداث المستخدمة في خطوط مراقبة الرصد 。

## いいえ

回答 (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- エンドツーエンドのテスト (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) を実行します。

説明:
- تحداث Norito منظم لتدقيقات الحوكمة (موقع؟)。

回答:
- SLA を使用します。
- انتهاء صلاحية 別名 اقل من العتبة。
- مخالفات الاحتفاظ (マニフェスト لم يجدد قبل الانتهاء)。

概要:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` ステータス、マニフェスト、エイリアス、バックログ、SLA レイテンシ緩みのないように、スラックをチェックしてください。

## ランブック

- `docs/source/sorafs/migration_ledger.md` レジストリ。
- メッセージ: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) يغطي المقاييس والتنبيه والنشر والنسخ الاحتياطي واستعادةああ。
- ニュース: ニュース - ニュース、ニュース、ニュース、ニュース。
- API を使用してください (وثائق Docusaurus)。

## いいえ

1. マニフェスト検証 (ManifestValidator)。
2. انهاء مخطط Norito + قيم السياسة الافتراضية。
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. 試合の試合結果。
5. ランブック/ランブックをダウンロードしてください。

SF-4 を開発し、SF-4 を開発しました。
休息時間:

- `GET /v2/sorafs/pin` と `GET /v2/sorafs/pin/{digest}` はマニフェストを表示します
  別名 واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة。
- `GET /v2/sorafs/aliases` と `GET /v2/sorafs/replication` のエイリアス
  あなたのことを忘れないでください。

CLI を使用する (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) レジストリ بدون لمس
API を使用してください。