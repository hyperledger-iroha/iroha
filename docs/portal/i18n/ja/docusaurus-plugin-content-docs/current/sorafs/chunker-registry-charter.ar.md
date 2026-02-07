---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: ميثاق سجل chunker لـ SoraFS
サイドバーラベル: チャンカー
説明: ميثاق الحوكمة لتقديم ملفات chunker واعتمادها。
---

:::note ノート
テストは `docs/source/sorafs/chunker_registry_charter.md` です。スフィンクスの正体は、スフィンクスです。
:::

# ميثاق حوكمة سجل chunker في SoraFS

> **مُصادَق عليه:** 2025-10-29 من قبل ソラ議会インフラパネル (انظر)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。評価 評価 評価 評価 評価 評価 評価 評価
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق بديل。

チャンカー SoraFS。
وهو يكمل [دليل تأليف ملفات chunker](./chunker-profile-authoring.md) عبر وصف كيفية اقتراح الملفات الجديدة ومراجعتها
وتصديقها ثم إيقافها لاحقاً。

## ああ

ينطبق الميثاق على كل إدخال في `sorafs_manifest::chunker_registry` وعلى
ツール (マニフェスト CLI、プロバイダー広告 CLI、SDK)。 ويُطبّق ثوابت エイリアス ハンドル名
評価 `chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب.
- جب أن يظهر المقبض المعتمد `namespace.name@semver` كأول إدخال في `profile_aliases`。
  そうです。
- 最高のパフォーマンスを見せてください。

## ああ

- **著者** – يُعدّون المقترح، ويعيدون توليد fixtures، ويجمعون أدلة الحتمية。
- **ツーリング ワーキング グループ (TWG)** – テストを行っています。
- **ガバナンス評議会 (GC)** – يراجع تقرير TWG، ويوقع ظرف المقترح، ويوافق على جداول النشر/الإيقاف。
- **ストレージ チーム** – セキュリティ チーム。

## سير العمل عبر دورة الحياة

1. **今日のこと**
   - ينفذ المؤلف قائمة التحقق من دليل التأليف ويُنشئ JSON من نوع `ChunkerProfileProposalV1` ضمن
     `docs/source/sorafs/proposals/`。
   - CLI の手順:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - 広報番組の試合情報。

2. ** ツール (TWG)**
   - テスト (フィクスチャ、ファズ、マニフェスト/PoR)。
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من نجاح
     `ensure_charter_compliance()` そうです。
   - CLI (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) يعكس البدائل والمقابض المحدثة。
   - قدّم تقريراً قصيراً يلخص النتائج وحالة النجاح/الفشل.

3. **マサチューセッツ州 (GC)**
   - TWG の記録。
   - ダイジェスト المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     備品の追加。
   - 最高のパフォーマンスを実現します。

4. **アフィリエイト**
   - PR メッセージ:
     - `sorafs_manifest::chunker_registry_data`。
     - التوثيق (`chunker_registry.md` وأدلة التأليف/المطابقة)。
     - 備品。
   - SDK のバージョン。

5.***************
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً نافذة نشر مزدوجة (فترات سماح) وخطةそうです。

6. **評価**
   - 最高のパフォーマンスを実現します。
   - يجب على TWG توثيق خطوات الحد من المخاطر وتحديث سجل الحوادث。

## ツール

- `sorafs_manifest_chunk_store` と `sorafs_manifest_stub` يوفّران:
  - `--list-profiles` です。
  - `--promote-profile=<handle>` كتولية البيانات المعتمدة المستخدمة عند ترقية ملف.
  - `--json-out=-` は、stdout の標準出力をテストします。
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`、`provider_advert_stub`)。 يجب أن تفشل اختبارات CI إذا كانت
  ありがとうございます。

## いいえ

- خزّن كل تقارير الحتمية في `docs/source/sorafs/reports/`.
- محاضر المجلس التي تشير إلى قرارات chunker محفوظة ضمن
  `docs/source/sorafs/migration_ledger.md`。
- حدّث `roadmap.md` و `status.md` بعد كل تغيير كبير في السجل。

## いいえ

- دليل التأليف: [دليل ملفات チャンカー](./chunker-profile-authoring.md)
- 番号: `docs/source/sorafs/chunker_conformance.md`
- 意味: [チャンカー](./chunker-registry.md)