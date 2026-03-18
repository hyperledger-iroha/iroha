---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: SoraFS
説明: 重要な情報を入力してください。
---

> مقتبس من [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md)。

# SoraFS

RFC معمارية SoraFS。レビュー を日本語に翻訳する
ログインしてください。 और देखें
RFC を使用してください。
(`docs/source/sorafs_architecture_rfc.md`) ログインしてください。

|ああ |ログイン | ログイン重要 | और देखेंログイン | ログインああ |
|----------|--------------|--------------|--------------|---------------|----------|
| M0 | 1–6 |チャンカーの試合結果パイプライン、CAR + マニフェスト、アーティファクト、最高です。 |ドキュメント、DevRel、SDK | اعتماد `sorafs_manifest_stub` مع flags التوقع، تسجيل الإدخالات في هذا السجل، الحفاظ على الـ CDN القديم。 | ✅ こんにちわ |
| M1 | 7–12 | CI フィクスチャー別名 متاحة في staging؛ツールのフラグ。 |ドキュメント、ストレージ、ガバナンス |フィクスチャー、エイリアス、ステージング、`--car-digest/--root-cid` を使用します。 | ⏳ علّق |
| M2 | 13–20 |レジストリを固定する レジストリを固定するアーティファクトを作成するレジストリを確認してください。 |ストレージ、運用、ガバナンス |レジストリをピン留めし、ホストをホストします。 | ⏳ علّق |
| M3 | 21 歳以上 |別名 فقط؛レジストリを登録するCDN を参照してください。 |運用、ネットワーキング、SDK | DNS セキュリティ URL セキュリティ セキュリティ セキュリティ デフォルト セキュリティSDK。 | ⏳ علّق |
| R0～R3 | 2025-03-31 → 2025-07-01 |プロバイダーの広告: R0 は R1 を示し、R2 はハンドル/機能を示します。 R3 はペイロードを示します。 |可観測性、運用、SDK、DevRel |広告 `grafana_sorafs_admission.json` 広告 `grafana_sorafs_admission.json` 広告 広告 R2 広告30 人以上。 | ⏳ علّق |

محاضر مستوى التحكم في الحوكمة التي تشير إلى هذه المعالم موجودة تحت
`docs/source/sorafs/`。 يجب على الفرق إضافة نقاط مؤرخة أسفل كل صف عندما تقع أحداث
ملحوظة (مثل تسجيلات エイリアス جديدة أو مراجعات حوادث レジストリ) لتوفير أثر تدقيقي。

## いいえ

- 2025-11-01 — تم توزيع `migration_roadmap.md` على مجلس الحوكمة وقوائم المشغلين
  意味بانتظار المصادقة في جلسة المجلس القادمة (المرجع: متابعة
  `docs/source/sorafs/council_minutes_2025-10-29.md`)。
- 2025-11-02 — يفرض ISI تسجيل Pin Registry الآن التحقق المشترك للـ chunker/السياسة عبر
  ヘルパー `sorafs_manifest`، مما يبقي المسارات オンチェーン متسقة مع فحوصات Torii。
- 2026-02-13 — ロールアウト (R0–R3) のロールアウト
  ログインしてください。
  (`provider_advert_rollout.md`、`grafana_sorafs_admission.json`)。