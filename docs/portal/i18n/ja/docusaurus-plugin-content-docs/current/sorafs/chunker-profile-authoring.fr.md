---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: プロファイル チャンカー作成ガイド SoraFS
Sidebar_label: チャンカー作成ガイド
説明: 提案者が新しいプロファイル チャンカー SoraFS とフィクスチャを注ぐチェックリスト。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/chunker_profile_authoring.md` を参照します。 Gardez les deux は、スフィンクスのヘリテージ セットを完全に再現したものをコピーします。
:::

# プロファイル チャンカー作成ガイド SoraFS

Ce ガイドの明示的なコメントの提案者と発行者が新しいプロファイル チャンカーを注ぐ SoraFS。
RFC アーキテクチャ (SF-1) および登録の参照 (SF-2a) を完了します
具体的な修正の実行、検証のテスト、提案のモデルの実行。
カノニクの例を注いでください。
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
ドライラン協会のログなど
`docs/source/sorafs/reports/sf1_determinism.md`。

## ヴュー・ダンサンブル

登録するアカウントのプロフィール:

- CDC のパラメータとマルチハッシュ識別情報のアナウンス
  アーキテクチャ ;
- 最高の備品 (JSON Rust/Go/TS + corpora fuzz + témoins PoR) クエリ
  ツールを必要としない検証用の SDK です。
- 統治権を与えるメタドンネ プレテス (名前空間、名前、セムバー) を含める
  ロールアウトおよびフェネット操作の管理。など
- コンセイユの審査の前に、決定権を決定するためのスイートを渡します。

チェックリストを作成し、規則を尊重して提案を行ってください。

## 登録簿の確認

Avant de rédiger une proposition, verifiez qu'elle respecte la charte du registre appliquée
パー `sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- モノトーンの外観を強化するためのプロファイルを作成します。
- Le handle canonique (`namespace.name@semver`) doit apparaître dans la liste d'alias
- Aucun 別名 ne peut entrer en crash avec un autre handle canonique ni apparaître plus d'une fois。
- 別名は、ビデオやスペースを必要としないことです。

CLI ユーティリティの補助:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes maintiennent les propositions alignées avec la charte du registre et fournissent
les métadonnées canoniques necessaires auxDiscussions de gouvernance。

## メタドンネが要求するもの

|チャンピオン |説明 |例 (`sorafs.sf1@1.0.0`) |
|----------|---------------|----------------------------|
| `namespace` |プロファイルの再グループ化。 | `sorafs` |
| `name` |リベレリシブル。 | `sf1` |
| `semver` |パラメータのアンサンブルを注ぐバージョンの意味。 | `1.0.0` |
| `profile_id` |プロファイル全体のモノトーン属性を識別します。たくさんの存在を保存しておきます。 | `1` |
| `profile.min_size` |バイト単位の最小限のチャンク。 | `65536` |
| `profile.target_size` |バイト単位の長いファイル。 | `262144` |
| `profile.max_size` |バイト単位の最大長。 | `524288` |
| `profile.break_mask` |ローリング ハッシュ (16 進数) を使用してマスクを適応させます。 | `0x0000ffff` |
| `profile.polynomial` |コンスタンテ デュ ポリノーム ギア (六角)。 | `0x3da3358b4dc173` |
| `gear_seed` | 64 KiB のテーブル ギアの配信に使用されるシード。 | `sorafs-v1-gear` |
| `chunk_multihash.code` |チャンクごとにマルチハッシュを注ぐコードのダイジェスト。 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |フィクスチャーのまとめのダイジェスト。 | `13fa...c482` |
| `fixtures_root` |レパートリー相対的なコンテンツの備品レジェネレ。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | PoR 決定権を与えるシード (`splitmix64`)。 | `0xfeedbeefcafebabe` (例) |

提案とインテリアのドキュメントを作成するための装置を作成する
登録、ツール、CLI および管理の自動化に関する一般的な治具
マニュエルを回収することなく価値を確認する。 En cas de doute、execute les CLIs chunk-store et
マニフェスト avec `--json-out=-` は、メタドンの計算とレビューのメモをストリーマーに注ぎます。

### CLI と登録の連絡先

- `sorafs_manifest_chunk_store --profile=<handle>` — チャンクのリランサー、
  マニフェストのダイジェストと提案されたパラメータのチェックを行います。
