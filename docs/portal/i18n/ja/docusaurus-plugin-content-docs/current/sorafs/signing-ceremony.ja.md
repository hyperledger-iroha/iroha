---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d70b9231127d0b913e0aa01aba875b26f11aa79d8ee8274f580f5bbfd0e565b
source_last_modified: "2025-11-14T04:43:22.321629+00:00"
translation_last_reviewed: 2026-01-30
---

> Roadmap: **SF-1b — Sora Parliament fixture approvals.**
> Parliament のワークフローが旧来のオフライン「評議会署名セレモニー」を置き換えます。

SoraFS chunker fixtures の手動署名儀式は廃止されました。すべての承認は
**Sora Parliament** を通過します。これは Nexus を統治する抽選ベースの DAO です。
Parliament メンバーは市民権取得のために XOR をボンドし、パネル間をローテーションし、
fixtures のリリースを承認・却下・ロールバックする on-chain 投票を行います。
本ガイドではこのプロセスと開発者向け tooling を説明します。

## Parliament の概要

- **市民権** — オペレータは必要な XOR をボンドして市民として登録し、抽選対象になります。
- **パネル** — 責務はローテーションするパネルに分割されます (Infrastructure,
  Moderation, Treasury, ...)。Infrastructure Panel が SoraFS fixture 承認を担当します。
- **抽選とローテーション** — パネルの席は Parliament 憲章で定められた周期で再抽選され、
  単一グループの独占を防ぎます。

## Fixture 承認フロー

1. **提案提出**
   - Tooling WG が候補の `manifest_blake3.json` bundle と fixture diff を
     `sorafs.fixtureProposal` 経由で on-chain registry にアップロードします。
   - 提案には BLAKE3 digest、セマンティックバージョン、変更ノートが記録されます。
2. **レビューと投票**
   - Infrastructure Panel が Parliament のタスクキュー経由で割り当てを受け取ります。
   - パネルメンバーは CI 成果物を確認し、パリティテストを実行し、on-chain で重み付き投票を行います。
3. **確定**
   - クオーラムに達すると、ランタイムが承認イベントを発行し、
     canonical manifest digest と fixture payload の Merkle コミットメントを含めます。
   - イベントは SoraFS registry にミラーされ、クライアントは最新の Parliament 承認 manifest を取得できます。
4. **配布**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) が Nexus RPC から承認 manifest を取得します。
     リポジトリの JSON/TS/Go 定数は `export_vectors` の再実行と on-chain 記録との
     digest 検証で同期されます。

## 開発者ワークフロー

- fixtures の再生成:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Parliament の fetch helper を使って承認済み envelope を取得し、署名を検証し、
  ローカル fixtures を更新します。`--signatures` は Parliament が公開した envelope に
  向けます。helper は付随する manifest を解決し、BLAKE3 digest を再計算し、
  canonical プロファイル `sorafs.sf1@1.0.0` を強制します。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

manifest が別 URL にある場合は `--manifest` を渡します。署名なし envelope は、
ローカル smoke 実行のために `--allow-unsigned` を設定しない限り拒否されます。

- staging gateway で manifest を検証する場合は、ローカル payload ではなく Torii を指定します:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- ローカル CI では `signer.json` roster が不要になりました。
  `ci/check_sorafs_fixtures.sh` は repo 状態を最新の on-chain commitment と比較し、
  乖離があれば失敗します。

## ガバナンス注意点

- Parliament 憲章がクオーラム、ローテーション、エスカレーションを規定するため、
  crate レベルの設定は不要です。
- 緊急ロールバックは Parliament のモデレーションパネルで扱われます。
  Infrastructure Panel が以前の manifest digest を参照する revert 提案を提出し、
  承認後にリリースが置き換わります。
- 過去の承認履歴は SoraFS registry に残り、フォレンジック replay に利用可能です。

## FAQ

- **`signer.json` はどこへ？**  
  削除されました。署名の帰属はすべて on-chain にあり、リポジトリの
  `manifest_signatures.json` は開発者向け fixture に過ぎず、最新の承認イベントと
  一致している必要があります。

- **ローカル Ed25519 署名はまだ必要？**  
  いいえ。Parliament の承認は on-chain アーティファクトとして保存されます。
  ローカル fixtures は再現性のために存在しますが、Parliament digest と照合されます。

- **承認状況の監視方法は？**  
  `ParliamentFixtureApproved` イベントを購読するか、Nexus RPC 経由で registry を
  照会して現在の manifest digest とパネルの名簿を取得してください。
