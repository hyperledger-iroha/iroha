---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/lane-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスレーンモデル
タイトル: レーンズ في Nexus
説明: レーン、レーン、世界状態、ソラ Nexus。
---

# レーン数 Nexus وتقسيم WSV

> ** الحالة:** NX-1 - レーンを確認してください。  
> **المالكون:** Nexus コア WG、ガバナンス WG  
> **ロードマップ:** NX-1 في `roadmap.md`

`docs/source/nexus_lanes.md` を使用して、Sora Nexus を使用して SDK を使用します。レーン数はモノレポです。世界国家 (レーン) 世界国家 (レーン) جموعات مدققين عامة او خاصة باعمال معزولة。

## ああ

- **レーン:** シャード منطقي من 元帳 Nexus مع مجموعة مدققين خاصة به وバックログ تنفيذ。 `LaneId` です。
- **データ スペース:** レーン レーン、データ スペース、データ スペース。
- **レーン マニフェスト:** レーン マニフェスト:** レーン マニフェスト:** レーン マニフェスト:** レーン マニフェスト:**ありがとうございます。
- **グローバル コミットメント:** レーン間の移動を証明します。 NPoS へのコミットメント。

## レーン

レーン、フック、フック。 هندسة التهيئة (`LaneConfig`) هذه السمات كي تتمكن العقد وSDK والادوات من فهم التخطيط دون منطق مخصص。

|レーン | और देखें عضوية المدققين | WSV | और देखेंニュース | ニュースऔर देखें
|----------|-----------|---------------|--------------|---------|--------|-------------|
| `default_public` |ああ |パーミッションレス (グローバルステーク) |ログインしてください。 SORA 議会 | `xor_global` | دفتر عام اساسي |
| `public_custom` |ああ |許可のないステークゲート |ログインしてください。ステーク | ステーク`xor_lane_weighted` | طبيقات عامة عالية السعة |
| `private_permissioned` | और देखें مجموعة مدققين ثابتة (معتمدة من الحوكمة) |コミットメントと証拠 |連邦評議会 | `xor_hosted_custody` | CBDC 認証 | CBDC
| `hybrid_confidential` | और देखें認証済みZK プルーフ |コミットメント + 認証 | حدة نقود قابلة للبرمجة | `xor_dual_fund` |ログイン して翻訳を追加する

レーンを通過する:

- 別名 للداتاسبيس - تجميع مقروء للبشر يربط سياسات الامتثال。
- ガバナンス ハンドル - معرف يحل عبر `Nexus.governance.modules`。
- 決済ハンドル - 決済ルーター XOR。
- メタデータ `/status` ダッシュボード。

## هندسة تهيئة レーン (`LaneConfig`)

`LaneConfig` ランタイム カタログ レーン。マニフェストは次のようになります。レーンを超えてください。

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` يعيد حساب الهندسة عند تحميل التهيئة (`State::set_nexus`)。
- エイリアス ナメクジ エイリアス ナメクジ エイリアス`_` を確認してください。ナメクジ فارغ نعود الى `lane{id}`。
- `shard_id` メタデータ `da_shard_id` (`lane_id`) ジャーナル シャード シャードリプレイ 再起動/リシャーディング。
- プレフィックス المفاتيح بقاء نطاقات مفاتيح WSV لكل レーン منفصلة حتى عند مشاركة نفس バックエンド。
- クラ ホストの名前管理者は、マニフェストを作成する必要があります。
- マージヒント (`lane_{id:03}_merge`) ルートとマージヒント、コミットメント、レーン。

## 世界国家- 世界国家 Nexus レーン。レーン العامة تحفظ حالة كاملة؛レーン マークル/機密情報 ルート マークル/コミットメント マージ元帳。
- MV يسبق كل مفتاح ببادئة 4 بايت من `LaneConfigEntry::key_prefix`, منتجا مفاتيح مثل `[00 00 00 01] ++ PackedKey`.
- 分析（アカウント、資産、トリガー、分析）、範囲スキャン、分析、分析。やあ。
- メタデータのマージ元帳の表示: レーンのルート マージヒント ルートの説明 `lane_{id:03}_merge` マークレーンを確認してください。
- クロスレーン（アカウントエイリアス、資産レジストリ、ガバナンスマニフェスト）やあ。
- **سياسة الاحتفاظ** - レーン العامة تحتفظ باجسام الكتل كاملة؛レーンは、コミットメントを確認し、チェックポイントは、コミットメントを確認します。レーンの機密情報は、ワークロードの重要性を考慮したものです。
- **ツール** - 管理者 (`kagami`、管理者 CLI) 名前空間 スラッグ メトリクスتسميات Prometheus او ارشفة مقاطع クラ。

## ルーティング وAPI

- テスト Torii REST/gRPC `lane_id` テストغيابها يعني `lane_default`。
- SDK のレーン、エイリアス、`LaneId` のカタログ レーン。
- ルーティングとカタログ、レーン、データスペース。 يوفر `LaneConfig` エイリアス مناسبة للتيليمتري في ダッシュボード والlogs。

## 決済

- レーンを XOR に変換します。レーンとガス、XOR のコミットメントを確認します。
- 証拠、メタデータ、エスクロー (保管庫、保管庫)。
- 決済ルーター (NX-3) バッファーとレーンの接続。ああ。

## ガバナンス

- レーンのカタログ。 `LaneConfigEntry` 別名 وslug الاصلين لابقاء تيليمتري ومسارات التدقيق مقروءة。
- Nexus レジストリ マニフェストは、`LaneId` データスペース、ガバナンス ハンドル、決済ハンドル、メタデータを示します。
- フック ランタイム フック (`gov_upgrade_id` ) テレメトリ ブリッジ 差分 (`gov_upgrade_id`) `nexus.config.diff`)。

## テレメトリのステータス

- `/status` のエイリアス、レーン、データスペース、ハンドル、カタログ、`LaneConfig`。
- スケジューラー (`nexus_scheduler_lane_teu_*`) のエイリアス/スラッグ、バックログ、TEU のエイリアス/スラッグ。
- `nexus_lane_configured_total` يحصي عدد ادخالات lan المشتقة ويعاد حسابه عند تغير التهيئة。レーンの差分を確認してください。
- ゲージのバックログ メタデータのエイリアス/説明。

## التهيئة وانواع Norito

- `LaneCatalog`、`LaneConfig`、و`DataSpaceCatalog` تعيش في `iroha_data_model::nexus` وتوفر هياكل متوافقة مع Norito للـマニフェスト وSDK。
- `LaneConfig` يعيش في `iroha_config::parameters::actual::Nexus` ويشتق تلقائيا من カタログNorito エンコード ヘルパー ランタイム。
- 情報 (`iroha_config::parameters::user::Nexus`) レーンとデータスペースの情報レーン ID は、エイリアスとレーンの ID です。

## ああ、

- 決済ルーター (NX-3) は、XOR スラグ レーンをバッファーします。
- ツール、列ファミリー、レーン、レーン、ネームスペース、スラッグ。
- クロスレーンのフィクスチャー (順序付け、枝刈り、競合検出)。
- ホワイトリスト/ブラックリストのフック (NX-12)。

---

* NX-1 NX-2 NX-18 を参照してください。 `roadmap.md` 追跡者追跡者追跡者追跡者追跡者追跡者追跡者追跡者*