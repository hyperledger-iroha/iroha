---
lang: ja
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# アジェンダ評議会提案スキーマ (MINFO-2a)

ロードマップ参照: **MINFO-2a — 提案書式検証ツール。**

アジェンダ評議会のワークフローは、市民が提出したブラックリストと政策変更をバッチ処理します。
提案はガバナンスパネルがレビューする前に提出されます。この文書では、
正規ペイロード スキーマ、証拠要件、および重複検出ルール
新しいバリデータ (`cargo xtask ministry-agenda validate`) によって消費されるため、
提案者は、JSON 送信をポータルにアップロードする前に、ローカルで lint を実行できます。

## ペイロードの概要

アジェンダ提案は `AgendaProposalV1` Norito スキーマを使用します
(`iroha_data_model::ministry::AgendaProposalV1`)。フィールドは次の場合に JSON としてエンコードされます。
CLI/ポータル画面を通じて送信します。

|フィールド |タイプ |要件 |
|------|------|--------------|
| `version` | `1` (u16) | `AGENDA_PROPOSAL_VERSION_V1` と等しくなければなりません。 |
| `proposal_id` |文字列 (`AC-YYYY-###`) |安定した識別子。検証中に強制されます。 |
| `submitted_at_unix_ms` | u64 | Unix エポックからのミリ秒数。 |
| `language` |文字列 | BCP-47 タグ (`"en"`、`"ja-JP"` など)。 |
| `action` |列挙型 (`add-to-denylist`、`remove-from-denylist`、`amend-policy`) |省の措置を要請した。 |
| `summary.title` |文字列 | 256 文字以下を推奨します。 |
| `summary.motivation` |文字列 |なぜその行動が必要なのか。 |
| `summary.expected_impact` |文字列 |アクションが受け入れられた場合の結果。 |
| `tags[]` |小文字の文字列 |オプションのトリアージラベル。許可される値: `csam`、`malware`、`fraud`、`harassment`、`impersonation`、`policy-escalation`、`terrorism`、`spam`。 |
| `targets[]` |オブジェクト | 1 つ以上のハッシュ ファミリ エントリ (以下を参照)。 |
| `evidence[]` |オブジェクト | 1 つ以上の証拠の添付ファイル (以下を参照)。 |
| `submitter.name` |文字列 |表示名または組織。 |
| `submitter.contact` |文字列 |電子メール、マトリックス ハンドル、または電話。公開ダッシュボードから編集しました。 |
| `submitter.organization` |文字列 (オプション) |レビュー担当者の UI に表示されます。 |
| `submitter.pgp_fingerprint` |文字列 (オプション) | 40 進数の大文字の指紋。 |
| `duplicates[]` |文字列 |以前に送信されたプロポーザル ID へのオプションの参照。 |

### ターゲットエントリ (`targets[]`)

各ターゲットは、プロポーザルによって参照されるハッシュ ファミリー ダイジェストを表します。

|フィールド |説明 |検証 |
|----------|---------------|---------------|
| `label` |レビュー担当者コンテキストのフレンドリ名。 |空ではない。 |
| `hash_family` |ハッシュ識別子 (`blake3-256`、`sha256` など)。 | ASCII 文字/数字/`-_.`、≤48 文字。 |
| `hash_hex` |小文字の 16 進数でエンコードされたダイジェスト。 | ≥16 バイト (32 16 進数文字) で、有効な 16 進数である必要があります。 |
| `reason` |ダイジェストを実行する必要がある理由の短い説明。 |空ではない。 |

バリデーターは、同じ内の重複する `hash_family:hash_hex` ペアを拒否します。
同じフィンガープリントが既に存在する場合、提案とレポートは競合します。
レジストリが重複しています (下記を参照)。

### 証拠の添付ファイル (`evidence[]`)

