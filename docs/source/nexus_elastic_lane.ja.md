---
lang: ja
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

# Elastic Lane Provisioning Toolkit (NX-7)

> **ロードマップ項目:** NX-7 — Elastic lane provisioning tooling  
> **ステータス:** Tooling 完了 — manifests、catalog snippets、Norito payloads、smoke tests を生成し、
> load-test bundle helper は slot latency gating + evidence manifests を統合して、バリデータの
> load run を bespoke scripting なしで公開できるようになりました。

このガイドは、`scripts/nexus_lane_bootstrap.sh` helper を使って lane manifest 生成、
lane/dataspace catalog snippets、rollout evidence を自動化する手順を説明します。
目的は、Nexus lanes (public/private) を複数ファイルの手編集や catalog geometry の再計算なしで
素早く立ち上げることです。

## 1. 前提条件

1. lane alias、dataspace、validator set、fault tolerance (`f`)、settlement policy のガバナンス承認。
2. 最終版の validator 一覧 (account IDs) と protected namespace 一覧。
3. 生成された snippet を追記できる node config リポジトリへのアクセス。
4. lane manifest registry のパス (`nexus.registry.manifest_directory` と `cache_directory`)。
5. lane 用の telemetry/PagerDuty 連絡先 (有効化と同時にアラートを配線するため)。

## 2. Lane artifacts の生成

リポジトリのルートから helper を実行します:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

主なフラグ:

- `--lane-id` は `nexus.lane_catalog` の新しいエントリの index と一致する必要があります。
- `--dataspace-alias` と `--dataspace-id/hash` は dataspace catalog entry を制御します
  (省略時は lane id を使用)。
