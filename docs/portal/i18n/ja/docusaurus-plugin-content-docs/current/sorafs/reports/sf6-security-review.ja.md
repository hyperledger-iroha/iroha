---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/reports/sf6-security-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f13e19bcff89607282ec1b1f354678d6398610741bd251b8dd313c0374da774
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SF-6 セキュリティレビュー

**評価期間:** 2026-02-10 → 2026-02-18  
**レビュー担当:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**スコープ:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`)、proof streaming APIs、Torii の manifest 処理、Sigstore/OIDC 統合、CI の release hooks。  
**アーティファクト:**  
- CLI ソースとテスト (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii の manifest/proof handlers (`crates/iroha_torii/src/sorafs/api.rs`)  
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministic parity harness (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA パリティレポート](./orchestrator-ga-parity.md))

## Methodology

1. **Threat modelling workshops** で developer ワークステーション、CI システム、Torii ノードの攻撃者能力をマッピング。  
2. **Code review** で credential surface (OIDC token exchange, keyless signing)、Norito manifest 検証、proof streaming の back-pressure を重点的に確認。  
3. **Dynamic testing** で fixture manifests の再生と故障モード (token replay, manifest tampering, proof stream の切断) を parity harness と専用 fuzz drives で検証。  
4. **Configuration inspection** で `iroha_config` defaults、CLI flags、release scripts を確認し、決定論的で監査可能な実行を保証。  
5. **Process interview** で remediation flow、escalation paths、audit evidence の捕捉を Tooling WG の release owners と確認。

## Findings Summary

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Keyless signing | OIDC token audience defaults が CI templates で暗黙になっており、cross-tenant replay のリスク。 | Release hooks と CI templates に `--identity-token-audience` の明示的強制を追加 ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`)。CI は audience が欠けると失敗。 |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths が無制限の subscriber buffers を受け入れ、メモリ枯渇が可能。 | `sorafs_cli proof stream` が bounded channel サイズを強制し、決定論的 truncation を行い、Norito summaries を記録してストリームを中断。Torii mirror も response chunks を制限 (`crates/iroha_torii/src/sorafs/api.rs`)。 |
| SF6-SR-03 | Medium | Manifest submission | `--plan` 不在時に CLI が embedded chunk plans を検証せずに manifests を受理。 | `sorafs_cli manifest submit` が `--expect-plan-digest` 未指定なら CAR digests を再計算して比較し、mismatch を拒否して remediation hints を表示。テストは成功/失敗ケースをカバー (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 | Low | Audit trail | Release checklist に security review の署名付き承認ログが欠落。 | [release process](../developer-releases.md) にセクションを追加し、GA 前に review memo hashes と sign-off チケット URL の添付を必須化。 |

High/Medium の所見はレビュー期間中に修正され、既存の parity harness で検証済み。潜在的な critical は残っていない。

## Control Validation

- **Credential scope:** CI templates は explicit audience/issuer を必須化。CLI と release helper は `--identity-token-audience` が `--identity-token-provider` を伴わない場合に fail fast。  
- **Deterministic replay:** 更新テストは manifest submission の正/負フローをカバーし、mismatch digests が非決定論的な失敗として残り、ネットワークに触れる前に検出される。  
- **Proof streaming back-pressure:** Torii は PoR/PoTR items を bounded channels でストリームし、CLI は切り詰めた latency サンプル + 5 件の failure 例のみ保持。無制限の subscriber 増殖を防ぎ、決定論的 summaries を維持。  
- **Observability:** Proof streaming counters (`torii_sorafs_proof_stream_*`) と CLI summaries が abort 理由を記録し、監査の breadcrumbs を提供。  
- **Documentation:** Developer guides ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) が security-sensitive flags と escalation workflows を明記。

## Release Checklist Additions

Release managers は GA candidate を昇格する際、以下の evidence を**必ず**添付する:

1. 最新の security review memo の hash (本書)。  
2. Remediation ticket へのリンク (例: `governance/tickets/SF6-SR-2026.md`)。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` の output (explicit audience/issuer arguments を含む)。  
4. Parity harness のログ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. Torii release notes に bounded proof streaming telemetry counters が含まれていることの確認。

上記アーティファクトを収集できない場合、GA sign-off はブロックされる。

**Reference artefact hashes (2026-02-20 sign-off):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Outstanding Follow-ups

- **Threat model refresh:** 四半期ごと、または CLI の大きな flag 追加前に再レビューする。  
- **Fuzzing coverage:** Proof streaming transport encodings は `fuzz/proof_stream_transport` で fuzzing 済み。identity, gzip, deflate, zstd payloads をカバー。  
- **Incident rehearsal:** Token compromise と manifest rollback を想定したオペレーター演習を計画し、ドキュメントに実践手順を反映する。

## Approval

- Security Engineering Guild 代表: @sec-eng (2026-02-20)  
- Tooling Working Group 代表: @tooling-wg (2026-02-20)

署名済み approvals は release artefact bundle と一緒に保管する。
