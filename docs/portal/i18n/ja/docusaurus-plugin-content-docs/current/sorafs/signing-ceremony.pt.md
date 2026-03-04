---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 調印式
タイトル: アサナチュラの代替品
説明: Como o Parlamento Sora aprova e distribui fixtures do chunker SoraFS (SF-1b)。
サイドバーラベル: Cerimonia de assinatura
---

> ロードマップ: **SF-1b - Parlamento Sora の備品の開発。**
> オフラインで「セレモニア デ アシナチュラ ド コンセルホ」というアンチガを置き換える議会をご覧ください。

おお、OS フィクスチャを使用してチャンカー SoraFS を実行するための儀式マニュアル。
Todas as aprovacoes agora passam pelo **Parlamento Sora**、DAO のベースアダ エム ソーテイオ クエリ
ガバナ o Nexus。会員は、シダダニア、ロタシオナムを観察する XOR ブロックを開催します
entre paineis と votam オンチェーンパラ承認、rejeitar または reverter releases defixture です。
開発者向けにプロセスやツールを説明します。

## ビザオ ジェラル ド パルラメント

- **Cidadania** - Operadores bloqueiam o XOR necessario para se inscrever como cidadaos e
  セ・トルナル・エレギヴィス・アオ・ソルテイオ。
- **Paineis** - As responsabilidades sao divididas entre paineis rotativos (Infraestrutura,
  モデラソー、テソウラリア、...)。インフラストラクチャのパイネル、およびアプリケーションの実行
  器具は SoraFS を実行します。
- **Sorteio e rotacao** - パイネルのカディラとして、定義されたカデンシアの定義として
  constituicao do Parlamento para que nenhum grupo は aprovacoes として独占されます。

## フィクスチャのフルクソ

1. **提案の提出**
   - O ツーリング WG は、バンドル候補 `manifest_blake3.json` の差分フィクスチャを参照します
     `sorafs.fixtureProposal` 経由でオンチェーンのレジストリにアクセスします。
   - BLAKE3 をダイジェストするための登録簿、ムダンカの解釈と同様の意味。
2. **Revisao e votacao**
   - 議会の情報を参照してください。
   - メンバーは、CI の検査、検査の検査を行っています。
     オンチェーン上の登録投票ポンダーラド。
3. **ファイナライザカオ**
   - クォーラムの実行、または承認を含むイベントのランタイム発行
     カノニコのダイジェスト、マニフェスト、妥協のマークル、ペイロード、フィクスチャ。
   - イベント エスペルハド レジストリ SoraFS パラケ クライアント ポッサム バスカー o
     マニフェストは最近の承認を得るために議会に提出します。
4. **ディストリビューカオ**
   - CLI ヘルパー (`cargo xtask sorafs-fetch-fixture`) によるマニフェストの実行
     Nexus RPC。定数として JSON/TS/Go はリポジトリ ficam sincronizadas ao を実行します
     reexecutar `export_vectors` オンチェーンのレジストリ コントラ ダイジェストの検証。

## Fluxo デ トラバルホ デ開発者

- Regenere フィクスチャー コム:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- バイシャルのパーラメント取得ヘルパー、エンベロープ アプロバード、検証を使用します。
  あらゆる状況に対応したフィクスチャを提供します。 Aponte `--signatures` パラオエンベロープ
  公共のペロ・パルラメント。 o ヘルパー解決 o マニフェスト関連付け、再計算 o
  BLAKE3 をダイジェストし、canonico `sorafs.sf1@1.0.0` を実行します。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

`--manifest` を外部 URL のマニフェストに渡します。封筒はシナチュアに似ています
サン・レクサドス、メノス・クエ`--allow-unsigned`セハ・デフィニド・パラ・スモーク・ラン・ロカイス。

- ゲートウェイデステージング経由のマニフェストの検証、Torii em vez de の対応
  ペイロードの場所:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI ローカル nao exige mais um 名簿 `signer.json`。
  `ci/check_sorafs_fixtures.sh` 究極の妥協点を比較し、リポジトリを作成します
  オンチェーンとファルハの分岐点。

## 統治に関するノート

- 議会定足数、回転性エスカロナメントの構成 - 直江
  必要な設定はクレートに必要ありません。
- 議会での緊急事態のロールバック。 ○
  インフラストラクチャーは、ダイジェストの参照を元に戻すためのインフラストラクチャーを作成します
  前に明示し、代わりにリリースを承認します。
- Aprovacoes Historicalas permanecem disponiveis no registry SoraFS パラ リプレイ
  法廷。

## よくある質問

- **`signer.json` をご覧ください?**  
  フォイ・リモビデオ。オンチェーンでの活動を可能にします。 `manifest_signatures.json`
  リポジトリと開発者向けのアペナス フィクスチャがありません。開発者は、究極的に対応します。
  イベント・デ・アプロバカオ。

- **Ainda exigimos assinaturas Ed25519 locais?**  
  ナオ。アプロバコとして、オンチェーンでパルラメント サオ アルマゼナダス コモ アルテファトを行います。備品
  再現性のある場所が存在し、議会でのダイジェストの検証が行われます。

- **コモは監視アプロバコを装備していますか?**  
  `ParliamentFixtureApproved` のイベントを割り当て、Nexus RPC 経由でレジストリを参照します
  回復中または消化中の実際の症状は、痛みを伴うチャマダです。