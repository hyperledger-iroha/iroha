---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: シリアス SF-6
概要: 重要な情報 セキュリティ情報 セキュリティ情報 セキュリティ情報 セキュリティ情報 セキュリティ情報が明らかになります。
---

# マスコットSF-6

** 年月日:** 2026-02-10 → 2026-02-18  
**セキュリティ エンジニアリング ギルド (`@sec-eng`)、ツール ワーキング グループ (`@tooling-wg`)  
** 説明:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、証明ストリーミング、マニフェストTorii は Sigstore/OIDC をリリースしました。  
** 名前:**  
- CLI による (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- マニフェスト/証明 في Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- リリース (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- ハーネス تساوي حتمي (`crates/sorafs_car/tests/sorafs_cli.rs`, [تقرير تكافؤ GA لمشغل SoraFS](./orchestrator-ga-parity.md))

## ああ

1. **脅威モデリング** 脅威モデル Torii。  
2. **مراجعة الشيفرة** ركزت على أسطح الاعتمادات (تبادل رموز OIDC، キーレス署名) は、マニフェストをマニフェストします。 Norito バックプレッシャー耐性のあるストリーミング。  
3. ** フィクスチャ マニフェストの確認 (トークン リプレイ、マニフェストの改ざん、証明ストリームの確認) のパリティハーネスとファズドライブが重要です。  
4. **فحص الإعدادات** デフォルトの `iroha_config` を使用して、CLI をリリースしてください。 قابلة للتدقيق。  
5. **重要な** 修復、証拠、所有者、ツーリング WG。

## 重要な意味

| ID |ああ |ああ |認証済み |ああ |
|----|----------|------|----------|------------|
| SF6-SR-01 |ヤス |キーレス署名 |視聴者は、OIDC の視聴者と視聴者、CI の視聴者、リプレイ、視聴者を確認します。 | `--identity-token-audience` フック CI ([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`)。 CI يفشل عند غياب 視聴者。 |
| SF6-SR-02 |重要 |プルーフストリーミング |バックプレッシャーとバッファーの両方が必要です。 | يفرض `sorafs_cli proof stream` أحجام قنوات محدودة مع truncation حتمي، ويسجل Norito 概要 ويجهض التدفق؛ Torii は応答チャンク (`crates/iroha_torii/src/sorafs/api.rs`) をミラーリングします。 |
| SF6-SR-03 |重要 |マニフェスト | CLI は、チャンク プランのマニフェスト、`--plan` のチャンク プランをマニフェストします。 | CAR ダイジェスト `--expect-plan-digest` 不一致を修復します。翻訳 (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 | और देखें監査証跡 | كانت قائمة التحقق للإطلاق تفتقر إلى سجل موافقة موقع لمراجعة الأمان. | [リリース プロセス](../developer-releases.md) のハッシュと、サインオフの GA。 |

高/中レベルのパリティ ハーネスを確認してください。最高です。

## और देखें

- ** 評価:** 評価 CI 視聴者 発行者 評価CLI リリース ヘルパーのリリース `--identity-token-audience` および `--identity-token-provider`。  
- ** マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト不一致のダイジェストが見つかりませんでした。  
- **バック プレッシャー プルーフ ストリーミング:** Torii セキュリティ PoR/PoTR セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ レイテンシーمقطوعة + خمسة أمثلة فشل، مانعا نمو المشتركين غير المحدود مع الحفاظ على 要約 حتمية。  
- ** 評価:** プルーフ ストリーミング (`torii_sorafs_proof_stream_*`) と CLI の概要、パンくずリスト。  
- **التوثيق:** تشير أدلة المطورين ([開発者インデックス](../developer-index.md)[CLI リファレンス](../developer-cli.md)) 説明すごいですね。

## إضافات إلى قائمة التحقق للإطلاق

**يجب** على مديري الإطلاق إرفاق الأدلة التالية عند ترقية مرشح GA:

1. ハッシュ لآخر مذكرة مراجعة أمان (هذا المستند)。  
2. 修復 (مثل `governance/tickets/SF6-SR-2026.md`)。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` は、視聴者/発行者を表します。  
4. パリティ ハーネス (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. テストは、Torii をカウンターします。

アーティファクトの承認、GA の承認。

**ハッシュ مرجعية للقطع الفنية (サインオフ بتاريخ 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## متابعات معلقة- ** حديث نموذج التهديد:** إعادة هذه المراجعة كل ربع أو قبل إضافات كبيرة لأعلام CLI。  
- **ファジング:** ファジングの証明ストリーミング `fuzz/proof_stream_transport` ペイロード: ID、gzip、deflate、zstd。  
- ** ステータス:** ステータス ステータス トークン ロールバック マニフェスト ステータス ステータスありがとうございます。

## ああ

- セキュリティ エンジニアリング ギルド: @sec-eng (2026-02-20)  
- ツールワーキンググループ: @tooling-wg (2026-02-20)

バンドル バンドルをダウンロードしてください。