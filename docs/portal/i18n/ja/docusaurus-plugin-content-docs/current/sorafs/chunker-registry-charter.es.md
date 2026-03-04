---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: SoraFS のチャンカーの登録情報
サイドバーラベル: チャンカーの登録情報
説明: チャンカーのプレゼンタシオンとパーファイルのアプロバシオンに関するカルタ デ ゴベルナンザ。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/chunker_registry_charter.md` のページを参照してください。スフィンクスの記録を保存し、記録を保存する必要があります。
:::

# SoraFS チャンカーの登録カルタ

> **Ratificado:** 2025-10-29 por el Sora 議会インフラパネル (ver)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。 Cualquier enmienda requiere un
> 投票デゴベルナンザ正式;ロス・エキポス・デ・インプリメント・デベン・トラタール・エステ・ドキュメント・コモ
> ノルマティボ・ハスタ・ケ・セ・アプルーベ・ウナ・カルタ・ケ・ロ・ススティティア。

SoraFS の進化したチャンカーのレジストリでの役割の手順を定義します。
[チャンカーの自動ファイル](./chunker-profile-authoring.md) の新しい説明を補完します。
提案、改訂、批准、最終的な減価償却を可能にします。

## アルカンス

`sorafs_manifest::chunker_registry` y の情報を受け取る
登録用のより適切なツール (マニフェストの CLI、プロバイダー広告の CLI、
SDK)。エイリアスの不変性を無視して検証を処理します
`chunker_registry::ensure_charter_compliance()`:

- 物事を正確に把握するために必要な ID を失います。
- El ハンドル canónico `namespace.name@semver` **debe** aparecer como primera
  エントラーダ en `profile_aliases`。シグエン・ロス・エイリアス・ヘレダドス。
- Las cadenas de alias se recortan、息子 únicas と no colisionan con ハンドル canónicos
  デ・オトラス・エントラダス。

## 役割

- **作成者** – プロプエスタの準備、フィクスチャの再生成、および再コピー
  決定論の証拠。
- **ツーリング ワーキング グループ (TWG)** – 有効性を確認するためのチェックリスト
  公開計画は不変です。
- **ガバナンス評議会 (GC)** – TWG 報告書改訂、法律事務所
  公開/非推奨の廃止。
- **ストレージ チーム** – 登録および公開の実装を担当
  実際のドキュメント。

## フルホ デル シクロ デ ヴィーダ

1. **プロプエスタのプレゼンテーション**
   - 自動作成の検証チェックリストの自動出力
     JSON `ChunkerProfileProposalV1` ja
     `docs/source/sorafs/proposals/`。
   - CLI のサリダを含む:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - コンテンガ フィクスチャ、プロプエスタ、決定レポートの作成を支援
     実際の登録。

2. **ツールの改訂 (TWG)**
   - 検証チェックリスト (フィクスチャ、ファズ、マニフェスト/PoR のパイプライン) を再現します。
   - エジェクタ `cargo test -p sorafs_car --chunker-registry` y アセグラ ケ
     `ensure_charter_compliance()` 新しいエントリを見つけてください。
   - CLI のコンポーネントの検証 (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) refleje los alias y は、actualizados を処理します。
   - 報告書を作成し、報告書を作成します。

3. **アプロバシオン デル コンセホ (GC)**
   - TWG のレポートとプロプエスタのメタデータの改訂。
   - プロプエスタ企業ダイジェスト (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     ロスの試合の準備をしっかりと整えてください。
   - 知事の行為に関する登録。

4. **出版**
   - Fusiona el PR、実際の担当者:
     - `sorafs_manifest::chunker_registry_data`。
     - Documentación (`chunker_registry.md`、guías de autoría/conformidad)。
     - 決定的な予定とレポート。
   - オペラドールと装備の SDK の新しい実行とロールアウト計画を通知します。

5. **非推奨/レティーロ**
   - 公共の場での活動を含む、存在する権利を維持するための手続き
     二重 (期間) と現実化の計画。
     登録と移行の台帳。

6. **緊急事態宣言**
   - 市長の不正行為に対するホットフィックスの削除が必要です。
   - TWG は、事件の記録と実際の記録を記録します。

## ツールへの期待

- `sorafs_manifest_chunk_store` y `sorafs_manifest_stub` 指数:
  - `--list-profiles` 登録パラ検査。
  - `--promote-profile=<handle>` メタデータの一般的なブロック
    アルプロムーバーアンパーフィル。
  - `--json-out=-` パラ送信が標準出力、改訂ログを報告します
    再現可能。
- `ensure_charter_compliance()` 関連するバイナリを呼び出します
  (`manifest_chunk_store`、`provider_advert_stub`)。ラス プルエバス CI デベン フォールラー SI
  ヌエバス・エントラーダス・ヴィオラン・ラ・カルタ。

## レジストロ- `docs/source/sorafs/reports/` に関する決定論的な報告を保護します。
- チャンカーの生存に関する参照者の決定に対する行動
  `docs/source/sorafs/migration_ledger.md`。
- Actualiza `roadmap.md` y `status.md` después de cada cambio 市長 del registro。

## 参考資料

- 自動ギア: [チャンカーのパーファイル自動ギア](./chunker-profile-authoring.md)
- 適合チェックリスト: `docs/source/sorafs/chunker_conformance.md`
- 登録参照: [チャンカーのパーファイル登録](./chunker-registry.md)