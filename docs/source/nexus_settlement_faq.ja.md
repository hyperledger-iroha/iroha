<!-- Japanese translation for docs/source/nexus_settlement_faq.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: draft
translator: LLM (Codex)
---

# Nexus 決済 FAQ

**ロードマップ:** NX-14 — Nexus ドキュメント & オペレーター向けランブック  
**ステータス:** 2026-03-24 初版（決済ルーター & CBDC プレイブック仕様と同期）  
**想定読者:** Nexus (Iroha 3) ローンチに向けて準備しているオペレーター、SDK 作者、ガバナンス審査担当者。

この FAQ は、NX-14 レビューで挙がった決済ルーティング、XOR 変換、テレメトリ、監査エビデンスに関する質問へ回答します。詳細仕様は `docs/source/settlement_router.md`、CBDC 固有のポリシーは `docs/source/cbdc_lane_playbook.md` を参照してください。

> **要約:** すべての決済フローは Settlement Router を経由し、公開レーンでは XOR バッファを払い出し、レーン固有の手数料を適用します。オペレーターは公開済みマニフェストと同じ内容のルーティング設定（`config/config.toml`）、テレメトリ・ダッシュボード、監査ログを維持する必要があります。

## FAQ

### どのレーンが決済を担当し、自分のデータスペースはどこに属しますか？

- 各データスペースはマニフェスト内で `settlement_handle` を宣言します。デフォルトの割り当ては以下の通りです。
  - `xor_global`: デフォルトの公開レーン。
  - `xor_lane_weighted`: 独自の流動性を使う公開カスタムレーン。
  - `xor_hosted_custody`: プライベート/CBDC レーン（エスクロー型 XOR バッファ）。
  - `xor_dual_fund`: シールド + 公開フローを混在させるハイブリッド/機密レーン。
- レーンクラスの解説は `docs/source/nexus_lanes.md`、最新のカタログ承認は `docs/source/project_tracker/nexus_config_deltas/*.md` を参照してください。`irohad --sora --config … --trace-config` を実行すると、監査用に実行時のカタログを確認できます。

### Settlement Router はどのように換算レートを決めますか？

- ルーターは単一の経路で決定論的な価格を適用します。
  - 公開レーンではオンチェーンの XOR 流動性プール（公開 DEX）を利用し、流動性が薄い場合はガバナンス承認済みの TWAP にフォールバックします。
  - プライベートレーンは事前に XOR バッファへ資金を積み立てます。決済を引き落とす際、ルーターは `{lane_id, source_token, xor_amount, haircut}` を記録し、バッファが逸脱した場合は `haircut.rs` に定義されたヘアカットを適用します。
- 設定値は `config/config.toml` の `[settlement]` セクションに格納されています。ガバナンス指示がない限り独自変更は避けてください。各フィールドは `docs/source/settlement_router.md` で説明されています。

### 手数料とリベートはどう適用されますか？

- 手数料はマニフェスト内でレーンごとに定義します。
  - `base_fee_bps`: すべての決済デビットに適用される基本手数料。
  - `liquidity_haircut_bps`: 共有流動性プロバイダーへの補償。
  - `rebate_policy`: 任意。例: CBDC プロモーションリベート。
- ルーターは手数料の内訳を含む `SettlementApplied` イベント（Norito 形式）を発行し、SDK や監査人が台帳エントリーを突き合わせられるようにします。

### 健全な決済を証明するテレメトリは？

- Prometheus 指標（`iroha_telemetry` と settlement router がエクスポート）:
  - `nexus_settlement_latency_seconds{lane_id}` — 公開レーンでは P99 < 900 ms、プライベートレーンでは < 1200 ms。
  - `settlement_router_conversion_total{source_token}` — トークンごとの換算ボリューム。
  - `settlement_router_haircut_total{lane_id}` — ガバナンスメモのない非ゼロ値は即時アラート。
