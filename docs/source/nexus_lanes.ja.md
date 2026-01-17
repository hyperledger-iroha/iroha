<!-- Japanese translation of docs/source/nexus_lanes.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

# Nexus レーンモデルと WSV 分割

> **ステータス:** NX-1 デリバラブル — レーンのタクソノミ、構成ジオメトリ、ストレージレイアウトは実装可能な状態。  
> **オーナー:** Nexus Core WG, Governance WG  
> **関連ロードマップ項目:** NX-1

本ドキュメントは Nexus のマルチレーン合意層におけるターゲットアーキテクチャをまとめる。目的は単一の決定論的なワールドステートを維持しつつ、各データスペース（レーン）が公開または非公開のバリデータセットで分離されたワークロードを実行できるようにすることにある。

> **Cross-lane proofs:** 本メモはジオメトリとストレージに焦点を当てる。レーンごとの決済コミットメント、relay パイプライン、ロードマップ **NX-4** に必要な merge-ledger 証明は [nexus_cross_lane.md](nexus_cross_lane.md) に記載する。

## コンセプト

- **Lane:** Nexus 台帳の論理シャードで、独自のバリデータセットと実行バックログを持つ。安定した `LaneId` で識別される。
- **Data Space:** コンプライアンス、ルーティング、決済ポリシーを共有する 1 つ以上のレーンを束ねるガバナンスバケット。各 dataspace は `fault_tolerance (f)` も宣言し、レーン relay 委員会のサイズ (`3f+1`) を決める。
- **Lane Manifest:** バリデータ、DA ポリシー、ガストークン、決済ルール、ルーティング権限を記述するガバナンス管理メタデータ。
- **Global Commitment:** レーンが発行する証明で、新しい状態ルート、決済データ、任意の cross-lane 転送を要約する。グローバルな NPoS リングがコミットメントを順序付ける。

## レーンのタクソノミ

レーン種別は可視性、ガバナンス面、決済フックを正規に定義する。設定ジオメトリ（`LaneConfig`）がこれらの属性を保持するため、ノード、SDK、ツールは個別ロジックなしでレイアウトを把握できる。

| レーン種別 | 可視性 | バリデータ構成 | WSV 露出 | デフォルトガバナンス | 決済ポリシー | 典型用途 |
|-----------|--------|----------------|---------|----------------------|--------------|----------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | 基本的な公開レジャー |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | 高スループットの公開アプリ |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC、コンソーシアムワークロード |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | プライバシー重視のプログラマブルマネー |

すべてのレーン種別は以下を宣言する必要がある:

- データスペースのエイリアス — コンプライアンスポリシーを束ねる人間可読のグルーピング。
- ガバナンスハンドル — `Nexus.governance.modules` を通じて解決される識別子。
- 決済ハンドル — 決済ルータが XOR バッファをデビットするために消費する識別子。
- オプションのテレメトリメタデータ（説明、連絡先、ビジネスドメイン）— `/status` とダッシュボードで表示される。

## レーン構成ジオメトリ（`LaneConfig`）

