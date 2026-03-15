---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: SoraFS チャンカーのロールアウト登録チェックリスト
Sidebar_label: チャンカーのロールアウトのチェックリスト
説明: チャンカーの登録を実際に展開する計画。
---

:::メモ フエンテ カノニカ
リフレジャ`docs/source/sorafs/chunker_registry_rollout_checklist.md`。スフィンクスの記録を保存し、記録を保存する必要があります。
:::

# SoraFS のロールアウト登録チェックリスト

エステチェックリストは、プロモーターと新しいチャンカーの必要性を把握します
審査の承認と承認を一括して改訂を行います。
ハヤ・シド・ラティフィカダ・ゴベルナンザ・カルタ・デ・ゴベルナンザ。

> **Alcance:** Aplica a todas las release que modifican
> `sorafs_manifest::chunker_registry`、ロス・ソブレス・デ・アドミシオン・デ・証明オーロス
> フィクスチャのバンドル CANONICOS (`fixtures/sorafs_chunker/*`)。

## 1. 事前の検証

1. フィクスチャと決定性の検証を再生成する:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 確定性のあるハッシュを確認する
   `docs/source/sorafs/reports/sf1_determinism.md` (パフォーマンスに関するレポート)
   偶然、ロス・アーティファクトス・レジェネラドスが発生した。
3. アセグラクエリ `sorafs_manifest::chunker_registry` コンパイル
   `ensure_charter_compliance()` 出力:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 申請書類の作成:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de actas del consejo en `docs/source/sorafs/council_minutes_*.md`
   - 決定論報告書

## 2. ゴベルナンサの乱用

1. ツーリング ワーキング グループの情報と提案のダイジェスト
   ソラ議会インフラパネル。
2. 不正行為登録
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. ロスの試合日程を公開する:
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. 取得ヘルパー経由で海にアクセスできるかどうかを確認します。
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージングによるロールアウト

[ステージングのためのマニフェストのプレイブック](./staging-manifest-playbook) を参照してください。
レコリード・デタラード・デ・エストス・パソス。

1. Torii 発見 `torii.sorafs` のアプリケーションの実現
   有効化許可 (`enforce_admission = true`)。
2. 登録管理者権限を取得する
   `torii.sorafs.discovery.admission.envelopes_dir` のステージング参照。
3. API を介してプロバイダーの広告を確認し、発見します。
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. マニフェスト/プランのヘッダーのエンドポイントの取り出し:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. テレメトリのダッシュボード (`torii_sorafs_*`) と現在の規制を確認します。
   新しいエラーを報告します。

## 4. 本番環境でのロールアウト

1. ステージング コントラ ロス ノード Torii の製造を繰り返します。
2. アクティバシオンの通知 (フェチャ/ホーラ、グラシアの期間、ロールバックの計画)
   ロス・カナレス・デ・オペラドールSDK。
3. リリース後の PR の融合:
   - 備品と地味な現実
   - Cambios de documentación (アラカルタ参照、決定報告書)
   - ロードマップ/ステータスを更新
4. 手続きに関する公開およびアーカイブの手順。

## 5. ロールアウト後のオーディトリア

1. Captura métricas フィナーレ (発見のコンテオ、エキシトのフェッチ、
   エラーのヒストグラム) 24 時間のロールアウト。
2. Actualiza `status.md` は、決定性のある完全なレポートを再開します。
3. Registra cualquier Tarea de seguimiento (p. ej.、más guía de autoría de perfiles)
   ja `roadmap.md`。