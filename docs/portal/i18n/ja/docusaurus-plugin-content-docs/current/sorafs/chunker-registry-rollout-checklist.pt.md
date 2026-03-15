---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: SoraFS のチャンカーを登録するロールアウトのチェックリスト
Sidebar_label: チャンカーのロールアウトのチェックリスト
説明: チャンカーの登録を行うための、ロールアウトの計画。
---

:::note フォンテ カノニカ
リフレテ`docs/source/sorafs/chunker_registry_rollout_checklist.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# SoraFS を登録するロールアウトのチェックリスト

エステ チェックリストは、プロモーション担当者と新しい情報を取得するために必要な情報を取得します。
認可証書と証明証書と改訂証書を一括して発行します。
批准のための統治。

> **Escopo:** 修正機能をリリースするためのアプリケーションを作成します
> `sorafs_manifest::chunker_registry`、プロバイダー入場エンベロープ、ou バンドル
> フィクスチャ canonicos (`fixtures/sorafs_chunker/*`)。

## 1. バリダカオ飛行前

1. フィクスチャを再生成し、決定性を検証します:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. クエリのハッシュの決定性を確認する
   `docs/source/sorafs/reports/sf1_determinism.md` (パフォーマンスに関する関係
   関連するもの）batem com os artefatos regenerados。
3. ガランタ クエリ `sorafs_manifest::chunker_registry` コンピラ コム
   `ensure_charter_compliance()` 実行時:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 提案書を作成する:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atas do conselho em `docs/source/sorafs/council_minutes_*.md`
   - 決定論との関係

## 2. 統治の承認

1. ツーリング ワーキング グループの関係者への報告、要約、提案
   ソラ議会インフラパネル。
2. 承認に関する登録
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. パブリック・オー・エンベロープ・アッシナド・ペロ・パルラメント・ジュント・コム・オス・フィクスチャー：
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. ガバナンス フェッチ ヘルパー経由でエンベロープのアクセスを確認します。
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージングをロールアウトする

[マニフェスト ステージングのプレイブック](./staging-manifest-playbook) パラメータを参照してください。
パッソ・ア・パッソ・デタルハド。

1. Torii com Discovery `torii.sorafs` ハビリタドおよびアドミッション強制の埋め込み
   リガド (`enforce_admission = true`)。
2. Envie OS プロバイダーのレジストリの承認エンベロープ
   `torii.sorafs.discovery.admission.envelopes_dir` のステージング参照。
3. Verifique キュー プロバイダーは、API de Discovery を介してプロパガムを宣伝します。
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. ガバナンスのマニフェスト/プラン com ヘッダーのエンドポイントを実行します。
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. テレメトリのダッシュボード (`torii_sorafs_*`) のアラート記録を確認します。
   新たにエラーを報告します。

## 4. プロデュースを展開する

1. ステージング ノード Torii を製造します。
2. ジャネラ デ アティバカオ (データ/時間、猶予期間、ロールバック計画) 番号を通知します。
   SDK とオペラドールの操作。
3. PR とリリースのコンテンツをマージします。
   - 備品と封筒の調整
   - Mudancas na documentacao (憲章の参照、決定論の関係)
   - ロードマップ/ステータスを更新
4. タギーエは、出自を明らかにするリリースです。

## 5. オーディトリアのposロールアウト

1. 最終的な指標の取得 (検出数、取得に成功した分類群、ヒストグラム)
   デエロ) 24 時間後ロールアウト。
2. `status.md` comum resumo curto e link para o relatorio determinismoを実現します。
3. tarefas de acompanhamento の登録 (例: orientacao adicional para authoring)
   デパーフィス）em `roadmap.md`。