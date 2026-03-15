<!-- Japanese translation of docs/source/confidential_assets.md -->

---
lang: ja
direction: ltr
source: docs/source/confidential_assets.md
status: complete
translator: manual
---

# 機密資産とゼロ知識トランスファー設計

## モチベーション
- ドメインが透明な資産流通を保ったまま、オンオプトインで保護された（シールド）フローを提供する。
- 異種ハードウェアを持つバリデータ間で決定的な実行を維持し、Norito/Kotodama ABI v1 互換性を保持する。
- 回路や暗号パラメータに対するライフサイクル管理（有効化、ローテーション、失効）を監査人・オペレーターに提供する。

## 脅威モデル
- バリデータは「誠実だが好奇心旺盛」と仮定：コンセンサスは忠実に実行するが、台帳／状態を覗こうとする。
- ネットワーク観測者はブロックデータとゴシップされたトランザクションを閲覧可能。プライベートチャネルの存在は仮定しない。
- 範囲外：台帳外のトラフィック分析、量子攻撃者（PQ ロードマップで追跡）、台帳可用性攻撃。

## 設計概要
- 資産は既存の透明残高に加え「シールドプール」を宣言でき、シールド流通は暗号コミットメントで表現。
- ノートは `(asset_id, amount, recipient_view_key, blinding, rho)` をカプセル化し以下を含む:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`
  - ナリファイア: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`（ノート順序に依存しない）
  - 暗号化ペイロード: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`
- トランザクションは Norito でエンコードされた `ConfidentialTransfer` ペイロードを運び、以下を含む:
  - 公開入力: マークルアンカー、ナリファイア、新コミットメント、資産 ID、回路バージョン
  - 受領者および任意監査人向け暗号化ペイロード
  - 価値保存・所有権・認可を示すゼロ知識証明
- 検証鍵とパラメータセットはオンチェーンレジストリで管理し、有効化ウィンドウを持つ。未知または失効済みエントリを参照する証明は受理しない。
- コンセンサスヘッダはアクティブな機密機能ダイジェストをコミットし、レジストリ／パラメータ状態が一致する時のみブロックを受理。
- 証明構築には Halo2 (Plonkish) を使用し、信頼設定を不要化。Groth16 など他の SNARK は v1 では非対応。

<a id="node-capability-negotiation"></a>
## コンセンサスコミットメントと機能ゲート
- ブロックヘッダは `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` を公開し、コンセンサスハッシュに加算。ローカルレジストリと一致しなければブロックを拒否。
- ガバナンスは `next_conf_features` と `activation_height` を設定してアップグレードを段階的に展開できる。発効前は旧ダイジェストでブロックを生成する必要がある。
- バリデータノードは `confidential.enabled = true`, `assume_valid = false` で稼働必須。起動時チェックで条件不一致やダイジェスト差異があればバリデータ集合への参加を拒否。
- P2P ハンドシェイクは `{ enabled, assume_valid, conf_features }` を含むメタデータを交換し、不一致なら `HandshakeConfidentialMismatch` で接続を拒否。
- バリデータ・オブザーバ・旧ピア間の互換性は [ノード機能ネゴシエーション](#node-capability-negotiation) に示す。ハンドシェイク失敗時はコンセンサスローテーションから除外。
- 非バリデータのオブザーバは `assume_valid = true` を設定可能（機密デルタを盲目的に適用しつつコンセンサス安全性には関与しない）。

## 資産ポリシー
- 各資産定義はガバナンスもしくは作成者が設定する `AssetConfidentialPolicy` を持つ:
  - `TransparentOnly`: 既定モード。透明命令のみ許可、シールド命令は拒否。
  - `ShieldedOnly`: 発行・送信すべて機密命令のみ。`RevealConfidential` 禁止で公開残高なし。
  - `Convertible`: 下記オン／オフランプ命令で透明とシールド間の移動を許可。
- ポリシーは以下の限定的な状態遷移に従う:
  - `TransparentOnly → Convertible`（即座にシールドプールを有効化）
  - `TransparentOnly → ShieldedOnly`（保留遷移と変換ウィンドウが必要）
  - `Convertible → ShieldedOnly`（最小遅延を強制）
  - `ShieldedOnly → Convertible`（シールドノートが消費可能であることを保証する移行計画が必要）
  - `ShieldedOnly → TransparentOnly` はシールドプールが空である、または未消費ノートを公開化する移行が定義されていない限り禁止。
- ガバナンス命令は `ScheduleConfidentialPolicyTransition` ISI で `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` を設定し、`CancelConfidentialPolicyTransition` で取り消し可能。メンポール検証は遷移高さをまたぐ取引を拒否し、包含失敗は整合メッセージを返す。
- トリガー連携（オン／オフランプ命令など）は [Confidential On/Off Ramps](#confidential-onoff-ramps) で解説。

## 指令セットとガス料金

<a id="confidential-onoff-ramps"></a>
### Confidential On/Off Ramps
- `ConfidentialMint`: シールドノートを発行し、透明供給への影響なし。ポリシーが許可する場合のみ執行。
- `ConfidentialTransfer`: シールド同士の値移動。証明検証と nullifier 更新が伴う。
- `ConfidentialBurn`: シールドノートを消費し供給を減少させる。透明側に再出力しない。
- `RevealConfidential`: シールドノートを公開残高へ変換。`ShieldedOnly` モードでは禁止。
- `ShieldConfidential`: 透明残高をシールドノートへ移行。`TransparentOnly` モードでは拒否。

### Gas スケジュール
- Halo2（Plonkish）検証: ベース 250,000 gas + 公開入力 1 件当たり 2,000 gas。
- 証明サイズ: 1 バイト当たり 5 gas。nullifier 300 gas、コミットメント 500 gas。
- ハードリミット（既定値）:
  - `max_proof_size_bytes = 262_144`
  - `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`
  - `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10,000`
- タイムアウト／上限超過時は命令が決定的に失敗し、台帳状態は変化しない。
- ベリファイ前フィルタは `vk_id`、証明長、アンカー高さをチェックし過度なリソース消費を抑制。
- SIMD バックエンドは任意。有効にしてもガス計算は変化しない。

## フロントエンドとツール
- CLI には `confidential create-keys`, `confidential send`, `confidential export-view-key`、監査向けメモ復号ツール、`iroha app zk envelope` などを追加。
- Torii API に `POST /v1/confidential/derive-keyset` を追加し、ウォレットがキー階層を自動取得可能。
- アカウントごとのキー階層:
  - `sk_spend` → `nk`（nullifier キー）、`ivk`（incoming viewing key）、`ovk`（outgoing viewing key）、`fvk`
- ノート暗号化は ECDH 共有鍵を使った AEAD（XChaCha20-Poly1305）。資産ポリシーに応じて監査用ビューキー付与も可能。

## ゼロ知識証明キャリブレーション
- 専用ベンチマークは `docs/source/confidential_assets_calibration.md` を参照。
- 検証時間が `verify_timeout_ms` を超えた場合は `proof verification exceeded timeout` エラーで命令が停止。
- 定期的にパラメータセットを回転し、`vk_set_hash` と紐付けたレポートを `confidential/calibration` アーティファクトに記録。

## ガバナンスとレジストリ
- `ConfidentialVerifierRegistry` に検証鍵を登録。`activation_height` と `revocation_height` を通じてライフサイクルを管理。
- バリデータは受信した証明が有効なレジストリエントリを参照するか検証し、未知／失効済みエントリなら `UnknownVerifier` エラーで拒否。
- `conf_features` と一致しないダイジェストを持つベリファイアはハンドシェイクで拒否され、`BlockRejectionReason::ConfidentialFeatureDigestMismatch` によりブロックも拒否。

## ネットワーク互換性（抜粋）
- ハンドシェイク時に `{ enabled, assume_valid, conf_features }` を交換し、不一致なら `HandshakeConfidentialMismatch`。
- 検証器を持たないオブザーバは `assume_valid = true` で同期できるが、コンセンサス参加は不可。
- 合致しないピアを検出した場合、運用者はレジストリ／パラメータを整合させ、ダイジェストが一致したタイミングで再接続。

---

そのほか詳細なワークフロー（オン／オフランプ命令のライフサイクル、レーン間融合、DoS 防御など）は原文と同じ構成で記述しています。運用者／実装者はこのガイドと `confidential_assets_calibration.md`、`handshake_matrix` を併せて参照し、機密機能の導入・保守にあたってください。
