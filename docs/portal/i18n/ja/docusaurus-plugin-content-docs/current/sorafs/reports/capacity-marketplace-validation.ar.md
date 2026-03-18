---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: التحقق من سوق سعة SoraFS
タグ: [SF-2c、受け入れ、チェックリスト]
要約: قائمة تحقق للقبول تغطي انضمام المزودين، تدفقات النزاعات، وتسوية الخزانة التي تضبط SoraFS を参照してください。
---

# قائمة تحقق التحقق من سوق سعة SoraFS

** 日:** 2026-03-18 -> 2026-03-24  
** 対応:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**النطاق:** مسارات انضمام المزودين، تدفقات تحكيم النزاعات، وعمليات تسوية الخزانة المطلوبة لـ SF-2c GA。

最高のパフォーマンスを見せてください。テストはテスト (テストはフィクスチャのテスト) で行われます。

## قائمة تحقق القبول

### और देखें

|ああ |ああ |意味 |
|------|-----------|----------|
|レジストリ إعلانات السعة القياسية | `/v1/sorafs/capacity/declare` アプリ API とメタデータレジストリを確認してください。 | `crates/iroha_torii/src/routing.rs:7654` |
|スマート コントラクト ペイロード ペイロード | スマート コントラクト ペイロードضمن اختبار وحدات أن معرفات المزود وحقول GiB الملتزم بها تطابق الإعلان الموقع قبل الحفظ。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI アーティファクト セキュリティ | CLI ハーネス Norito/JSON/Base64 の往復数の計算オフラインです。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|ログイン して翻訳を追加するポリシーのデフォルト設定を確認してください。 | `../storage-capacity-marketplace.md` |

### 翻訳

|ああ |ああ |意味 |
|------|-----------|----------|
|ペイロードをダイジェストで表示する | ペイロードを表示する保留中の元帳のペイロード。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI يطابق المخطط القياسي | CLI の Base64/Norito JSON 形式 `CapacityDisputeV1` の証拠バンドルを確認します。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|再生 يثبت حتمية النزاع/العقوبة |テレメトリー 証明失敗証明 スナップショット 台帳 台帳 スラッシュ マーク仲間たち。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
|ランブック ランブック | ランブックロールバックを実行してください。 | `../dispute-revocation-runbook.md` |

### 翻訳

|ああ |ああ |意味 |
|------|-----------|----------|
|台帳 يطابق توقع 浸漬 لمدة 30 يوما | 30 年以内に決済を完了する前に、元帳を確認してください。すごい。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|台帳を作成する | يقارن `capacity_reconcile.py` 手数料台帳 بصادرات تحويل XOR المنفذة، ويصدر مقاييس Prometheus، ويضبط موافقةアラートマネージャー。 | `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|請求とテレメトリの管理 | يعرض استيراد Grafana تراكم GiB-hour، عدادات Strikes، والضمان المربوط لتمكين الرؤية لدى فريقああ。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|ぬるぬるリプレイ | ビデオゲームフックをしっかりと浸してください。 | `./sf2c-capacity-soak.md` |

## 大事な

承認:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

ペイロード ペイロード `sorafs_manifest_stub capacity {declaration,dispute}` وأرشفة بايتات JSON/Norito は、次のとおりです。

## アーティファクト

|アーティファクト |パス |ブレイク2b-256 |
|----------|------|---------------|
| حزمة موافقة انضمام المزودين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة موافقة تسوية النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة تسوية الخزانة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

アーティファクトを確認するために、アーティファクトを確認してください。

## ありがとう

- ストレージ チーム リーダー — @storage-tl (2026-03-24)  
- ガバナンス評議会書記 — @council-sec (2026-03-24)  
- 財務業務責任者 — @treasury-ops (2026-03-24)