---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: チャンカーの登録ロールアウトのチェックリスト SoraFS
Sidebar_label: チェックリスト ロールアウト チャンカー
説明: 登録チャンカーのロールアウトを計画します。
---

:::note ソースカノニク
リフレット`docs/source/sorafs/chunker_registry_rollout_checklist.md`。 Gardez les deux は、スフィンクスのヘリテージ セットを完全に再現したものをコピーします。
:::

# 登録ロールアウトのチェックリスト SoraFS

Cette チェックリストの詳細を確認し、必要なプロムヴォワールとヌーボー プロファイルを作成する
制作後のレビューを含む、4 回の入場料を一括で支払う
統治憲章の批准。

> **Portée :** S'applique à toutes les releases qui modifient
> `sorafs_manifest::chunker_registry`、入学許可証の封筒
> 正規のフィクスチャのバンドル (`fixtures/sorafs_chunker/*`)。

## 1. 事前の検証

1. 治具の再生成と決定の検証:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 決定性を確認する
   `docs/source/sorafs/reports/sf1_determinism.md` (プロファイルの関係
   関連) 特派員 aux artefacts régénérés。
3. Assurez-vous que `sorafs_manifest::chunker_registry` コンパイル avec
   `ensure_charter_compliance()` 日本語:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 提案に関するメッテズ・ア・ジュール・ドシエ：
   - `docs/source/sorafs/proposals/<profile>.json`
   - コンセイユ・スーのアントレ・デ・ミニッツ `docs/source/sorafs/council_minutes_*.md`
   - 決定主義の関係

## 2. 検証ガバナンス

1. ツーリングワーキンググループの関係と提案の要約
   auそら議会インフラパネル。
2. 承認の詳細を登録する
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. Publiez l'envelope Signée par le Parlement à côté des fixtures :
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. le helper de fetch de gouvernance 経由でアクセス可能な封筒の確認:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ロールアウトのステージング

Référez-vous au [プレイブック マニフェスト ステージング](./staging-manifest-playbook) を注ぎます
手順の詳細。

1. Torii のアベックディスカバリ `torii.sorafs` のアクティブ化とアプリケーションの展開
   入学活動 (`enforce_admission = true`)。
2. レパートリーの許可を承認する封筒の申請
   ステージング参照の登録は `torii.sorafs.discovery.admission.envelopes_dir` です。
3. プロバイダーが API ディスカバリー経由で宣伝者を宣伝することを確認します。
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. エンドポイントの管理上のヘッダーのマニフェスト/計画の実行:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. ダッシュボードのテレメトリー (`torii_sorafs_*`) と規則の確認
   間違いのない新しいレポートを作成します。

## 4. ロールアウト生産

1. 本番環境でのステージングの繰り返し Torii。
2. Annoncez la fenêtre d'activation (日付/時間、猶予期間、ロールバック計画)
   aux canaux オペレーターと SDK。
3. リリース コンテンツの PR をマージします。
   - 備品と封筒のミス・ア・ジュール
   - 文書の変更 (チャートの参照、決定性の確認)
   - ロードマップ/ステータスを更新する
4. 出所を示す資料のリリースとアーカイブをタグ付けします。

## 5. ロールアウト後の監査

1. Capturez les métriques Finales (発見の完了、成功のフェッチ、
   ヒストグラム エラー) ロールアウト後 24 時間。
2. Mettez à jour `status.md` avec un bref resumé et un lien vers le rapport de determinisme。
3. Consignez les tâches de suivi (例: プロファイルの作成ガイダンス)
   `roadmap.md`。