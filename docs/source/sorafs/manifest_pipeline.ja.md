---
lang: ja
direction: ltr
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS チャンク化 → マニフェストパイプライン

このメモは、バイトの payload を SoraFS レジストリへの pinning に適した Norito エンコードの
マニフェストへ変換するために必要な最小手順をまとめたものです。

1. **payload を決定的にチャンク化する**
   - `sorafs_car::CarBuildPlan::single_file`（内部で SF-1 chunker を使用）を使い、チャンクの
     オフセット、長さ、BLAKE3 ダイジェストを導出します。
   - このプランは payload のダイジェストとチャンクのメタデータを公開し、下流のツールが
     CAR 組み立てと Proof-of-Replication のスケジューリングに再利用できます。
   - 代替として、プロトタイプの `sorafs_car::ChunkStore` はバイト列を取り込み、後続の CAR
     構築に使う決定的なチャンクメタデータを記録します。Store は 64 KiB / 4 KiB の PoR
     サンプリングツリー（ドメインタグ付き、チャンク整列）を導出するため、スケジューラーは
     payload を再読込せずに Merkle 証明を要求できます。
    `--por-proof=<chunk>:<segment>:<leaf>` を使ってサンプル葉の JSON 証跡を出力し、
    `--por-json-out` でルートダイジェストのスナップショットを書き出します。`--por-proof`
    と `--por-proof-out=path` を組み合わせて証跡を保存し、`--por-proof-verify=path` で既存の
    証明が現在の payload に対する `por_root_hex` 計算結果と一致することを確認します。複数の
    葉に対しては、`--por-sample=<count>`（`--por-sample-seed` と `--por-sample-out` は任意）で
    決定的なサンプルを生成し、要求が利用可能な葉数を超える場合は `por_samples_truncated=true`
    を設定します。
   - bundle 証明（CAR マニフェスト、PoR スケジュール）を構築する場合は、チャンクの
     オフセット/長さ/ダイジェストを保存してください。
   - 正規のレジストリエントリと交渉ガイダンスは
     [`sorafs/chunker_registry.md`](chunker_registry.md) を参照してください。

2. **マニフェストをラップする**
   - チャンクメタデータ、ルート CID、CAR コミットメント、pin ポリシー、alias claims、
     ガバナンス署名を `sorafs_manifest::ManifestBuilder` に渡します。
   - Norito バイトを取得するために `ManifestV1::encode` を呼び、Pin Registry に記録される
     正規ダイジェストを得るために `ManifestV1::digest` を呼びます。

3. **公開する**
   - ガバナンス（評議会署名、alias 証明）を通じてマニフェストのダイジェストを提出し、
     決定的なパイプラインでマニフェストのバイト列を SoraFS に pin します。
   - マニフェストが参照する CAR ファイル（任意の CAR インデックスも含む）が同じ SoraFS
     pin セットに保存されていることを確認してください。

### CLI クイックスタート

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

このコマンドはチャンクのダイジェストとマニフェスト詳細を出力します。`--manifest-out`
および/または `--car-out` が指定されている場合、Norito payload と仕様準拠の CARv2
アーカイブ（pragma + header + CARv1 blocks + Multihash index）をディスクに書き込みます。
ディレクトリパスを渡すと、ツールは再帰的（辞書順）に走査し、すべてのファイルをチャンク化して
ディレクトリルートの dag-cbor ツリーを生成します。この CID はマニフェストと CAR の両方の
ルートに現れます。JSON レポートには、計算された CAR payload ダイジェスト、アーカイブ全体の
ダイジェスト、サイズ、raw CID、ルート（dag-cbor codec を分離）が含まれ、さらにマニフェストの
alias/metadata エントリも含まれます。CI 実行中に計算されたルートや codec を *検証* するには
`--root-cid`/`--dag-codec` を使用し、payload ハッシュを強制するには `--car-digest`、
事前計算された raw CAR 識別子（CIDv1、`raw` codec、BLAKE3 multihash）を強制するには
`--car-cid`、出力 JSON をマニフェスト/CAR アーティファクトと一緒に保存するには `--json-out`
を使用してください。

`--manifest-signatures-out` が指定され（`--council-signature*` フラグが少なくとも 1 つ必要）、
ツールはマニフェストの BLAKE3 ダイジェスト、チャンクプランの SHA3-256 集約ダイジェスト
（オフセット、長さ、チャンクの BLAKE3 ダイジェスト）と評議会の署名を含む
`manifest_signatures.json` エンベロープも出力します。エンベロープは chunker プロファイルを
`namespace.name@semver` の正規形式で記録し、旧 `namespace-name` エンベロープは互換性のため
引き続き検証されます。下流の自動化はこのエンベロープをガバナンスログへ公開するか、マニフェスト
と CAR のアーティファクトと一緒に配布できます。外部署名者からエンベロープを受け取った場合は、
`--manifest-signatures-in=<path>` を追加して、CLI がダイジェストを確認し、計算したばかりの
マニフェストダイジェストに対して各 Ed25519 署名を検証するようにしてください。

複数の chunker プロファイルが登録されている場合は `--chunker-profile-id=<id>` で明示的に
選択できます。このフラグは [`chunker_registry`](chunker_registry.md) の数値 ID に対応し、
チャンク化のパスと出力されたマニフェストが同じ `(namespace, name, semver)` を参照することを
保証します。自動化では canonical handle（`--chunker-profile=sorafs.sf1@1.0.0`）を優先し、
数値 ID の固定化を避けてください。現在のレジストリエントリを見るには
`sorafs_manifest_chunk_store --list-profiles` を実行し（出力は `sorafs_manifest_chunk_store`
が提供する一覧と一致します）、レジストリ更新準備時には `--promote-profile=<handle>` を使って
canonical handle と alias メタデータをエクスポートします。

