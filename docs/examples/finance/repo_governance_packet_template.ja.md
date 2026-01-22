---
lang: ja
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

# Repo Governance Packet Template (Roadmap F1)

Roadmap F1 (repo lifecycle documentation & tooling) に必要な artefact bundle を準備する際にこのテンプレートを使用します。目的は、レビューアに対して、提案で参照される input、hash、evidence bundle を一覧化した単一の Markdown を渡し、ガバナンス評議会が bytes を再生できるようにすることです。

> テンプレートを自身の evidence ディレクトリにコピーし (例:
> `artifacts/finance/repo/2026-03-15/packet.md`)、placeholders を置き換え、
> 下記の hashed artefacts の隣に commit/upload してください。

## 1. Metadata

| Field | Value |
|-------|-------|
| Agreement/change identifier | `<repo-yyMMdd-XX>` |
| Prepared by / date | `<desk lead> - 2026-03-15T10:00Z` |
| Reviewed by | `<dual-control reviewer(s)>` |
| Change type | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodian(s) | `<custodian id(s)>` |
| Linked proposal / referendum | `<governance ticket id or GAR link>` |
| Evidence directory | ``artifacts/finance/repo/<slug>/`` |

## 2. Instruction Payloads

`iroha app repo ... --output` で desk が承認した staged Norito instructions を記録します。各エントリには、出力ファイルの hash と、投票が通過した際に送信されるアクションの短い説明を含めてください。

| Action | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | desk + counterparty が承認した cash/collateral legs を含む。 |
| Margin call | `instructions/margin_call.json` | `<sha256>` | cadence + call を引き起こした participant id を記録。 |
| Unwind | `instructions/unwind.json` | `<sha256>` | 条件成立後の reverse-leg の証跡。 |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

repo が `--custodian` を使う場合は必ずこのセクションを記入します。governance packet には、各 custodian の署名済み acknowledgement と、`docs/source/finance/repo_ops.md` sec 2.8 で参照されるファイルの hash が含まれている必要があります。

| Custodian | File | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<custodian@domain>` | `custodian_ack_<custodian>.md` | `<sha256>` | custody window、routing account、drill contact を含む署名済み SLA。 |

> acknowledgement は他の evidence (`artifacts/finance/repo/<slug>/`) と同じ場所に保存し、
> `scripts/repo_evidence_manifest.py` が staged instructions と config snippets と同じツリーに記録できるようにしてください。詳細は
> `docs/examples/finance/repo_custodian_ack_template.md` を参照。

## 3. Configuration Snippet

クラスタに適用される `[settlement.repo]` TOML ブロックを貼り付けます (`collateral_substitution_matrix` を含む)。hash を snippet の隣に保存して、repo booking 承認時に有効だった runtime policy を監査人が確認できるようにします。

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Post-Approval Configuration Snapshots

referendum もしくは governance vote が完了し、`[settlement.repo]` 変更がロールアウトされた後、各 peer の `/v1/configuration` snapshots を取得します。監査人が承認済みポリシーがクラスタ全体で有効であることを証明できるようにします (`docs/source/finance/repo_ops.md` sec 2.9 参照)。

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | File | SHA-256 | Block height | Notes |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | config rollout 直後に取得した snapshot。 |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` が staged TOML と一致することを確認。 |

digests を `hashes.txt` (または同等の summary) に peer ids と並べて記録し、どのノードが変更を取り込んだか追跡できるようにします。snapshots は TOML snippet の隣の `config/peers/` に置かれ、`scripts/repo_evidence_manifest.py` が自動収集します。

## 4. Deterministic Test Artefacts

次の最新 outputs を添付してください:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

CI が生成する log bundles や JUnit XML のファイルパス + hashes を記録します。

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lifecycle proof log | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` 出力で取得。 |
| Integration test log | `tests/repo_integration.log` | `<sha256>` | substitution + margin cadence を含む。 |

## 5. Lifecycle Proof Snapshot

すべての packet には `repo_deterministic_lifecycle_proof_matches_fixture` から出力された deterministic lifecycle snapshot を含めます。export knobs を有効にして harness を実行し、reviewers が JSON frame と digest を `crates/iroha_core/tests/fixtures/` の fixture と比較できるようにします (`docs/source/finance/repo_ops.md` sec 2.7 参照)。

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

または、固定された helper を使って fixtures を再生成し、evidence bundle に一括コピーします:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | proof harness が出力する canonical lifecycle frame。 |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` の uppercase hex digest を反映; 未変更でも添付。 |

## 6. Evidence Manifest

evidence ディレクトリ全体の manifest を生成して、監査人がアーカイブを展開せずに hashes を検証できるようにします。helper は `docs/source/finance/repo_ops.md` sec 3.2 のワークフローを反映します。

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | governance ticket / referendum notes に checksum を記録。 |

## 7. Telemetry & Event Snapshot

`AccountEvent::Repo(*)` の関連エントリと、`docs/source/finance/repo_ops.md` で参照されるダッシュボードや CSV exports をエクスポートします。レビュー担当が evidence にすぐ辿れるよう、ファイルと hashes をここに記録してください。

| Export | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | desk アカウントに絞り込んだ raw Torii event stream。 |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Grafana の Repo Margin パネルからの export。 |

## 8. Approvals & Signatures

- **Dual-control signers:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` (署名済み GAR PDF または minutes upload)
- **Storage location:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

完了したら各項目にチェックを入れてください。

- [ ] Instruction payloads を staged/hashed/添付済み。
- [ ] Configuration snippet の hash を記録。
- [ ] Deterministic test logs を取得 + hash。
- [ ] Lifecycle snapshot + digest をエクスポート。
- [ ] Evidence manifest を生成し hash を記録。
- [ ] Event/telemetry exports を取得 + hash。
- [ ] Dual-control acknowledgements をアーカイブ。
- [ ] GAR/minutes をアップロードし digest を上記に記録。

このテンプレートを各 packet に保持することで、governance DAG が決定的になり、repo ライフサイクルの意思決定のための portable manifest を監査人に提供できます。