`LaneConfig` は検証済みレーンカタログから導出されるランタイムのジオメトリである。ガバナンスマニフェストを置き換えるものではなく、すべての設定済みレーンに対して決定論的なストレージ識別子とテレメトリヒントを提供する。

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` は構成が読み込まれるたびにジオメトリを再計算する（`State::set_nexus`）。
- エイリアスは小文字スラグに正規化され、連続する英数字以外の文字は `_` に潰れる。エイリアスが空のスラグになる場合は `lane{id}` にフォールバックする。
- キー接頭辞により、同じバックエンドを共有していてもレーンごとのキー範囲が分離される。
- `shard_id` はカタログのメタデータキー `da_shard_id` から導出され（デフォルトは `lane_id`）、永続化されたシャードカーソルのジャーナルを駆動して再起動/リシャーディング時の DA リプレイを決定論的に保つ。
- Kura セグメント名はホスト間で決定的であり、監査者は専用ツールなしでセグメントディレクトリとマニフェストを突合できる。
- merge セグメント（`lane_{id:03}_merge`）は当該レーンの最新の merge-hint ルートとグローバル状態コミットメントを保持する。
- ガバナンスがレーンのエイリアスを変更すると、ノードは対応する `blocks/lane_{id:03}_{slug}` ディレクトリ（および階層スナップショット）を自動的にリネームし、監査者が常に正規スラグを確認できるようにする。

## ワールドステート分割

- Nexus の論理ワールドステートはレーンごとの状態空間の合成である。公開レーンは全状態を永続化し、非公開/機密レーンは Merkle/commitment ルートを merge-ledger にエクスポートする。
- MV ストレージは `LaneConfigEntry::key_prefix` にある 4 バイトのレーン接頭辞をすべてのキーに付与し、`[00 00 00 01] ++ PackedKey` のようなキーを生成する。
- 共有テーブル（アカウント、アセット、トリガ、ガバナンスレコード）はレーン接頭辞でグルーピングされたエントリを格納し、レンジスキャンを決定論的に保つ。
- merge-ledger メタデータも同じレイアウトを反映し、各レーンが `lane_{id:03}_merge` に merge-hint ルートと縮約されたグローバル状態ルートを書き込む。これによりレーン引退時のターゲット保持/退避が可能になる。
- cross-lane インデックス（アカウントエイリアス、アセットレジストリ、ガバナンスマニフェスト）は明示的な `(LaneId, DataSpaceId)` ペアを保持する。これらのインデックスは共有カラムファミリに置かれるが、レーン接頭辞と明示的な dataspace ID を用いて決定論的なルックアップを維持する。
- merge ワークフローは merge-ledger エントリに基づく `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` タプルを使って、公開データと非公開 commitment を結合する。

## Kura と WSV の分割

- **Kura セグメント**
  - `lane_{id:03}_{slug}` — レーンの主要ブロックセグメント（ブロック、インデックス、レシート）。
  - `lane_{id:03}_merge` — 縮約状態ルートと決済アーティファクトを記録する merge-ledger セグメント。
  - グローバルセグメント（合意証跡、テレメトリキャッシュ）はレーン中立のため共有され、キーにはレーン接頭辞が含まれない。
- ランタイムはレーンカタログ更新を監視する。新規レーンは `kura/blocks/` と `kura/merge_ledger/` にブロック/merge-ledger ディレクトリが自動的に作成され、引退レーンは `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*` にアーカイブされる。
- 階層状態スナップショットも同じライフサイクルに従う。各レーンは `<cold_root>/lanes/lane_{id:03}_{slug}` に書き込み、`<cold_root>` は `cold_store_root`（`cold_store_root` 未設定時は `da_store_root`）を指し、引退時にディレクトリツリーが `<cold_root>/retired/lanes/` に移される。
- **キー接頭辞** — `LaneId` から算出した 4 バイト接頭辞は MV エンコード済みキーに常に前置される。ホスト固有のハッシュは使われないため、順序はノード間で同一となる。
- **ブロックログのレイアウト** — ブロックデータ、インデックス、ハッシュは `kura/blocks/lane_{id:03}_{slug}/` の下に配置される。merge-ledger ジャーナルは同一のスラグ（`kura/merge/lane_{id:03}_{slug}.log`）を再利用し、レーンごとのリカバリフローを分離する。
- **保持ポリシー** — 公開レーンは完全なブロック本文を保持する。commitment のみのレーンは、commitment が権威となるためチェックポイント後に古い本文をコンパクト化できる。機密レーンは専用セグメントに暗号文ジャーナルを保持し、他のワークロードをブロックしないようにする。
- **ツール** — `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` は派生した `LaneConfig` を用いて `<store>/blocks` と `<store>/merge_ledger` を検査し、アクティブ/引退セグメントを報告し、引退ディレクトリ/ログを `<store>/retired/...` にアーカイブして証跡の決定性を保つ。メンテナンスツール（`kagami`、CLI 管理コマンド）は、メトリクスや Prometheus ラベルの公開、Kura セグメントのアーカイブ時にスラグ付き namespace を再利用すべきである。

## ストレージ予算

- `nexus.storage.max_disk_usage_bytes` は、Nexus ノードが Kura、WSV のコールドスナップショット、SoraFS ストレージ、ストリーミングスプール（SoraNet/SoraVPN）で消費すべき総ディスク予算を定義する。
- 全体予算を超えた場合のエビクションは決定論的で、SoraNet のプロビジョニングスプールをパス順で削除し、次に SoraVPN スプール、続いて tiered-state のコールドスナップショットを古い順に削除する（`da_store_root` が設定されている場合はそこへオフロード）、次に Kura の退役セグメントを削除し、最後に Kura のアクティブなブロックボディを `da_blocks/` に退避して読み取り時に DA 再水和する。
- `nexus.storage.max_wsv_memory_bytes` は WSV の決定論的インメモリ計測サイズを `tiered_state.hot_retained_bytes` に反映させて WSV ホット層を上限化する。グレース保持により一時的に超過する可能性があるが、超過はテレメトリ（`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`）で観測できる。
- `nexus.storage.disk_budget_weights` は基準点でディスク予算をコンポーネントに分割する（合計 10,000 必須）。導出された上限は `kura.max_disk_usage_bytes`、`tiered_state.max_cold_bytes`、`sorafs.storage.max_capacity_bytes`、`streaming.soranet.provision_spool_max_bytes`、`streaming.soravpn.provision_spool_max_bytes` に適用される。
- Kura のストレージ予算の適用は、アクティブ/引退レーンのセグメントに跨るブロックストアのバイト数を合算し、未永続化のキュー済みブロックも含めて書き込み遅延中の超過を防ぐ。
- SoraVPN のプロビジョニングスプールは `streaming.soravpn` 設定を使い、SoraNet のプロビジョニングスプールと独立して上限が適用される。
- コンポーネントごとの制限も有効で、明示的な非ゼロ上限がある場合は Nexus 由来の上限と小さい方が適用される。
- 予算テレメトリは `storage_budget_bytes_used{component=...}` と `storage_budget_bytes_limit{component=...}` を用いて `kura`、`wsv_hot`、`wsv_cold`、`soranet_spool`、`soravpn_spool` の使用量/上限を報告する。`storage_budget_exceeded_total{component=...}` は新規データが拒否されると増加し、ログがオペレーターに警告を出す。
- DA エビクションのテレメトリとして、`storage_da_cache_total{component=...,result=hit|miss}` と `storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` が `kura` と `wsv_cold` のキャッシュ動作と移動バイト数を追跡する。
- Kura は承認時に使う会計（ディスクバイト + キュー済みブロック、merge-ledger のペイロードがある場合はそれも含む）をそのまま報告するため、ゲージは永続化済みバイトだけではなく実効的な圧力を反映する。

## ルーティングと API

- Torii の REST/gRPC エンドポイントは任意の `lane_id` を受け付ける。未指定の場合は `lane_default` を意味する。
- SDK はレーンセレクタを提供し、レーンカタログを使ってユーザーフレンドリーなエイリアスを `LaneId` にマッピングする。
- ルーティングルールは検証済みカタログに基づいてレーンと dataspace の両方を選択できる。`LaneConfig` はダッシュボードやログ向けにテレメトリに適したエイリアスを提供する。

## 決済と手数料

- すべてのレーンはグローバルバリデータセットに XOR 手数料を支払う。レーンはネイティブガストークンを徴収できるが、commitment と並行して XOR の等価額をエスクローする必要がある。
- 決済証明には金額、換算メタデータ、エスクロー証明（例: グローバル手数料ボールトへの転送）が含まれる。
- 統合決済ルータ（NX-3）は同一のレーン接頭辞を用いてバッファをデビットするため、決済テレメトリはストレージジオメトリと一致する。

## ガバナンス

- レーンはカタログを通じてガバナンスモジュールを宣言する。`LaneConfigEntry` は元のエイリアスとスラグを保持し、テレメトリと監査トレイルの可読性を保つ。
- Nexus レジストリは、`LaneId`、dataspace バインディング、ガバナンスハンドル、決済ハンドル、メタデータを含む署名済みレーンマニフェストを配布する。
- ランタイムアップグレードフックは引き続きガバナンスポリシー（デフォルト `gov_upgrade_id`）を強制し、テレメトリブリッジを介して差分を記録する（`nexus.config.diff` イベント）。
- レーンマニフェストは管理レーンの dataspace バリデータプールを定義し、stake 選出レーンは公開レーンのステーキングレコードからバリデータプールを導出する。

## テレメトリとステータス

- `/status` はレーンのエイリアス、dataspace バインディング、ガバナンスハンドル、決済プロファイルを公開し、カタログと `LaneConfig` に基づいて導出される。
- スケジューラメトリクス（`nexus_scheduler_lane_teu_*`）はレーンのエイリアス/スラグを表示し、オペレーターがバックログと TEU 圧力を素早く把握できるようにする。
- `nexus_lane_configured_total` は導出されたレーンエントリ数をカウントし、構成変更時に再計算される。テレメトリはレーンジオメトリが変化するたびに署名付き差分を送出する。
- dataspace のバックログゲージはエイリアス/説明メタデータを含み、オペレーターがキュー圧力とビジネスドメインを関連付けやすくする。

## 設定と Norito 型

- `LaneCatalog`、`LaneConfig`、`DataSpaceCatalog` は `iroha_data_model::nexus` にあり、マニフェストと SDK 向けに Norito 互換構造を提供する。
- `LaneConfig` は `iroha_config::parameters::actual::Nexus` にあり、カタログから自動的に導出される。内部ランタイムヘルパーであるため Norito エンコードは不要。
- ユーザー向け設定（`iroha_config::parameters::user::Nexus`）は宣言的なレーン/データスペース記述子を引き続き受け付け、解析時にジオメトリを導出して無効なエイリアスや重複レーン ID を拒否する。
- `DataSpaceMetadata.fault_tolerance` はレーン relay 委員会のサイズを制御する。委員会メンバーは `(dataspace_id, lane_id)` を結合した VRF エポックシードにより、dataspace バリデータプールからエポックごとに決定論的にサンプリングされる。

## 残作業

- 新しいジオメトリに合わせて決済ルータ更新（NX-3）を統合し、XOR バッファのデビットとレシートをレーンスラグでタグ付けする。
- merge アルゴリズム（順序、プルーニング、競合検出）を最終化し、cross-lane リプレイ用のリグレッションフィクスチャを追加する。
- ホワイトリスト/ブラックリストおよびプログラマブルマネーポリシーのコンプライアンスフックを追加する（NX-12 で追跡）。

---

*本ドキュメントは NX-2 から NX-18 のタスク進行に伴い更新される。未解決の疑問はロードマップまたはガバナンストラッカーに記録してほしい。*
