<!-- Japanese translation of docs/source/norito_json_inventory.md -->

---
lang: ja
direction: ltr
source: docs/source/norito_json_inventory.md
status: complete
translator: manual
---

# Norito JSON インベントリ（Serde 使用状況）

_最終更新: 2025-11-09（`python3 scripts/inventory_serde_usage.py --json` 実行結果）_

直近のインベントリでは **本番クレートに Serde の利用はゼロ** であることが確認できました。ランタイムモジュールはすべて Norito の derive・ビジター・ヘルパーのみを使用しています。生 JSON レポートに現れる一致は次のものに限られます。

- Serde 排除のガードレールを記述したポリシードキュメント（`AGENTS.md`, `CONTRIBUTING.md`, `status.md` など）。
- ガードレールを実装／文書化するスクリプト（`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`, `scripts/list_serde_json_candidates.sh`）。
- `#[derive(JsonSerialize, JsonDeserialize)]` など Norito 固有の derive（文字列に `Serialize` が含まれるためヒットしますが実装は Norito）。
- テスト専用の許可リスト（歴史的 Serde 出力と Norito スナップショットを比較するフィクスチャ）。

## サマリ

| カテゴリ | Serde ヒット件数 | 解釈 |
| --- | --- | --- |
| 本番クレート（`core`, `torii`, `ivm`, `config`, `norito`, `fastpq_prover`, `cli`） | 0 | クリーンを確認。Norito の derive のみがツリーに存在。 |
| ツール／ポリシードキュメント（`scripts`, `docs`, `AGENTS.md`, `misc`） | ガードレールの記述のみ | インベントリ行はドキュメントまたは deny スクリプトを指す。 |
| サンプル／テスト（`data_model_samples`, `integration`, `pytests`） | 意図的なフィクスチャ | 回帰検証のため歴史的 JSON スナップショットを保持。 |

付随する機械可読レポートは `docs/source/norito_json_inventory.json` にあり、許可済み一致と理由を記録しています。今後の実行でも本番ヒットがゼロであり続けるべきであり、許可リスト外のエントリが出現した場合は Serde 除去取り組みの回 regress とみなしてください。
