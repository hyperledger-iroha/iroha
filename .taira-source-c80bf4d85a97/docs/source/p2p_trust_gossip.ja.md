---
lang: ja
direction: ltr
source: docs/source/p2p_trust_gossip.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19912eba5930ce21483d018a6dd846fb38a4f2060013e5cdcf8dd1173a0a9aa7
source_last_modified: "2025-12-09T08:20:57.803383+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/p2p_trust_gossip.md -->

# Trust Gossip 機能

trust-gossip プレーンは、ピアに関する署名済みの信頼ヒント（スコア、bad-gossip 罰則、
unknown-peer 罰則）を運び、ピアアドレスのゴシップとは意図的に分離されている。
本ノートは公開 (NPoS) オーバーレイと許可型デプロイの双方に向けて、能力セマンティクス、
ゲーティング規則、運用上の期待値をまとめる。

## 決定

unknown-peer 罰則は引き続き有効にする（この面を削除しない）ことで、許可型デプロイが
トポロジ外からの信頼スパムを拒否できる一方、ピアアドレスのゴシップには影響を与えない。
公開 (NPoS) オーバーレイは unknown peer への罰則を付けないままにし、信頼交換を開放的に保つ。
運用者は `p2p_trust_penalties_total{reason="unknown_peer"}` と `trust_min_score` の下限に
注意し、予期しない低下を監視すること。スコアは `trust_decay_half_life_ms` に従ってゼロへ減衰し、
手動介入なしで回復できる。

## ケイパビリティモデル

- 能力交渉には 2 つの設定フラグが関与する。
  - `network.trust_gossip` — trust gossip を送受信するローカル意図（デフォルト `true`）。
  - `network.soranet_handshake.trust_gossip` — ハンドシェイクでの広告（デフォルト `true`）。
- ハンドシェイクは各側の 2 つのフラグを AND する。**両方のピア** が両方のフラグを
  `true` にした場合のみ trust フレームが流れる。どちらかが無効化すると trust gossip は
  エンドツーエンドでスキップされるが、ピアアドレスのゴシップは影響を受けない。
- トピック分類は明示的で、`NetworkMessage::PeerTrustGossip` は `Topic::TrustGossip` に対応し、
  ピアアドレスのゴシップは `Topic::PeerGossip` のまま。caps/backoff/relay TTL は変更されず、
  peer gossip と共有される。

## 送受信の挙動

- **送信経路**
  - ローカル能力が無効のとき、`Topic::TrustGossip` への投稿/ブロードキャストは即座に拒否され、
    `p2p_trust_gossip_skipped_total{direction="send",reason="local_capability_off"}` を記録する。
  - リモートが trust を交渉していない場合、フレームはキュー投入前にスキップされ、
    `reason="peer_capability_off"` が付与されて debug ログに出る。ピアアドレスのゴシップは
    既存の Low キューで継続する。
- **受信経路**
  - ローカルが能力を無効化している、または接続が trust なしで交渉された場合、trust フレームは
    cap/size の適用前に破棄される。破棄は `direction="recv"` の同じスキップカウンタを増やす。
  - トピックの caps/backpressure は維持される。trust と peer gossip は Low キューと
    `max_frame_bytes_peer_gossip` を共有するため、能力ゲートが peer gossip を飢餓させない。
- **Relay**
  - relay TTL と転送ルールは peer gossip と同じ。ハブは trust を交渉済みのピアにのみ
    trust フレームを転送し、そうでない場合は同じテレメトリラベルでスキップする。

## 設定チェックリスト

- 公開/NPoS オーバーレイ: 両方のノブを有効にして trust スコアがネットワーク全体に伝播するようにする。
  スキップカウンタはゼロのままが望ましく、増加があれば設定ミスの可能性を調査する。
- 許可型オーバーレイ: `network.trust_gossip=false` にして trust 交換を抑止し、ピアアドレスの
  ゴシップは維持する。`p2p_trust_gossip_skipped_total` はピアが trust を送ろうとすると増えるが、
  これは想定どおりで能力ゲートの確認になる。
- テンプレート: `docs/source/references/peer.template.toml` は `[network]` 配下に `trust_gossip` を
  追加し、trust の減衰/罰則と並べて公開する。設定リファレンスは両方のノブとスキップカウンタを記載する。

## テレメトリと診断

- スキップカウンタ: `p2p_trust_gossip_skipped_total{direction,reason}` は送信/受信のドロップ
  （`local_capability_off` と `peer_capability_off`）で増加する。公開モードでは予期せぬ抑止の
  検知に、許可型モードでは能力ゲートの検証に使う。
- trust スコアのメトリクスは変更なし: `p2p_trust_score{peer_id}`、
  `p2p_trust_penalties_total{reason}`、`p2p_trust_decay_ticks_total{peer_id}`。
- ログ: debug レベルで能力ドロップが記録される。peer gossip は継続するため、ドロップは
  アドレスゴシップの停滞と相関しないはず。

## 非目標

- スロットリング変更なし: この能力は新しいレート制限や Low キュー cap を**追加しない**。
  ピアアドレスのゴシップは既存の `PeerGossip` トピックと cap を利用する。
- ピアアドレスの抑止なし: trust gossip を無効化してもアドレスゴシップや relay 対象選択には
  影響しない。片方が有効・片方が無効の混在構成でもアドレス交換は維持され、接続は継続する。
- ABI トグルなし: trust-gossip の opcode/syscall は無条件に出荷され、能力はワイヤ上で
  送受信するかどうかだけを制御する。

## テスト範囲

- Unit: `trust_gossip_allowed` は能力が無効なとき trust フレームを拒否する。
- Integration: `crates/iroha_p2p/tests/integration/p2p_trust_gossip.rs` が以下をカバーする。
  - Trust 無効 → trust フレームは両方向で破棄され、peer gossip は流れる。
  - Trust 有効 → trust フレームが既存の caps/fanout の下でエンドツーエンドに配送される。

これらの挙動は決定論的かつハードウェア非依存であり、スキップはカウント/ログされるだけで
再送は行われない。
