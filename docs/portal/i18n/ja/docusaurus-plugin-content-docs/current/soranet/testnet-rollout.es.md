---
lang: es
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55d3ae00e6d7fb2e93be7bf75f68f63825f343224c8dea1544b75a3265412cd4
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: testnet-rollout
lang: ja
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
このページは `docs/source/soranet/testnet_rollout_plan.md` にある SNNet-10 rollout plan を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

SNNet-10 は SoraNet anonymity overlay の段階的な有効化をネットワーク全体で調整する。roadmap の bullet を具体的な deliverables、runbooks、telemetry gates に落とし込み、SoraNet がデフォルト transport になる前に全ての operator が期待事項を理解できるようにするため、この計画を使う。

## Launch phases

| Phase | Timeline (target) | Scope | Required artefacts |
|-------|-------------------|-------|--------------------|
| **T0 - Closed Testnet** | Q4 2026 | >=3 ASNs で運用される 20-50 relays（core contributors 管理）。 | Testnet onboarding kit、guard pinning smoke suite、baseline latency + PoW metrics、brownout drill log。 |
| **T1 - Public Beta** | Q1 2027 | >=100 relays、guard rotation 有効、exit bonding 強制、SDK betas は `anon-guard-pq` で SoraNet をデフォルト。 | 更新済み onboarding kit、operator verification checklist、directory publishing SOP、telemetry dashboard pack、incident rehearsal reports。 |
| **T2 - Mainnet Default** | Q2 2027 (SNNet-6/7/9 completion を条件にゲート) | 本番ネットワークは SoraNet がデフォルト; obfs/MASQUE transports と PQ ratchet enforcement を有効化。 | Governance approval minutes、direct-only rollback procedure、downgrade alarms、署名済み success metrics report。 |

**スキップパスは存在しない**。各フェーズは前段階の telemetry と governance artefacts を出荷してから昇格する必要がある。

## Testnet onboarding kit

各 relay operator は次のファイルを含む deterministic package を受け取る:

| Artefact | Description |
|----------|-------------|
| `01-readme.md` | 概要、連絡先、タイムライン。 |
| `02-checklist.md` | Pre-flight checklist（hardware、network reachability、guard policy verification）。 |
| `03-config-example.toml` | SNNet-9 compliance blocks に合わせた最小 SoraNet relay + orchestrator config。最新の guard snapshot hash を pin する `guard_directory` ブロックを含む。 |
| `04-telemetry.md` | SoraNet privacy metrics dashboards と alert thresholds を接続する手順。 |
| `05-incident-playbook.md` | Brownout/downgrade 対応手順と escalation matrix。 |
| `06-verification-report.md` | Smoke tests 合格後に operator が記入して返送するテンプレート。 |

レンダリング済みコピーは `docs/examples/soranet_testnet_operator_kit/` にある。昇格ごとに kit を更新し、version 番号はフェーズに追随する（例: `testnet-kit-vT0.1`）。

Public-beta (T1) 向けには、`docs/source/soranet/snnet10_beta_onboarding.md` にある concise onboarding brief が prerequisites、telemetry deliverables、submission workflow をまとめ、deterministic kit と validator helpers への参照を付けている。

`cargo xtask soranet-testnet-feed` は promotion window、relay roster、metrics report、drill evidence、stage-gate template が参照する attachment hashes を集約した JSON feed を生成する。`cargo xtask soranet-testnet-drill-bundle` で drill logs と attachments を先に署名し、feed が `drill_log.signed = true` を記録できるようにする。

## Success metrics

フェーズ間の昇格は、少なくとも 2 週間収集した次の telemetry によってゲートされる:

- `soranet_privacy_circuit_events_total`: circuits の 95% が brownout/downgrade なしで完了; 残り 5% は PQ 供給により制限。
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: 1 日あたりの fetch sessions の <1% が予定外の brownout を発火。
- `soranet_privacy_gar_reports_total`: 期待される GAR category mix からの変動が +/-10% 以内; スパイクは承認済み policy updates で説明される必要がある。
- PoW ticket success rate: 3 s の目標ウィンドウ内で >=99%; `soranet_privacy_throttles_total{scope="congestion"}` で報告。
- Latency (95th percentile) per region: circuits が完全に構築された後は <200 ms; `soranet_privacy_rtt_millis{percentile="p95"}` で収集。

Dashboard と alert templates は `dashboard_templates/` と `alert_templates/` にある。telemetry repository にミラーし、CI lint checks に追加すること。昇格申請前に `cargo xtask soranet-testnet-metrics` で governance 向けレポートを生成する。

Stage-gate submissions は `docs/source/soranet/snnet10_stage_gate_template.md` に従うこと。テンプレートは `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` にある。

## Verification checklist

各フェーズに入る前に、operators は以下をサインオフする:

- ✅ Relay advert が current admission envelope で署名済み。
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) が合格。
- ✅ `guard_directory` が最新の `GuardDirectorySnapshotV2` artefact を指し、`expected_directory_hash_hex` が committee digest と一致（relay startup が検証済み hash をログ出力）。
- ✅ PQ ratchet metrics (`sorafs_orchestrator_pq_ratio`) が要求フェーズの target thresholds を上回る。
- ✅ GAR compliance config が最新タグと一致（SNNet-9 catalogue を参照）。
- ✅ Downgrade alarm simulation（collectors を無効化し、5 分以内に alert を期待）。
- ✅ PoW/DoS drill を実施し、mitigation steps を文書化。

事前入力済みテンプレートは onboarding kit に含まれる。operators は完了したレポートを governance helpdesk に提出してから production credentials を受け取る。

## Governance & reporting

- **Change control:** promotions には Governance Council の承認が必要で、council minutes に記録し status page に添付する。
- **Status digest:** relay count、PQ ratio、brownout incidents、未完了 action items を要約した週次更新を公開する（cadence 開始後は `docs/source/status/soranet_testnet_digest.md` に保存）。
- **Rollbacks:** DNS/guard cache invalidation と client communication templates を含め、30 分以内に前フェーズへ戻す署名済み rollback plan を維持する。

## Supporting assets

- `cargo xtask soranet-testnet-kit [--out <dir>]` は `xtask/templates/soranet_testnet/` から onboarding kit を生成し、指定ディレクトリへ出力する（デフォルトは `docs/examples/soranet_testnet_operator_kit/`）。
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` は SNNet-10 success metrics を評価し、governance review に適した structured pass/fail report を出力する。サンプルは `docs/examples/soranet_testnet_metrics_sample.json` にある。
- Grafana と Alertmanager の templates は `dashboard_templates/soranet_testnet_overview.json` と `alert_templates/soranet_testnet_rules.yml` にある。telemetry repository にコピーするか、CI lint checks に接続する。
- SDK/portal messaging 向けの downgrade communication template は `docs/source/soranet/templates/downgrade_communication_template.md` にある。
- 週次 status digests は `docs/source/status/soranet_testnet_weekly_digest.md` を canonical form として使う。

Pull requests は artefact や telemetry の変更と合わせてこのページを更新し、rollout plan を canonical に保つこと。
