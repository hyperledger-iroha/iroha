---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
title: SoraFS کپیسٹی سمیولیشن رَن بُک
サイドバーラベル: ੩پیسٹی سمیولیشن رَن بُک
説明: 再現可能な治具、Prometheus のエクスポート、Grafana ダッシュボード、SF-2c の接続、および接続ありがとうございます
---

:::注意事項
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔スフィンクス دستاویزی مجموعہ مکمل طور پر منتقل نہیں ہو جاتا دونوں نقول کو ہم آہنگうわー
:::

یہ رن بُک وضاحت کرتی ہہ کہ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن کِٹ کیسے چلائیں حاصل شدہ میٹرکس کیسے دیکھیں۔ `docs/examples/sorafs_capacity_simulation/` 決定論的フィクスチャ 割り当て クォータ フェールオーバー スラッシング修復 エンドツーエンド ویلیڈیٹありがとうございますペイロード数 `sorafs_manifest_stub capacity` ペイロード ہیں؛マニフェスト/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI の فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh``sorafs_manifest_stub capacity` ペイロード Norito ペイロード Base64 BLOB Torii リクエストボディ JSON サマリー説明:

- クォータの制限、制限、プロバイダーの宣言、
- レプリケーション順序とステージングされたマニフェストとプロバイダーの管理
- 停止前のベースライン、停止間隔、フェールオーバー回復、テレメトリ スナップショット
- 停止のシミュレート、スラッシュ、紛争ペイロードのシミュレート

تمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. 指標の指標

```bash
./analyze.py --artifacts ./artifacts
```

日付:

- `capacity_simulation_report.json` - 割り当て、フェイルオーバー デルタ、紛争メタデータ
- `capacity_simulation.prom` - Prometheus テキストファイル メトリック (`sorafs_simulation_*`) ノード エクスポーター テキストファイル コレクター スタンドアロン スクレイピング ジョブ

Prometheus スクレイピング構成の説明:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

テキストファイル コレクター کو `capacity_simulation.prom` کی طرف پوائنٹ کریں (ノード エクスポーター استعمال کریں تو اسے `--collector.textfile.directory` والی ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ امپورٹ کریں

1. Grafana میں `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. `Prometheus` データソース ویری ایبل کو اوپر دیے گئے ターゲットをスクレイピング سے جوڑیں۔
3. 説明:
   - **クォータ割り当て (GiB)** プロバイダーのコミット済み/割り当て済み残高 ہے۔
   - **フェイルオーバー トリガー** 停止メトリクス *フェイルオーバー アクティブ* ہو جاتا ہے۔
   - **停止中の稼働時間の低下** プロバイダー `alpha` کے لیے فیصدی نقصان دکھاتا ہے۔
   - **要求されたスラッシュ パーセンテージ** 紛争の修正率 دکھاتا ہے۔

## 4. すごい

- `sorafs_simulation_quota_total_gib{scope="assigned"}` قدر `600` رہتی ہے جب تک コミット合計 >= 600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور 代替プロバイダー メトリック میں `beta` نمایاں ہوتا ہے۔
- `sorafs_simulation_slash_requested` プロバイダ `alpha` کے لیے `0.15` (15% スラッシュ) رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں تاکہ تصدیق ہو سکے کہ フィクスチャ اب بھی CLI スキーマ کے مطابق ہیں۔