監査人は `--por-json-out=path` を使って Proof-of-Retrievability の完全なツリーを要求でき、
これは sampling 検証向けに chunk/segment/leaf ダイジェストをシリアライズします。個別の証跡は
`--por-proof=<chunk>:<segment>:<leaf>` で出力し（`--por-proof-verify=path` で検証）、
`--por-sample=<count>` は重複のない決定的サンプルを生成してスポットチェックに使えます。

JSON を書き出すフラグ（`--json-out`、`--chunk-fetch-plan-out`、`--por-json-out` など）は
`-` をパスとして受け付けるため、一時ファイルを作成せずに payload を stdout へ直接
ストリームできます。

`--chunk-fetch-plan-out=path` を使って、マニフェストプランに付随する順序付きの chunk fetch
仕様（chunk index、payload offset、length、BLAKE3 digest）を保存します。マルチソースの
クライアントは、生成された JSON を source payload を再読込せずに SoraFS fetch オーケストレータへ
直接渡せます。CLI が出力する JSON レポートにも `chunk_fetch_specs` 配列が含まれます。
`chunking` セクションと `manifest` オブジェクトの両方が `profile_aliases` を canonical
`profile` handle と並べて公開するため、SDK はレガシー `namespace-name` 形式から互換性を失わずに
移行できます。

stub を再実行する場合（CI や release パイプラインなど）、`--plan=chunk_fetch_specs.json`
または `--plan=-` を渡して以前生成した仕様を取り込めます。CLI は取り込みを続行する前に、
各チャンクの index、offset、length、BLAKE3 digest が新たに導出された CAR プランと一致することを
検証するため、古い／改ざんされたプランを防げます。

### ローカルオーケストレーションのスモークテスト

`sorafs_car` クレートには `sorafs-fetch` が同梱されており、`chunk_fetch_specs` 配列を消費して
ローカルファイルからのマルチプロバイダ取得をシミュレートする開発者向け CLI です。
`--chunk-fetch-plan-out` が生成した JSON を指定し、1 つ以上のプロバイダ payload パス（`#N` で
並列度を増やすことも可能）を渡すと、チャンク検証、payload 再組み立て、プロバイダの成功/失敗数
とチャンクごとのレシートを要約した JSON レポートを出力します。

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

このフローを使ってオーケストレーターの挙動を検証したり、実際のネットワーク転送を SoraFS
ノードに接続する前にプロバイダ payload を比較できます。

ローカルファイルではなくライブの Torii gateway に接続する必要がある場合は、`--provider=/path`
フラグを新しい HTTP 向けオプションに置き換えてください。

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

CLI は stream token を検証し、chunker/profile の整合性を強制し、通常のプロバイダレシートと
併せて gateway メタデータを記録します。これにより、オペレーターはロールアウトの証跡として
レポートをアーカイブできます（blue/green フローの詳細はデプロイメントハンドブック参照）。

`--provider-advert=name=/path/to/advert.to` を渡すと、CLI は Norito エンベロープをデコードし、
Ed25519 署名を検証して、プロバイダが `chunk_range_fetch` 能力を広告していることを強制します。
これにより、マルチソース fetch シミュレーションがガバナンスの admission ポリシーと一致し、
範囲チャンク要求を満たせないレガシープロバイダの誤使用を防ぎます。

`#N` サフィックスはプロバイダの並列度上限を増やし、`@W` はスケジューリングの重みを設定します
（省略時は 1）。adverts や gateway ディスクリプタが提供されると、CLI は fetch 開始前に
オーケストレーターの scoreboard を評価します。適格なプロバイダはテレメトリを反映した重みを継承し、
JSON スナップショットは `--scoreboard-out=<path>` が指定されている場合に保存されます。能力チェックや
ガバナンスの期限に失敗したプロバイダは警告付きで自動的に除外され、実行は admission ポリシーに
準拠し続けます。サンプルの入出力ペアは
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` を参照してください。

`--expect-payload-digest=<hex>` および/または `--expect-payload-len=<bytes>` を渡すと、出力を
書き込む前に組み立てた payload がマニフェストの期待値に一致することを確認できます。これは
オーケストレーターがチャンクを静かに欠落/並べ替えしないことを保証したい CI スモークテストに有用です。

すでに `sorafs-manifest-stub` で生成された JSON レポートがある場合は、`--manifest-report=docs.report.json`
で直接渡してください。fetch CLI は埋め込み済みの `chunk_fetch_specs`、`payload_digest_hex`、`payload_len`
を再利用するため、別のプラン/検証ファイルを管理する必要はありません。

fetch レポートは監視に役立つ集約テレメトリも提供します：`chunk_retry_total`、`chunk_retry_rate`、
`chunk_attempt_total`、`chunk_attempt_average`、`provider_success_total`、`provider_failure_total`、
`provider_failure_rate`、`provider_disabled_total` は fetch セッション全体の健全性を示し、Grafana/Loki
ダッシュボードや CI アサーションに適しています。`--provider-metrics-out` を使えば、下流ツールが
プロバイダ単位の統計だけ必要な場合に `provider_reports` 配列のみを書き出せます。

### 次のステップ

- ガバナンスログにマニフェストダイジェストと並んで CAR メタデータを記録し、観測者が payload を
  再ダウンロードせずに CAR 内容を検証できるようにします。
- マニフェストと CAR の公開フローを CI に統合し、すべての docs/artefacts ビルドが自動的に
  マニフェストを生成し、署名を取得し、結果の payload を pin できるようにします。