- `lane_settlement_commitments[*].swap_metadata.volatility_class` で Stable/Elevated/Dislocated のどれを使ったかを確認できます。Elevated/Dislocated が記録された場合は、必ずインシデントログやガバナンス注記と突き合わせてください。
- ダッシュボード: `dashboards/grafana/nexus_settlement.json` と `nexus_lanes.json`。アラート定義は `dashboards/alerts/nexus_audit_rules.yml` にあります。
- テレメトリが劣化した場合は `docs/source/nexus_operations.md` のランブックに従ってインシデントを記録してください。

### 監査人が求めるエビデンスは？

1. **設定スナップショット** — `[settlement]` セクションを含む `config/config.toml` と、現在のマニフェストが参照するレーンカタログ。
2. **ルーターログ** — `settlement_router.log` を日次でアーカイブ。ハッシュ化された決済 ID、XOR デビット、ヘアカット適用箇所が含まれます。
3. **テレメトリエクスポート** — 上記メトリクスの週次スナップショット。
4. **突合レポート** — 推奨: `SettlementRecordV1` エントリー（`docs/source/cbdc_lane_playbook.md` 参照）をエクスポートし、財務台帳と比較。

### SDK 側で特別な対応は必要ですか？

- SDK は以下を提供する必要があります。
  - ` /v1/settlement/records` から決済イベントを取得するヘルパーと `SettlementApplied` ログの解釈。
  - クライアント設定でレーン ID と settlement handle を公開し、オペレーターが正しくルーティングできるようにする。
  - `docs/source/settlement_router.md` で定義された Norito ペイロード（`SettlementInstructionV1` など）を実装し、エンドツーエンドテストを用意。
- Nexus SDK クイックスタートには各言語向けのオンボーディングスニペットがあります。

### 決済はガバナンスや緊急ブレーキとどう連携しますか？

- ガバナンスはマニフェスト更新で特定の settlement handle を停止できます。ルーターは `paused` フラグを尊重し、新規決済を `ERR_SETTLEMENT_PAUSED` で拒否します。
- 緊急の「ヘアカット制限」は共用バッファの枯渇を防ぐため、ブロックごとの最大 XOR デビットを制御します。
- オペレーターは `governance.settlement_pause_total` を監視し、`docs/source/nexus_operations.md` のインシデントテンプレートに従って対応してください。

### バグ報告や変更リクエストはどこへ？

- 機能差分 → `NX-14` タグ付き issue を作成し、ロードマップへリンク。
- 緊急の決済インシデント → Nexus プライマリ（`docs/source/nexus_operations.md` 参照）へページし、ルーターログを添付。
- ドキュメント修正 → 本ファイルとポータル版（`docs/portal/docs/nexus/overview.md` など）への PR を提出。

### 代表的な決済フロー例を教えてください

以下の例では、最も一般的なレーン種別について、ルーターログ、台帳ハッシュ、テレメトリの紐付け方を示します。

#### プライベート CBDC レーン（`xor_hosted_custody`）

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

ルーターログ、台帳トランザクションハッシュ、テレメトリスナップショットを同じバンドルに収集してください。続く例では公開レーンとハイブリッドレーンの証跡を示します。

#### 公開レーン（`xor_global`）

公開データスペースは `xor_global` を経由し、共有 DEX バッファを消費します。TWAP にフォールバックした場合は、そのハッシュまたはガバナンスノートも添付してください。

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

TWAP 記録、ルーターログ、テレメトリ、台帳ハッシュを同じ証跡にまとめ、レーン0のレイテンシや TWAP 新鮮度アラートと紐付けます。

#### ハイブリッド/機密レーン（`xor_dual_fund`）

ハイブリッドレーンはシールドバッファと公開 XOR を組み合わせます。各決済では、どちらのバケットが XOR を供給したか、ヘアカットがどう分配されたかを示してください。

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

ルーターログに加え、デュアルファンドポリシー（ガバナンスカタログ抜粋）、該当レーンの `SettlementRecordV1` エクスポート、テレメトリを保存し、シールド/公開の配分がガバナンス制限に従ったことを証明してください。

決済ルーターの挙動やレーンポリシーが変わった場合は、本 FAQ とポータル版を更新してください。
