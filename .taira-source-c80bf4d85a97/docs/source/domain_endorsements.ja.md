---
lang: ja
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ドメインの承認

ドメインの承認により、運営者は委員会の署名付き声明に基づいてドメインの作成と再利用をゲートできるようになります。エンドースメント ペイロードはチェーン上に記録される Norito オブジェクトであるため、クライアントは誰がどのドメインをいつ証明したかを監査できます。

## ペイロードの形状

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: 正規ドメイン識別子
- `committee_id`: 人間が読める委員会ラベル
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: ブロックの高さの境界の有効性
- `scope`: オプションのデータスペースとオプションの `[block_start, block_end]` ウィンドウ (これを含む)。受け入れ可能なブロックの高さを**カバーする必要があります**
- `signatures`: `body_hash()` に対する署名 (`signatures = []` による承認)
- `metadata`: オプションの Norito メタデータ (プロポーザル ID、監査リンクなど)

## 施行

- Nexus が有効で `nexus.endorsement.quorum > 0` の場合、またはドメインごとのポリシーでドメインが必須としてマークされている場合は、承認が必要です。
- 検証では、ドメイン/ステートメントのハッシュ バインディング、バージョン、ブロック ウィンドウ、データスペース メンバーシップ、有効期限/年齢、および委員会のクォーラムが強制されます。署名者は、`Endorsement` ロールを持つライブ コンセンサス キーを持っている必要があります。リプレイは `body_hash` によって拒否されます。
- ドメイン登録に添付された承認には、メタデータ キー `endorsement` が使用されます。同じ検証パスが `SubmitDomainEndorsement` 命令でも使用され、新しいドメインを登録せずに監査の承認を記録します。

## 委員会と政策

- 委員会はオンチェーンに登録することも (`RegisterDomainCommittee`)、構成のデフォルトから派生することもできます (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`、id = `default`)。
- ドメインごとのポリシーは、`SetDomainEndorsementPolicy` (委員会 ID、`max_endorsement_age`、`required` フラグ) を介して構成されます。存在しない場合は、Nexus のデフォルトが使用されます。

## CLI ヘルパー

- 承認を作成/署名します (Norito JSON を標準出力に出力します):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- 承認を提出します:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- ガバナンスの管理:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

検証が失敗すると、安定したエラー文字列 (クォーラムの不一致、古い/期限切れの承認、スコープの不一致、不明なデータスペース、委員会の欠落) が返されます。