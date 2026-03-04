---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 658c8cb512811861fd937c9e54be42fe270b3d32c0a81b08be15785169725db1
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: transport
lang: ja
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
このページは `docs/source/soranet/spec.md` にある SNNet-1 transport 仕様を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

SoraNet は SoraFS の range fetch、Norito RPC streaming、将来の Nexus data lanes を支える匿名オーバーレイ。transport program（roadmap items **SNNet-1**、**SNNet-1a**、**SNNet-1b**）は決定的な handshake、post-quantum (PQ) capability negotiation、salt rotation plan を定義し、すべての relay、client、gateway が同じ security posture を観測できるようにする。

## 目標とネットワークモデル

- QUIC v1 上で 3-hop circuits (entry -> middle -> exit) を構築し、悪用 peer が Torii に直接到達しないようにする。
- QUIC/TLS の上に Noise XX *hybrid* handshake (Curve25519 + Kyber768) を重ね、session keys を TLS transcript に束縛する。
- PQ KEM/signature のサポート、relay role、protocol version を広告する capability TLVs を必須化し、未知の type は GREASE して将来の拡張を継続的に展開できるようにする。
- blinded-content salts を毎日 rotation し、guard relays を 30 日固定して directory churn が clients を匿名解除できないようにする。
- cells を 1024 B に固定し、padding/dummy cells を注入し、deterministic telemetry を出力して downgrade 試行を素早く検知する。

## Handshake pipeline (SNNet-1a)

1. **QUIC/TLS envelope** - clients は QUIC v1 で relays に接続し、governance CA が署名した Ed25519 certificates を使って TLS 1.3 handshake を完了する。TLS exporter (`tls-exporter("soranet handshake", 64)`) が Noise layer に種を渡し、transcripts を不可分にする。
2. **Noise XX hybrid** - protocol string `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` を使用し、prologue = TLS exporter。メッセージフロー:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH の出力と 2 つの Kyber encapsulation が最終的な symmetric keys に混ぜ込まれる。PQ material の交渉に失敗した場合、handshake は即座に中止され、classical-only の fallback は許可されない。

3. **Puzzle tickets & tokens** - relays は `ClientHello` の前に Argon2id proof-of-work ticket を要求できる。tickets は length-prefixed frames で、hash 済みの Argon2 solution を運び、policy の範囲内で失効する:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   `SNTK` で prefix された admission tokens は、issuer の ML-DSA-44 signature が active policy と revocation list に合致するときに puzzles をバイパスする。

4. **Capability TLV exchange** - 最終の Noise payload が以下の capability TLVs を運ぶ。必須 capability (PQ KEM/signature、role、version) が欠落していたり directory entry と不一致なら、clients は接続を中断する。

5. **Transcript logging** - relays は transcript hash、TLS fingerprint、TLV contents をログに残し、downgrade detectors と compliance pipelines に供給する。

## Capability TLVs (SNNet-1c)

Capabilities は `typ/length/value` の固定 TLV envelope を再利用する:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

現在定義されている types:

- `snnet.pqkem` - Kyber level（現行 rollout は `kyber768`）。
- `snnet.pqsig` - PQ signature suite（`ml-dsa-44`）。
- `snnet.role` - relay role（`entry`, `middle`, `exit`, `gateway`）。
- `snnet.version` - protocol version identifier。
- `snnet.grease` - 予約レンジに入るランダム filler で、将来 TLVs を許容させる。

Clients は必須 TLVs の allow-list を持ち、欠落や downgrade があれば handshake を失敗させる。Relays は同じ set を directory microdescriptor に公開し、validation を決定的にする。

## Salt rotation & CID blinding (SNNet-1b)

- Governance は `(epoch_id, salt, valid_after, valid_until)` を持つ `SaltRotationScheduleV1` レコードを公開する。Relays と gateways は directory publisher から署名済み schedule を取得する。
- Clients は `valid_after` で新しい salt を適用し、旧 salt を 12 h の grace period 保持し、遅延更新に耐えるため 7 epoch の履歴を保管する。
- Canonical blinded identifiers は次を使う:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways は `Sora-Req-Blinded-CID` で blinded key を受け取り、`Sora-Content-CID` でエコーする。Circuit/request blinding (`CircuitBlindingKey::derive`) は `iroha_crypto::soranet::blinding` にある。
- Relay が epoch を取り逃した場合、新規 circuits を停止し、schedule を取得した後に `SaltRecoveryEventV1` を発行する。オンコール dashboards はこれを paging signal として扱う。

## Directory data と guard policy

- Microdescriptors は relay identity (Ed25519 + ML-DSA-65)、PQ keys、capability TLVs、region tags、guard eligibility、現在アドバタイズする salt epoch を持つ。
- Clients は guard sets を 30 日固定し、署名済み directory snapshot と一緒に `guard_set` caches を永続化する。CLI/SDK wrappers は cache fingerprint を出し、rollout evidence を change review に添付できるようにする。

## Telemetry & rollout checklist

- 本番前に export する metrics:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Alert thresholds は salt rotation SOP の SLO matrix (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) と並置され、ネットワークを昇格する前に Alertmanager に反映する必要がある。
- Alerts: 5 分間で failure rate >5%、salt lag >15 分、または production で capability mismatches を観測。
- Rollout steps:
  1. hybrid handshake と PQ stack を有効化した staging で relay/client interoperability tests を実施する。
  2. salt rotation SOP (`docs/source/soranet_salt_plan.md`) をリハーサルし、drill artifacts を change record に添付する。
  3. directory で capability negotiation を有効化し、entry relays、middle relays、exit relays、最後に clients へ展開する。
  4. 各フェーズの guard cache fingerprints、salt schedules、telemetry dashboards を記録し、evidence bundle を `status.md` に添付する。

この checklist に従うことで、operator、client、SDK チームが SoraNet transports を足並みを揃えて導入し、SNNet roadmap に記された determinism と audit 要件を満たせる。
