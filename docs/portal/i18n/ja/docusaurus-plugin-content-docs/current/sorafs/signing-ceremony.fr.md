---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 調印式
タイトル: 署名の交換式
説明: コメント le Parlement Sora の承認とチャンカー SoraFS (SF-1b) の備品の配布。
Sidebar_label: 署名セレモニー
---

> ロードマップ : **SF-1b — Parlement Sora の備品の承認**
> Le workflow du Parlement remplace l'ancienne « ceremonie de signed du conseil » hors ligne。

チャンカー SoraFS の署名デフィクスチャーのマニュアルが廃止されました。トゥート レ
**Parlement Sora** に対する承認は承認され、DAO の基準は tirage に達します
au sort qui gouverne Nexus。 XOR からの議会のメンバーのブロック
citoyennete、トーナメントのパネルおよび投票オンチェーンの承認者、拒否者
フィクスチャのリリースを確認します。 CE ガイドのプロセスとツールの説明
開発者に注いでください。

## 国会議事堂のアンサンブル

- **Citoyennete** — 操作不能な XOR 要求により、通知が送信されます
  citoyens et devenir は、すべての条件を満たしています。
- **パネル** — パネルを回転させる責任を負います
  (インフラストラクチャ、モデレーション、トレゾレリなど)。ル・パネル・インフラストラクチャーのディティエント
  規定の承認 SoraFS。
- **並べ替えと回転を監視** — パネルの包囲戦を攻撃する
  議会の憲法の傾向は、グループ全体で指定されています
  レ・アプロベーションを独占する。

## 備品に対する承認の流動性

1. **提案に関する提案**
   - ツール WG テレバース ファイル バンドル候補 `manifest_blake3.json` および差分
     `sorafs.fixtureProposal` 経由でオンチェーンに登録するためのフィクスチャ。
   - BLAKE3 のダイジェストとバージョンの意味とメモに関する提案
     変化。
2. **レビューと投票**
   - パネルインフラストラクチャは、議会のファイルを通じて影響を再確認します。
   - 成果物 CI のメンバー検査、パリテなどのテストの実行
     オンチェーンで投票を検討します。
3. **完成**
   - 定足数を確保し、承認参加者のランタイムを均等化する必要があります
     マニフェストとエンゲージメントのペイロードのマークルのダイジェスト。
   - レジストリ SoraFS はクライアントの要求に応じて重複したものです
     recuperer le dernierマニフェストは議会を承認します。
4. **配布**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) 回復ファイル マニフェスト
     Nexus RPC 経由で承認します。 Les constantes JSON/TS/Go du depot の保存同期
     関連する `export_vectors` および登録に関する有効なダイジェスト
     オンチェーン。

## ワークフロー開発者

- 平均的なフィクスチャーの再生:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- 電話充電器を使用して議会を取得し、承認を取得します。
  検証者は署名とラフライチルはフィクスチャのロケールを確認します。ポインタ `--signatures`
  議会の出版物を封筒に入れます。ルヘルパーリソースルマニフェストアソシエ、
  BLAKE3 ダイジェストを再計算し、プロファイル canonique `sorafs.sf1@1.0.0` を適用します。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passer `--manifest` ファイルマニフェストは、自動 URL を設定します。レ・エンベロープ・ノン
署名者は、sauf si `--allow-unsigned` estアクティブな煙を実行するロコーを拒否します。

- ゲートウェイデステージング経由で検証ツールをマニフェストに注ぎ、cibler Torii plutot que des
  ペイロード ロコー :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI ローカル n'exige プラス un roster `signer.json`。
  `ci/check_sorafs_fixtures.sh` レポートの詳細なエンゲージメントを比較します
  オンチェーンとエコーのロスキルが発散します。

## 統治ノート

- 議会の憲法、定足数、ローテーションおよびエスカレード。
  Aucune の設定は必要不可欠なものです。
- 議会の審議委員会を通じて、緊急事態のロールバックを行います。ル
  パネル インフラストラクチャは、参照ファイルのダイジェストを元に戻す提案を削除します
  マニフェストの先例、およびリリースの変更は承認されます。
- レジストリ SoraFS の承認履歴を保持する
  リプレイフォレンシック。

## よくある質問

- **もう終わりです `signer.json` ?**  
  私は最高です。オンチェーンでの署名の帰属を強調します。 `manifest_signatures.json`
  デポで最も重要なフィクスチャを開発し、対応する必要があります
  承認の夕方。

- **署名のアンコールは Ed25519 ロケールですか ?**  
  ノン。議会の承認は、チェーン上で人工物を入手することを目的としています。レ・フィクスチャ
  ロケールは議会のダイジェストと比較して再生産性を維持するために存在します。- **コメントを監視する必要がありますか?**  
  Abonnez-vous a l'evenement `ParliamentFixtureApproved` を介してレジストリを問い合わせます
  Nexus RPC は、マニフェストの実際のダイジェストとパネルのメンバーのリストを取得します。