- `--validator` は繰り返し指定、または `--validators-file` から取得可能です。
- `--route-instruction` / `--route-account` は貼り付け可能な routing rules を出力します。
- `--metadata key=value` (または `--telemetry-contact/channel/runbook`) は runbook 連絡先を
  記録し、dashboards に正しい owners を表示します。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` は、lane に拡張された operator control が
  必要な場合に runtime-upgrade hook を manifest に追加します。
- `--encode-space-directory` は `cargo xtask space-directory encode` を自動実行します。別の
  出力先にしたい場合は `--space-directory-out` を使います。

スクリプトは `--output-dir` (デフォルトはカレントディレクトリ) に 3 つの artifacts を生成し、
encoding が有効な場合は 4 つ目も生成します:

1. `<slug>.manifest.json` — validator quorum、protected namespaces、任意の runtime-upgrade hook
   metadata を含む lane manifest。
2. `<slug>.catalog.toml` — `[[nexus.lane_catalog]]` と `[[nexus.dataspace_catalog]]`、および
   要求された routing rules を含む TOML snippet。dataspace entry に `fault_tolerance` を設定し、
   lane-relay committee を `3f+1` に合わせます。
3. `<slug>.summary.json` — geometry (slug/segments/metadata) と rollout steps、さらに
   `cargo xtask space-directory encode` の正確なコマンド ( `space_directory_encode.command` ) を
   含む audit summary。onboarding ticket に証拠として添付します。
4. `<slug>.manifest.to` — `--encode-space-directory` 時に生成。Torii の
   `iroha app space-directory manifest publish` 用。

`--dry-run` で JSON/snippets を出力だけ確認でき、`--force` で既存 artifacts を上書きできます。

## 3. 変更の適用

1. manifest JSON を設定済み `nexus.registry.manifest_directory` にコピーします
   (registry が remote bundles を mirror する場合は cache directory にもコピー)。
   manifests を config repo で管理している場合は commit します。
2. catalog snippet を `config/config.toml` (または `config.d/*.toml`) に追記します。
   `nexus.lane_count` が `lane_id + 1` 以上であることを確認し、必要な
   `nexus.routing_policy.rules` を更新します。
3. `--encode-space-directory` を使っていない場合は、summary に記載の
   `space_directory_encode.command` で manifest を encode し、Space Directory に publish します。
   これにより Torii が期待する `.manifest.to` が生成され、監査用 evidence が残ります。
4. `irohad --sora --config path/to/config.toml --trace-config` を実行し、trace 出力を rollout
   ticket にアーカイブします。これにより新しい geometry が生成された slug/segments と
   一致することが証明されます。
5. manifest/catalog 変更をデプロイ後、対象の validators を再起動します。
   summary JSON を ticket に保持してください。

## 4. Registry distribution bundle の作成

manifest、catalog snippet、summary が揃ったら、validators への配布用にパッケージ化します。
新しい bundler は `nexus.registry.manifest_directory` / `cache_directory` が期待する layout に
manifests をコピーし、main config を触らずに modules を差し替えられる governance catalog
overlay を出力し、必要なら bundle をアーカイブします:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

生成物:

1. `manifests/<slug>.manifest.json` — `nexus.registry.manifest_directory` に配置。
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory` に配置し、governance
   modules を上書き/交換します (`--module ...` は cached catalog を上書き)。
   NX-2 の pluggable module path で、module 定義を置き換えて bundler を再実行し、
   cache overlay を配布するだけで `config.toml` を触らずに更新できます。
3. `summary.json` — 各 manifest の SHA-256 / Blake2b digests と overlay metadata。
4. 省略可能な `registry_bundle.tar.*` — Secure Copy / artifacts storage 用。

air-gapped 環境に bundles をミラーする場合は、出力ディレクトリ全体 (または tarball) を
同期します。オンラインノードは manifest directory を直接マウントでき、オフラインノードは
tarball を展開して manifests と cache overlay を指定パスへコピーします。

## 5. Validator smoke tests

Torii 再起動後、smoke helper を実行して `manifest_ready=true` を確認し、metrics が
期待 lane count を出していること、sealed gauge がクリアであることを確認します。
manifest が必須の lanes は `manifest_path` が空であってはならず、欠落時は helper が
即座に失敗して NX-7 change control に署名済み bundle 参照が含まれるようにします:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

self-signed 環境では `--insecure` を追加します。lane 不在、sealed、あるいは
metrics/telemetry が期待値から逸脱すると non-zero で終了します。新しい
`--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog`、`--max-headroom-events`
で height/finality/backlog/headroom の telemetry を運用範囲に収めます。さらに
`--max-slot-p95/--max-slot-p99` と `--min-slot-samples` を使って NX-18 slot-duration SLO を
helper 内で直接適用できます。staging で metrics が未公開の場合のみ
`--allow-missing-lane-metrics` を使い、production では defaults を保持します。

同じ helper で scheduler load-test telemetry もチェックします。`--min-teu-capacity` で
`nexus_scheduler_lane_teu_capacity` を確認し、`--max-teu-slot-commit-ratio` で
`nexus_scheduler_lane_teu_slot_committed` と capacity の比率をゲートします。
`--max-teu-deferrals` と `--max-must-serve-truncations` で deferral/truncation カウンタを
ゼロに保ちます。これらの knobs により NX-7 の "deeper validator load tests" 要件が
再現可能な CLI check になります。helper は PQ/TEU の deferral や per-slot TEU が
headroom を超える場合に失敗し、CLI が lane ごとの概要を出力するため、evidence packets
は CI と同じ値を記録できます。

air-gapped 検証 (または CI) では live node を叩かずに Torii のキャプチャ結果を再生できます:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` の fixtures は bootstrap helper が生成する artifacts を再現し、
新しい manifests を bespoke scripting なしで lint できるようにします。CI は
`ci/check_nexus_lane_smoke.sh` と `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`)
で同じフローを実行し、NX-7 smoke helper が公開 payload 形式と互換であること、
 bundle digests/overlays が再現可能であることを確認します。

lane のリネーム時は `nexus.lane.topology` telemetry イベントを収集し
(例: `journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`)、
smoke helper に渡します。`--telemetry-file/--from-telemetry` は newline-delimited log を受け取り、
`--require-alias-migration old:new` は `alias_migrated` イベントがリネームを記録したことを
検証します:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` fixture には canonical な rename サンプルが含まれており、
CI が live node に触れずに telemetry parsing を検証できます。

## 6. Validator load tests (NX-7 evidence)

Roadmap **NX-7** は、lane を production ready とする前に再現可能な validator load run の
取得を要求します。目的は lane を十分に負荷して slot duration、settlement backlog、DA
quorum、oracle、scheduler headroom、TEU metrics を検証し、監査者が bespoke tooling なしで
再生できるように結果をアーカイブすることです。`scripts/nexus_lane_load_test.py` helper は
smoke checks、slot-duration gating、slot bundle manifest を 1 セットにまとめ、load runs を
governance ticket に直接公開できるようにします。

### 6.1 Workload preparation

1. run ディレクトリを作成し、対象 lane の canonical fixtures を取得します:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   Fixtures は `fixtures/nexus/lane_commitments/*.json` を反映し、workload generator に
   deterministic seed を提供します (`artifacts/.../README.md` に seed を記録してください)。
2. run 前に lane を baseline:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   stdout/stderr を run ディレクトリに保持し、smoke thresholds の監査性を確保します。
3. `--telemetry-file` (alias migration evidence) と `validate_nexus_telemetry_pack.py` 用の
   telemetry log を取得します:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. workload を開始 (k6 profile、replay harness、または federation ingestion tests) し、
   workload seed + slot range を控えておきます。metadata は 6.3 の telemetry manifest
   validator で使用されます。

5. 新しい helper で load run evidence をパッケージ化します。status/metrics/telemetry の
   payload と lane aliases、必要な alias migration を渡します。helper は `smoke.log`、
   `slot_summary.json`、slot bundle manifest、`load_test_manifest.json` を生成し、
   governance review 用の一式にします:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   このコマンドはガイド内の DA quorum、oracle、settlement buffer、TEU、slot-duration gates を
   同じく適用し、bespoke scripting なしで添付可能な manifest を生成します。

### 6.2 Instrumented run

workload の飽和中に:

1. Torii status + metrics を snapshot:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. slot-duration quantiles を計算し summary を保存:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. lane-governance snapshot を JSON + Parquet で export:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   JSON/Parquet snapshot は TEU utilization、scheduler trigger levels、RBC chunk/byte counters、
   transaction graph stats を lane ごとに記録し、rollout evidence が backlog と実行圧の両方を
   示します。

4. ピーク時に smoke helper を再実行して thresholds を確認 (`smoke_during.log`) し、
   workload 完了後に再度実行 (`smoke_after.log`) します。

### 6.3 Telemetry pack と governance manifest

run ディレクトリには telemetry pack (`prometheus.tgz`, OTLP stream, structured logs, harness
出力) を含める必要があります。pack を検証し、governance が期待する metadata を stamp します:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

最後に、収集した telemetry log を添付し、lane のリネームがある場合は alias migration
証拠を要求します:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

governance ticket に以下の artifacts をアーカイブします:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + pack contents (`prometheus.tgz`, `otlp.ndjson`, etc.)
- `nexus.lane.topology.ndjson` (または関連 telemetry slice)

この run は Space Directory manifests や governance trackers で NX-7 の canonical load test として
参照できます。

## 7. Telemetry と governance follow-ups

- 新しい lane id と metadata で dashboards (`dashboards/grafana/nexus_lanes.json` など) を更新します。
  `contact`, `channel`, `runbook` などの keys が labels の事前設定を容易にします。
- admission を有効化する前に PagerDuty/Alertmanager ルールを接続します。`summary.json` は
  `docs/source/nexus_operations.md` の checklist を反映しています。
- validator set が live になったら Space Directory に manifest bundle を登録します。helper が
  生成した同一の manifest JSON を、governance runbook に従って署名して使用します。
- `docs/source/sora_nexus_operator_onboarding.md` に従って smoke tests (FindNetworkStatus,
  Torii reachability) を実施し、上記 artifacts セットで evidence を収集します。

## 8. Dry-run の例

ファイルを書き込まずに artifacts をプレビューするには:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

このコマンドは JSON summary と TOML snippet を stdout に出力し、計画時の素早い反復を可能にします。

---

追加情報:

- `docs/source/nexus_operations.md` — オペレーション checklist と telemetry 要件。
- `docs/source/sora_nexus_operator_onboarding.md` — onboarding の詳細フロー (本 helper を参照)。
- `docs/source/nexus_lanes.md` — lanes geometry、slugs、tool が使う storage layout。