- `sorafs_manifest_chunk_store --json-out=-` — ストリーマー ル ラポール チャンク ストア バージョン
  自動比較の標準出力。
- `sorafs_manifest_stub --chunker-profile=<handle>` — マニフェスト等の確認者
  CAR は、canonique および les エイリアスを処理することを計画しています。
- `sorafs_manifest_stub --plan=-` — 注入器の `chunk_fetch_specs` 優先注入
  変更後のオフセット/ダイジェストを検証します。任務を遂行するための任務 (ダイジェスト、ラシーン PoR、マニフェストのハッシュ) を提案する
レビュー担当者は、再生産を繰り返します。

## チェックリストの決定と検証

1. **備品の見直し**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **パリスイートの執行者** — `cargo test -p sorafs_chunker` およびハーネス差分
   クロスランゲージ (`crates/sorafs_chunker/tests/vectors.rs`) の言語の相互作用
   新しい備品が配置されています。
3. **コーポラのファズ/バックプレッシャーを再確認** — exécutez `cargo fuzz list` et le harness de
   ストリーミング (`fuzz/sorafs_chunker`) の資産を保存します。
4. **Vérifier les témoins Proof-of-Retrievability** — exécutez
   `sorafs_manifest_chunk_store --por-sample=<n>` プロファイルの提案と確認の結果
   ラシネス特派員、マニフェストデフィクスチャ。
5. **CI のドライラン** — invoquez `ci/check_sorafs_fixtures.sh` ロケメント ;ファイルスクリプト
   doit réussir avec les nouvelles fixtures et le `manifest_signatures.json` が存在します。
6. **クロスランタイムの確認** — Go/TS コンソメメント ファイルのバインディングを保証する
   限界を超え、同一性をダイジェストします。

ツール WG のプロジェクトに関するコマンドとダイジェストの結果を文書化します。
推測のないレジュエ。

### 確認マニフェスト/PoR

フィクスチャの再生成後、パイプライン マニフェストの保証を完了して実行します
メタドン CAR と les preuves PoR の一貫性:

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

フィクスチャを使用したコーパス表現の要素の再配置
(例、1 GiB の le flux déterministe) と、命題の結果をダイジェストします。

## 提案モデル

レコードの命題 Norito `ChunkerProfileProposalV1` の記録
`docs/source/sorafs/proposals/`。テンプレート JSON ci-dessous illustre la forme 出席者
(必要なお金を交換) :


Fournissez との関係 Markdown 特派員 (`determinism_report`) が出撃をキャプチャします
コマンド、チャンクのダイジェストと検証の相違点を調べます。

## 統治の流動性

1. **Soumettre une PR avec proposition + fixtures.** 一般資産、ラを含む
   命題 Norito と `chunker_registry_data.rs` の時間。
2. **レビューツール WG.** レビュー担当者がチェックリストの検証と確認を行う
   que la proposition respecte les règles du registre (pas de réutilisation d'id,
   決定主義は満足する）。
3. **Enveloppe du conseil.** Une fois approuvée, les membres du conseil Signent le Diet
   命題 (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) とその関連事項
   leurs 署名 à l'enveloppe du profil Stockée avec les fixtures.
4. **登録期間中の出版物** 登録期間中の登録、ドキュメントおよび備品の結合。
   LE CLI は、政府の安全宣言を行う前にプロフィールを保存する必要があります。
   移住プレテ。
5. **Suivi de dépréciation.** 移住後、定期的に登録を完了する
   デ・マイグレーション。

## コンセイユ・デ・クリエーション

- ボリュームのあるコンポートメントを最小限に抑えるためのピュイサンス ド ペアをご用意しています。
- マニフェストとゲートウェイを使用せずにコードを変更するマルチハッシュ、調整者、製造業者、およびゲートウェイを変更します。
  操作に関する注意事項も含まれます。
- Gardez は、テーブル ギアの種子を取得し、監査を簡素化するためのユニークなグローバル化を実現します。
- Stockez 氏は、ベンチマークの成果物 (借方の比較など) を宣伝しています。
  `docs/source/sorafs/reports/` は未来を参照します。

ペンダント操作を開始し、移行の元帳を確認します
(`docs/source/sorafs/migration_ledger.md`)。規格に準拠したランタイムを注ぐ、ヴォワール
`docs/source/sorafs/chunker_conformance.md`。