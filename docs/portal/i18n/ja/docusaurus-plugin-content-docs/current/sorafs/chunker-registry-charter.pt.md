---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: SoraFS のチャンカーの登録情報
Sidebar_label: チャンカーの登録情報
説明: チャンカーの提出と承認に関する政府のカルタ。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/chunker_registry_charter.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# SoraFS のチャンカー登録の管理カード

> **Ratificado:** 2025-10-29 pelo Sora 議会インフラパネル (veja)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。 Qualquer emenda requer um
> 正式な統治に投票する。実装ツールを開発し、トラタール エステート ドキュメントを作成します。
> ノルマティボは、ケ ウマ カルタの代替品を食べました。

Esta carta は、SoraFS の処理プロセスまたはチャンカーの登録を定義しています。
Ela 補完 [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) 新規のディスクレーバー
事前、見直し、批准、そして最終的な継続の停止を完了します。

## エスコポ

情報をアプリケーションに入力し、`sorafs_manifest::chunker_registry` e
qualquer ツール que consome or registro (マニフェスト CLI、プロバイダー広告 CLI、
SDK)。エイリアスとハンドルの検証は不変です
`chunker_registry::ensure_charter_compliance()`:

- ID は、単調な形式を維持するための確実な情報を提供します。
- O ハンドル canonico `namespace.name@semver` **deve** aparecer como a primeira
  エントラーダ em `profile_aliases`。別名alternativos vem em seguida。
- エイリアス sao aparadas の文字列として、unicas e nao coridem com が canonicos を処理します
  デ・アウトラス・エントラダス。

## パペイス

- **Autor(es)** - プロポスタの準備、フィクスチャの再生成、およびコレクタの作成
  決定論の証拠。
- **ツーリング ワーキング グループ (TWG)** - 提案書をチェックリストとして検証します
  公開された情報は、常に登録されているかどうかを確認します。
- **ガバナンス評議会 (GC)** - TWG に関する改訂、提案書の提出
  パブリックカオ/非プレカカオの認可を取得します。
- **ストレージ チーム** - レジストリ公開の実装を管理します
  ドキュメントの実物。

## Fluxo do ciclo de vida

1. **提案提出**
   - 自動検証チェックリストを実行する責任者
     うーん、JSON `ChunkerProfileProposalV1` すすり泣く
     `docs/source/sorafs/proposals/`。
   - CLI で実行する内容を含めます:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - PR コンテントの設備、提案、決定性の関係を羨望のまなざし
     atualizacoes は登録を行います。

2. **ツーリングの見直し (TWG)**
   - 検証チェックリストを再作成します (フィクスチャ、ファズ、マニフェスト/PoR のパイプライン)。
   - `cargo test -p sorafs_car --chunker-registry` e garanta que を実行します。
     `ensure_charter_compliance()` は新しいエントリを受け取ります。
   - CLI の互換性を確認します (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) reflita os エイリアスは atualizados を処理します。
   - 承認/報告のステータスを正確に報告します。

3. **アプロバカオ ド コンセーリョ (GC)**
   - TWG に関する関連性を修正し、提案されたメタデータを修正します。
   - 提案された内容の要約 (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     AOS フィクスチャを組み合わせたエンベロープを付属しています。
   - 統治上の結果を登録します。

4. **パブリカオ**
   - Faca マージによる PR、アトゥアリザンド:
     - `sorafs_manifest::chunker_registry_data`。
     - Documentacao (`chunker_registry.md`、guias de autoria/conformidade)。
     - 決定性と関係性。
   - オペラドールが SDK を装備するか、新しいパフォーマンスやプレーンジャドのロールアウトを通知します。

5. **デプレカカオ / エンセラメント**
   - Propostas que substituem um perfilexistente devem incluir uma janela de publicacao
     デュプラ (ピリオドス デ カレンシア) ウム プランノ デ アップグレード。
     登録や移行元帳はありません。

6. **緊急事態宣言**
   - ホットフィックスを削除して、マイオリアのコンセルホ コムを実行してください。
   - O TWG は、事故の危険性を考慮した記録としてドキュメンタリーを開発しています。

## ツールへの期待

- `sorafs_manifest_chunk_store` と `sorafs_manifest_stub` の説明:
  - `--list-profiles` はレジストリを検査します。
  - `--promote-profile=<handle>` メタダドス メタ ジェラール オブ ブロコ
    アオプロムーバーウムパーフィル。
  - `--json-out=-` 標準出力の送信関連、改訂ログの閲覧
    再現です。
- `ensure_charter_compliance()` の初期化と関連するバイナリの呼び出し
  (`manifest_chunk_store`、`provider_advert_stub`)。 CI 開発のオスの精巣
  novas entradas violarem a carta。

## レジストロ

- Armazene の決定的な関係性は `docs/source/sorafs/reports/` です。
- 調査結果を参照して決定するように
  `docs/source/sorafs/migration_ledger.md`。
- `roadmap.md` と `status.md` をレジストリなしで取得します。

## 参考資料- オートリアのギア: [チャンカーのオートリアのギア](./chunker-profile-authoring.md)
- 適合チェックリスト: `docs/source/sorafs/chunker_conformance.md`
- 登録参照: [チャンカーの権限登録](./chunker-registry.md)