証拠エントリは、レビュー担当者がサポートするコンテキストを取得できる場所を文書化します。|フィールド |タイプ |メモ |
|------|------|------|
| `kind` |列挙型 (`url`、`torii-case`、`sorafs-cid`、`attachment`)ダイジェスト要件を決定します。 |
| `uri` |文字列 | HTTP(S) URL、Torii ケース ID、または SoraFS URI。 |
| `digest_blake3_hex` |文字列 | `sorafs-cid` および `attachment` タイプには必須。他の人にとってはオプションです。 |
| `description` |文字列 |レビュー担当者用のオプションの自由形式テキスト。 |

### レジストリが重複しています

オペレーターは既存の指紋のレジストリを維持して重複を防ぐことができます
ケース。バリデーターは、次のような形式の JSON ファイルを受け入れます。

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

プロポーザルのターゲットがエントリと一致すると、バリデータは、次の場合を除き中止されます。
`--allow-registry-conflicts` が指定されています (警告は引き続き出力されます)。
[`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) を使用して、
重複を相互参照する国民投票に対応した概要を生成する
レジストリとポリシーのスナップショット。

## CLI の使用法

単一のプロポーザルを lint して、重複したレジストリと照合してチェックします。

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

次の場合に重複ヒットを警告にダウングレードするには、`--allow-registry-conflicts` を渡します。
履歴監査を実行します。

CLI は、同梱されている同じ Norito スキーマと検証ヘルパーに依存します。
`iroha_data_model`。SDK/ポータルは `AgendaProposalV1::validate` を再利用できます。
一貫した動作を実現するためのメソッド。

## 並べ替え CLI (MINFO-2b)

ロードマップの参照: **MINFO-2b — マルチスロットの並べ替えと監査ログ。**

アジェンダ評議会の名簿は決定論的な並べ替えによって管理されるようになりました。
すべての描画を独立して監査できます。新しいコマンドを使用します。

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — すべての対象メンバーを説明する JSON ファイル:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  サンプルファイルは次の場所にあります
  `docs/examples/ministry/agenda_council_roster.json`。オプションのフィールド (役割、
  組織、連絡先、メタデータなど）がマークル リーフに取り込まれるため、監査人は
  抽選に出場した名簿を証明できる。

- `--slots` — 埋める議会の議席の数。
- `--seed` — 32 バイトの BLAKE3 シード (64 個の小文字 16 進文字)
  抽選のためのガバナンス議事録。
- `--out` — オプションの出力パス。省略すると、JSON の概要が出力されます。
  標準出力。

### 出力の概要

このコマンドは、`SortitionSummary` JSON BLOB を出力します。サンプル出力は次の場所に保存されます。
`docs/examples/ministry/agenda_sortition_summary_example.json`。主要なフィールド:

|フィールド |説明 |
|------|-----------|
| `algorithm` |分類ラベル (`agenda-sortition-blake3-v1`)。 |
| `roster_digest` |名簿ファイルの BLAKE3 + SHA-256 ダイジェスト (監査が同じメンバー リストに対して行われることを確認するために使用されます)。 |
| `seed_hex` / `slots` |監査人が描画を再現できるように、CLI 入力をエコーし​​ます。 |
| `merkle_root_hex` |名簿マークル ツリーのルート (`xtask/src/ministry_agenda.rs` の `hash_node`/`hash_leaf` ヘルパー)。 |
| `selected[]` |各スロットのエントリ。正規メンバーのメタデータ、適格なインデックス、元の名簿インデックス、決定論的描画エントロピー、リーフ ハッシュ、およびマークル証明兄弟が含まれます。 |

### 引き分けの確認1. `roster_path` によって参照される名簿を取得し、その BLAKE3/SHA-256 を確認します。
   ダイジェストは概要と一致します。
2. 同じシード/スロット/ロスターを使用して CLI を再実行します。結果の `selected[].member_id`
   順序は公開された概要と一致する必要があります。
3. 特定のメンバーについて、シリアル化されたメンバー JSON を使用してマークル リーフを計算します。
   (`norito::json::to_vec(&sortition_member)`) と各証明ハッシュを組み込みます。決勝戦
   ダイジェストは `merkle_root_hex` に等しくなければなりません。サンプルサマリのヘルパーは次のように示しています
   `eligible_index`、`leaf_hash_hex`、および `merkle_proof[]` を組み合わせる方法。

これらのアーティファクトは、検証可能なランダム性に関する MINFO-2b 要件を満たしています。
k-of-m 選択、およびオンチェーン API が接続されるまでの追加のみの監査ログ。

## 検証エラーのリファレンス

`AgendaProposalV1::validate` は `AgendaProposalValidationError` バリアントを発行します
ペイロードがリンティングに失敗するたびに。以下の表は、最も一般的なものをまとめたものです
エラーが発生するため、ポータルのレビュー担当者は CLI 出力を実用的なガイダンスに変換できます。|エラー |意味 |修復 |
|----------|----------|---------------|
| `UnsupportedVersion { expected, found }` |ペイロード `version` は、バリデーターがサポートするスキーマと異なります。 |最新のスキーマ バンドルを使用して JSON を再生成し、バージョンが `expected` と一致するようにします。 |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` が空であるか、`AC-YYYY-###` 形式ではありません。 |再送信する前に、文書化された形式に従って一意の識別子を入力してください。 |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` はゼロか欠落しています。 |送信タイムスタンプを Unix ミリ秒単位で記録します。 |
| `InvalidLanguageTag { value }` | `language` は有効な BCP-47 タグではありません。 | `en`、`ja-JP` などの標準タグ、または BCP-47 によって認識される別のロケールを使用します。 |
| `MissingSummaryField { field }` | `summary.title`、`.motivation`、または `.expected_impact` のいずれかが空です。 |指定された概要フィールドに空ではないテキストを入力します。 |
| `MissingSubmitterField { field }` | `submitter.name` または `submitter.contact` がありません。 |レビュー担当者が提案者に連絡できるように、不足している送信者のメタデータを提供します。 |
| `InvalidTag { value }` | `tags[]` エントリは許可リストにありません。 |タグを削除するか、文書化された値 (`csam`、`malware` など) のいずれかに名前を変更します。 |
| `MissingTargets` | `targets[]` 配列が空です。 |少なくとも 1 つのターゲット ハッシュ ファミリ エントリを指定します。 |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` |ターゲット エントリに `label` フィールドまたは `reason` フィールドがありません。 |再送信する前に、インデックス付きエントリの必須フィールドに入力してください。 |
| `InvalidHashFamily { index, value }` |サポートされていない `hash_family` ラベル。 |ハッシュ ファミリ名を ASCII 英数字と `-_` に制限します。 |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` |ダイジェストが有効な 16 進数でないか、16 バイト未満です。 |インデックス付きターゲットに小文字の 16 進ダイジェスト (32 桁以上の 16 進文字) を指定します。 |
| `DuplicateTarget { index, fingerprint }` |ターゲット ダイジェストは、以前のエントリまたはレジストリ フィンガープリントを複製します。 |重複を削除するか、裏付けとなる証拠を 1 つのターゲットにマージします。 |
| `MissingEvidence` |証拠の添付ファイルは提供されませんでした。 |複製資料にリンクする少なくとも 1 つの証拠記録を添付してください。 |
| `MissingEvidenceUri { index }` |証拠エントリに `uri` フィールドがありません。 |インデックス付き証拠エントリのフェッチ可能な URI またはケース識別子を提供します。 |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` |ダイジェスト (SoraFS CID または添付ファイル) を必要とする証拠エントリが欠落しているか、無効な `digest_blake3_hex` が含まれています。 |インデックス付きエントリには、64 文字の小文字の BLAKE3 ダイジェストを指定します。 |

## 例

- `docs/examples/ministry/agenda_proposal_example.json` — 正規、
  2 つの証拠添付ファイルを含む糸くずのない提案ペイロード。
- `docs/examples/ministry/agenda_duplicate_registry.json` — スターター レジストリ
  単一の BLAKE3 フィンガープリントと理論的根拠が含まれています。

ポータル ツールを統合するとき、または CI を作成するときに、これらのファイルをテンプレートとして再利用します。
自動送信をチェックします。