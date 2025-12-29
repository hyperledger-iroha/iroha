<!-- Japanese translation of docs/account_structure_sdk_alignment.md -->

---
lang: ja
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
translator: manual
---

# SDK とコーデック担当者向け IH-B32 ロールアウトノート

対象チーム: Rust SDK、TypeScript/JavaScript SDK、Python SDK、Kotlin SDK、コーデックツール

背景: `docs/account_structure.md` が更新され、IH-B32 アドレスの最終設計、相互運用のマッピング、実装チェックリストが確定しました。各ライブラリの挙動とテストを正規仕様に合わせてください。

主要な参照（行アンカー）:
- IH-B32 のエンコード規則と小文字限定ルール — `docs/account_structure.md:120`
- IH58 エイリアス手順と規範ベクトル — `docs/account_structure.md:151`
- 実装チェックリスト — `docs/account_structure.md:276`

アクションアイテム:
1. 各 SDK で IH-B32 を既定のテキスト形式として提示し、すべての入力が正規コーデックを通過するよう `alias@domain` のパースを廃止する。
2. 文書化された通りに CAIP-10、IH58 の変換を実装する。
3. 規範ベクトル（Ed25519 と secp256k1）およびチェックサム／チェーン不一致時の失敗ケースをテストに追加する。
4. レジストリ参照契約をミラーする: Nexus マニフェストが `{discriminant, ih58_prefix, chain_alias}` タプルを公開し、TTL セマンティクスを強制することを前提にする。
5. リリースノートを調整し、ダウンストリームの統合先に IH-B32 サポートが各言語で同時期に出荷されることを周知する。

コーデック／テストの更新が完了したら受領連絡をお願いします。未解決の質問はアカウントアドレッシング RFC のスレッドで追跡してください。
