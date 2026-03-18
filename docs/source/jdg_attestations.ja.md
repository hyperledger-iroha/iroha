---
lang: ja
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JDG 認証: ガード、ローテーション、保持

このノートでは、現在 `iroha_core` で出荷されている v1 JDG 構成証明ガードについて説明します。

- **委員会マニフェスト:** Norito でエンコードされた `JdgCommitteeManifest` バンドルはデータスペースごとのローテーションを実行します
  スケジュール (`committee_id`、順序付けされたメンバー、しきい値、`activation_height`、`retire_height`)。
  マニフェストには `JdgCommitteeSchedule::from_path` がロードされ、厳密に増加することが強制されます。
  アクティブ化の高さ (リタイア/アクティブ化の間のオプションの猶予オーバーラップ (`grace_blocks`))
  委員会。
- **構成証明ガード:** `JdgAttestationGuard` は、データスペースのバインディング、有効期限、古い境界を強制します。
  委員会 ID/しきい値の照合、署名者のメンバーシップ、サポートされている署名スキーム、およびオプション
  `JdgSdnEnforcer` による SDN 検証。サイズの上限、最大遅延、および許可される署名スキームは次のとおりです。
  コンストラクターのパラメーター。 `validate(attestation, dataspace, current_height)` はアクティブな
  委員会または構造的エラー。
  - `scheme_id = 1` (`simple_threshold`): 署名者ごとの署名、オプションの署名者ビットマップ。
  - `scheme_id = 2` (`bls_normal_aggregate`): 単一の事前集約された BLS 通常署名
    証明書ハッシュ。署名者ビットマップはオプションで、デフォルトは証明書内のすべての署名者です。 BLS
    集計の検証には、マニフェスト内の委員会メンバーごとの有効な PoP が必要です。行方不明または
    無効な PoP は認証を拒否します。
  `governance.jdg_signature_schemes` を介して許可リストを構成します。
- **保持ストア:** `JdgAttestationStore` は、構成可能なデータスペースごとに証明書を追跡します。
  データスペースごとの上限。挿入時に最も古いエントリを削除します。 `for_dataspace` に電話するか、
  `for_dataspace_and_epoch` は監査/再生バンドルを取得します。
- **テスト:** ユニット カバレッジは有効な委員会の選択、不明な署名者の拒否、古いものを実行するようになりました。
  構成証明の拒否、サポートされていないスキーム ID、および保持のプルーニング。参照
  `crates/iroha_core/src/jurisdiction.rs`。

ガードは、設定された許可リスト外のスキームを拒否します。