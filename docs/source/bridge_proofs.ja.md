---
lang: ja
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> このページは翻訳した要約であり、完全な翻訳ではありません。ガバナンス、
> API、proof の意味論、リリース要件については、
> [英語の正規ページ](bridge_proofs.md)が厳密な規範文書です。

# SCCP V1 bridge proof — 要約

## 初回リリースの範囲

SCCP V1 は初回リリース用の closed protocol です。外部 source として
サポートするのは `ethereum-mainnet`、`bsc-mainnet`、`tron-mainnet` のみで、
SORA 側の destination は `sora-taira` だけです。Solana、TON、custom network、
その他の SORA destination はサポートせず、安全側に倒して拒否します。

このリリースで `SubmitBridgeProof` が受理するのは、型付きの
`NativeProtocol` proof と `SccpDestination` proof だけです。汎用 `Ics` または
`TransparentZk` の提出は利用できず、権威ある on-chain verifier が実装される
までは拒否されます。

## 型付き registry と replay 防止

`SccpRegistryV1` は lane に固定された型付き append-only registry です。
各 lane が保持できる履歴は route revision が最大 64 件、native trust anchor が
最大 4,096 件です。履歴を暗黙に削除することはなく、上限に達した後の追加は
state を変更せず atomic に拒否されます。

Anchor interval は認証済みの consensus 進行座標で測定します。Ethereum は
finalized beacon slot、BSC と TRON は finalized native block height を使用します。
古い anchor は successor checkpoint の境界を含めて有効で、最後の current anchor
は終端が開いています。Terminal route の finality cutoff は、historical anchor の
successor checkpoint と厳密に一致しなければなりません。

永続 inbound record は event/source finality height と、検証済みの
`anchor_interval_height` を別々に保持します。Lane と anchor hash を key とする
永続 high-water index により、すでに受理した座標より低い successor checkpoint を
governance が選ぶことはできません。Snapshot hydration は永続 record から index を
再計算して完全一致を要求し、欠落、古い値、不正形式、裏付けのない値を拒否します。
消費済み message id も replay 防止のため永続化されます。

## Single-pass 検証と work limit

Destination proof と native proof は一度だけ構造化して一度だけ binding し、重い
暗号処理を始める前に deterministic work を予約します。Destination path は BN254
pairing-product と local BLS finality をそれぞれ一度だけ検証します。Native path は
canonical shortest-prefix を要求し、上限は BSC が 1,004 headers、TRON が 54
headers です。

`[zk.sccp]` は、proof count/bytes、native headers/bytes、Ethereum light-client
updates、secp256k1 recoveries、BLS aggregate checks/key contributions、BN254
pairing checks に対し、ゼロではない transaction 単位と block 単位の上限を課します。
これらの admission limit は consensus-bound です。すべての validator が config file
で同じ値を使う必要があり、environment variable による override はありません。

初回リリースのデフォルト上限は次のとおりです。

| Work dimension | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

1 proof が含められる canonical bytes は最大 8 MiB です。破棄または拒否された
transaction の予約済み work が block に漏れることはありません。

## Torii と HTTP の上限

Torii は SCCP endpoint ごとに JSON body 上限を設け、body の読み取り、メモリ確保、
暗号検証より前に適用します。上限を超える `Content-Length` または chunked body は
HTTP `413` で拒否されます。Client も decode 後の HTTP response を固定上限内で
読み取るため、`Content-Length` が欠落または虚偽でも上限を回避できません。

JSON、base64、Norito の入力はすべて canonical でなければなりません。Unknown field、
duplicate key、不一致の network/route/anchor、replay、work quota 超過、検証失敗は、
state を部分的に変更せず拒否されます。
