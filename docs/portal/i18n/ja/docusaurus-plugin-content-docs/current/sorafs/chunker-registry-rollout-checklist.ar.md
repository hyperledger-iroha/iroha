---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: قائمة تحقق لإطلاق سجل chunker لسوراFS
サイドバーラベル: チャンカー
説明: チャンカー。
---

:::note ノート
`docs/source/sorafs/chunker_registry_rollout_checklist.md`。スフィンクスの正体は、スフィンクスです。
:::

# قائمة تحقق لإطلاق سجل SoraFS

جمع هذه القائمة الخطوات المطلوبة لترقية ملف chunker جديد أو حزمة قبول مزوّد
重要な意味を持っています。

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` フィクスチャ
> المعتمدة (`fixtures/sorafs_chunker/*`)。

## 1. 重要な情報

1. 試合の日程:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 重要な意味を持つ言葉
   `docs/source/sorafs/reports/sf1_determinism.md` (日本)
   最高のパフォーマンスを見せてください。
3. أكد من أن `sorafs_manifest::chunker_registry` يبنى مع
   `ensure_charter_compliance()` 番号:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. メッセージ:
   - `docs/source/sorafs/proposals/<profile>.json`
   - ニュース `docs/source/sorafs/council_minutes_*.md`
   - ニュース

## 2. いいえ

1. ツール ワーキング グループ ダイジェスト ソラ議会インフラ パネル。
2. いいえ、いいえ。
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. 試合の日程:
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. 重要な情報:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージング

ارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل。

1. 発見 Torii 発見 `torii.sorafs` 施行 施行
   (`enforce_admission = true`)。
2. ステージング ステージング ステージング ステージング ステージング ステージング
   `torii.sorafs.discovery.admission.envelopes_dir`。
3. 発見の結果:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. マニフェスト/計画のマニフェスト/計画:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات التليمترية (`torii_sorafs_*`) وقواعد التنبيه تعرض الملف الجديد
   認証済みです。

## 4. いいえ

1. ステージングは Torii です。
2. SDK を使用します。
3. PR 情報:
   - 試合日程
   - تغييرات الوثائق (مراجع الميثاق، تقرير الحتمية)
   - ロードマップ/ステータス
4. 来歴。

## 5. いいえ、いいえ。

1. التقط المقاييس النهائية (発見、発見、発見、取得)
   الأخطاء) بعد 24 ساعة من الإطلاق.
2. حدّث `status.md` بملخص قصير ورابط لتقرير الحتمية.
3. سجّل أي مهام متابعة (مثل إرشادات إضافية لكتابة الملفات) في `roadmap.md`。