---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 調印式
タイトル: ستبدال مراسم التوقيع
説明: フィクスチャはチャンカー SoraFS (SF-1b) です。
サイドバーラベル: 重要な要素
---

> スコア: **SF-1b — 試合結果は次のとおりです。**
> مسار البرلمان يحل محل "مراسم توقيع المجلس" القديمة خارج الشبكة.

チャンカー SoraFS を確認してください。やあ
テストは、Nexus を実行します。
يقوم اعضاء البرلمان برهن XOR للحصول على المواطنة، ويتناوبون عبر اللجان، ويصوتون
オンチェーンのフィクスチャ。 شرح هذا الدليل
ありがとうございます。

## ナオミ

- **المواطنة** — يقوم المشغلون برهن XOR المطلوب للتسجيل كمواطنين واكتساب اهلية القرعة。
- **اللجان** — تتوزع المسؤوليات عبر لجان دوارة (البنية التحتية، الاشراف، الخزانة، ...)。
  備品は SoraFS です。
- ** القرعة والتناوب** — يعاد سحب مقاعد اللجان وفق الوتيرة المحددة في دستور البرلمان
  重要な問題を解決してください。

## フィクスチャ

1. **今日のこと**
   - ツーリングWG `manifest_blake3.json` 治具
     オンチェーン `sorafs.fixtureProposal`。
   - ブレイク 3 ダイジェスト ニュース。
2. **アフィリエイト**
   - 最高のパフォーマンスを見せてください。
   - アーティファクトの作成、CI の作成、および作成
     オンチェーン。
3. **アフィリャンهاء**
   - ランタイム ステータス ダイジェスト canonico マニフェスト
     マークルの備品。
   - ينعكس الحدث في سجل SoraFS حتى يتمكن العملاء من جلب احدث マニフェスト معتمد من البرلمان.
4.***
   - CLI (`cargo xtask sorafs-fetch-fixture`) マニフェスト Nexus RPC。
     JSON/TS/Go を使用して、`export_vectors` を使用します。
     オンチェーンでのダイジェストの視聴。

## और देखें

- 試合日程:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- 封筒、封筒、備品の確認
  ああ。 `--signatures` 封筒の番号マニフェストを作成する
  ダイジェストBLAKE3 وفرض الملف الشخصي القانوني `sorafs.sf1@1.0.0`。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

مرر `--manifest` اذا كان マニフェスト في عنوان مختلف。封筒の封筒
`--allow-unsigned` は煙を出します。

- マニフェストとステージング Torii ペイロード:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- CI カード、名簿、`signer.json`。
  `ci/check_sorafs_fixtures.sh` يقارن حالة المستودع باحدث التزام オンチェーン ويفشل
  認証済みです。

## すごい

- ニュース - ニュース - ニュース - ニュース木箱が必要です。
- 最高のパフォーマンスを実現します。テストを実行してください。
  ダイジェストを元に戻し、マニフェストを元に戻します。
- SoraFS を使用してください。

## いいえ

- **アライアンス `signer.json`**  
  そうです。 سناد التوقيعات بالكامل على السلسلة؛ `manifest_signatures.json` 年
  フィクスチャは、最高のパフォーマンスを実現します。

- **هل ما زلنا نحتاج تواقيع Ed25519 محلية؟**  
  ああ。素晴らしいアーティファクトが見つかります。試合日程
  ダイジェスト版をご覧ください。

- **كيف تراقب الفرق الموافقات؟**  
  応答 `ParliamentFixtureApproved` 応答 Nexus RPC 応答
  ダイジェストとマニフェスト。