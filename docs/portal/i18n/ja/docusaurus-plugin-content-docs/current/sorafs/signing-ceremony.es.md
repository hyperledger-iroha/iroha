---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 調印式
タイトル: 儀式の儀式
説明: SoraFS (SF-1b) のチャンカーの備品を販売する、Sora のパルラメントです。
サイドバーラベル: セレモニア・デ・ファーム
---

> ロードマップ: **SF-1b — Parlamento de Sora の設備の準備**
> アンティグアの再広場「セレモニア・デ・ファーム・デル・コンセホ」オフライン。

SoraFS の儀式マニュアルは、フィクスチャとチャンカーの詳細を示しています。トーダス
ラス アプロバシオネス アホラ パサン ポル エル **Parlamento de Sora**, la DAO basada en sorteo que gobierna
Nexus。フィアンザン XOR パラ オブテナー シウダダニア、ロタン エントレ パネレス Y のロス ミエンブロス デル パルラメント
チェーン上でプルエバン、レチャザン、またはフィクスチャのリリースを確認する際に投票します。エスタ ギア エクスプリカ
開発者向けのプロセスとツール。

## 議会の履歴書

- **シウダダニア** — ロス オペラドーレス フィアンザン エル XOR 要求パラ インクリビルス コモ シウダダノス y
  Volverse はパラソルテオでご覧いただけます。
- **パネル** — 回転パネルに対する責任（インフラストラクチャー、
  モデラシオン、テゾレリア、…）。インフラストラクチャのパネル、およびアプリケーションの実行
  SoraFSの治具。
- **ソルテオ・イ・ロータシオン** — 特定の分野でのパネル安全性の確認
  la constitucion del Parlamento para que ningun grupo monopolice las aprobaciones。

## 試合の準備を整える

1. **環境デプロプエスタ**
   - `manifest_blake3.json` マス エル ディフ デル フィクスチャのツール バンドル候補の WG
     `sorafs.fixtureProposal` 経由のオンチェーンのすべてのレジストリ。
   - BLAKE3 のレジストラ エル ダイジェスト、バージョンの意味とカンビオに関する情報。
2. **改訂版**
   - コーラ デ タレアス デルの移動に関するインフラストラクチャーのパネル
     パーラメント。
   - CI の遺物をパネル検査し、パリダーやエミテンの検査を行う
     オンチェーン上の投票ポンデラド。
3. **最終処理**
   - 定足数、ランタイムがイベントを含むイベントを発行することを許可します。
     カノニコのダイジェスト、マニフェスト、侵害、マークル、ペイロード、フィクスチャ。
   - SoraFS パラケ ロス クライアント プエダン オブテナー エルのレジストリを参照するイベント
     議会の報告書をマニフェストします。
4. **配布**
   - CLI のヘルパー (`cargo xtask sorafs-fetch-fixture`) のマニフェストの管理
     Nexus RPC。 JSON/TS/Go デル リポジトリの定数は、常に同期されています
     再取り出し `export_vectors` はオンチェーンの有効なダイジェスト コントロール レジストリです。

## 開発者向けのフルホ デ トラバホ

- Regenera フィクスチャの短所:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- 封筒をダウンロードし、検証するための、パルラメントの取得をサポートします。
  企業とリフレスカルフィクスチャのロケール。 Apunta `--signatures` al 封筒公開サイト
  パルラメント; el ヘルパー resuelve el マニフェスト コンパナンテ、再計算 el ダイジェスト BLAKE3 e
  インポネ エル パーフィル カノニコ `sorafs.sf1@1.0.0`。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pasa `--manifest` は、URL 上でマニフェストを実行します。ロス・エンベロープ・シン・ファーム・セ・レチャザン
一斉射撃は `--allow-unsigned` パラスモークを実行するロケールを設定します。

- ゲートウェイ デ ステージングのマニフェストを有効にし、ペイロードの Torii を確認する
  ロケール:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- El CI ローカルでは名簿 `signer.json` は必要ありません。
  `ci/check_sorafs_fixtures.sh` 最高の妥協点とレポジトリの比較
  オンチェーンとフォールアクアンドダイバージェン。

## ゴベルナンサのノート

- 政府議会定足数、回転とエスカラミエントの憲法;必要ありません
  簡単な構成。
- 緊急時のマネジャンの損失は、議会の緩和のためのパネルのトラベスでロールバックされます。
  ダイジェストで参照を元に戻すインフラストラクチャーのパネル
  事前にマニフェストを提出し、事前にリリースする必要があります。
- SoraFS パラメタの永久的な歴史的責任の管理
  リプレイフォレンス。

## よくある質問

- **ドンデセーフ `signer.json`?**  
  セエリミノ。オンチェーンで企業を存続させることができます。 `manifest_signatures.json`
  開発者向けのリポジトリとソロのフィクスチャは、究極のコンシディルと一致します
  不正行為の出来事。

- **Seguimos requiriendo farmas Ed25519 ロケール?**  
  いいえ、オンチェーン上のアルマセナン コンモ アーティファクトに関する国会議事堂。ロスの試合日程
  ロケールは再現性のあるものであり、議会の正当な内容を理解するために存在します。- **コモ監視のラス・アプロバシオネス・ロス・エクイポス?**  
  `ParliamentFixtureApproved` のイベントをサブスクライブするか、Nexus RPC 経由でレジストリを参照してください
  回復者、ダイジェスト、実際のマニフェスト、点呼、パネル。