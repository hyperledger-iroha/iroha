---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: 登録チャンカー SoraFS
Sidebar_label: チャート レジストリ チャンカー
説明: 申請とプロファイルの承認を求める統治憲章。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/chunker_registry_charter.md` を参照します。 Gardez les deux は、スフィンクスのヘリテージ セットを完全に再現したものをコピーします。
:::

# 登録チャンカー SoraFS の統治権限

> **批准者 :** 2025-10-29 ソラ議会インフラパネル (投票)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。 Tout 修正 exige un
> 正式な統治に投票する。反逆者の実装に関するドキュメントの実装
> 再配置の承認の基準。

SoraFS のプロセスと登録チャンカーのルールを決定します。
Elle complète le [プロファイル チャンカー作成ガイド](./chunker-profile-authoring.md) の新しいコメント
プロフィール、提案、承認、批准、最終決定の廃止。

## ポルテ

`sorafs_manifest::chunker_registry` et のチャート アップリケ、チャック エントレ
登録ツール (マニフェスト CLI、プロバイダー広告 CLI、
SDK)。エルは、不変条件を別名とハンドルの検証に適用します
`chunker_registry::ensure_charter_compliance()` :

- モノトーンの機能を強化するためのプロファイルを作成します。
- Le handle canonique `namespace.name@semver` **doit** apparaître en première
- さまざまな問題、ユニークな問題、および衝突問題を解決します
  canoniques d'autres のメインディッシュ。

## ロール

- **作者** – 提案の準備、備品およびコレクションの保存
  決定主義の準備。
- **ツーリング ワーキング グループ (TWG)** – チェックリストに関する提案の有効性
  出版者は、登録上の不変性を尊重します。
- **ガバナンス評議会 (GC)** – TWG の関係、提案の署名を検討する
  出版物/減価償却のカレンダーを承認します。
- **ストレージ チーム** – 登録および公開の実装を保守します
  レ・ミス・ア・ジュール・ド・ドキュメンテーション。

## 流動サイクルの変動

1. **提案の提案**
   - L'auteur exécute la checklist de validation du guide d'auteur et crée
     un JSON `ChunkerProfileProposalV1` ソース
     `docs/source/sorafs/proposals/`。
   - 出撃時の CLI を含める:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - さまざまな PR コンテンツの備品、提案、決定主義などの関係
     登録ミス。

2. **レビューツール (TWG)**
   - 検証のチェックリストを作成します (フィクスチャ、ファズ、パイプライン マニフェスト/PoR)。
   - 実行者 `cargo test -p sorafs_car --chunker-registry` と検証者
     `ensure_charter_compliance()` 最高の新人料理です。
   - CLI の検証機能 (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) 別名を参照し、不在時の対応を行います。
   - 法廷関係の履歴書および統計の合否を判断します。

3. **承認デュコンセイユ (GC)**
   - TWG と提案のメタドンの関係を審査する者。
   - 提案の署名者ファイル (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     そして、備品の管理を維持するための署名を管理します。
   - 管理者の投票結果の管理者。

4. **出版物**
   - 1 時間ごとに PR を行うフュージョンナー :
     - `sorafs_manifest::chunker_registry_data`。
     - ドキュメント (`chunker_registry.md`、規格ガイド/適合ガイド)。
     - 決定性と関係性。
   - 通知者は、新しいプロファイルとロールアウト前の SDK の操作と準備を行います。

5. **減価償却/買戻し**
   - 出版物の公開を含むプロファイルの存在に関する提案は存在しません
     ダブル (期間延長) とアップグレードの計画。
   - 期限切れ後のフェネートル ドゥ グレース、マルケ ル プロファイル コンメ ドプレシエ
     移行の登録および管理。

6. **緊急の変化**
   - 多数決での投票を中止し、ホットフィックスを早急に削除してください。
   - Le TWG は、事件の危険性を軽減するための記録を作成します。

## アテントツール

- `sorafs_manifest_chunk_store` および `sorafs_manifest_stub` 暴露:
  - `--list-profiles` 登録検査を行ってください。
  - `--promote-profile=<handle>` は、ブロック ド メタドンネの一般的な使用法を提供します
    プロモーションとプロフィール。
  - `--json-out=-` ストリーマーの関係と標準出力、レビューのログの永続化
    再生産可能。
- `ensure_charter_compliance()` 問題に関する問題を解決するための呼び出し
  (`manifest_chunk_store`、`provider_advert_stub`)。 CI のテストをテストします。
  de nouvelles entrées 暴力的な la charte。

## 登録- ストッカーは、`docs/source/sorafs/reports/` に関する決定的な関係を示します。
- Les minutes du conseil référant aux décisions chunker vivent sous
  `docs/source/sorafs/migration_ledger.md`。
- 不可抗力の登録変更後の法定期間 `roadmap.md` および `status.md`。

## 参照

- 作成ガイド: [プロファイルチャンカー作成ガイド](./chunker-profile-authoring.md)
- 適合チェックリスト: `docs/source/sorafs/chunker_conformance.md`
- 登録参照: [プロファイルチャンカーの登録](./chunker-registry.md)