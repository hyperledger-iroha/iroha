---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
title: خطة اعادة هيكلة دفتر Sora Nexus
説明: نسخة مطابقة لـ `docs/source/nexus_refactor_plan.md` توضح اعمال التنظيف المرحلية لقاعدة شفرة Iroha 3.
---

:::メモ
テストは `docs/source/nexus_refactor_plan.md` です。特別な日は、世界のすべての人々に感謝します。
:::

# خطة اعادة هيكلة دفتر Sora Nexus

ソラ Nexus 元帳 ("Iroha 3")。開発者は、genesis/WSV Sumeragi واشغلات を開発しました。ポインター ABI Norito。 هدف هو الوصول الى معمارية متماسكة قابلة للاختبار دون محاولة ادخال كل الاصلاحات فيさいごに。

## 0. مبادئ موجهة
- حفاظ على سلوك حتمي عبر عتاد متنوع؛機能フラグとフォールバックを表示します。
- Norito هيطبقة التسلسل。往復の料金は、Norito وتحديث の設備です。
- `iroha_config` (ユーザー -> 実際の -> デフォルト)。 「 」は、「 」と「 」を切り替えます。
- ABI バージョン V1 バージョン。ポインター タイプ/システムコールの説明。
- `cargo test --workspace` とゴールデン テスト (`ivm`、`norito`、`integration_tests`) をテストします。

## 1. طوبولوجيا المستودع
- `crates/iroha_core`: Sumeragi وWSV ジェネシス ウイルス (クエリ、オーバーレイ、ZK レーン) ウイルス。
- `crates/iroha_data_model`: ログインしてください。
- `crates/iroha`: CLI および SDK 。
- `crates/iroha_cli`: CLI インターフェイス インターフェイス、API インターフェイス、`iroha`。
- `crates/ivm`: ポインタ ABI のポインタ。
- `crates/norito`: コーデックは、JSON と AoS/NCB を表します。
- `integration_tests`: ジェネシス/ブートストラップ、Sumeragi、トリガー、ページネーション。
- ソラ Nexus 台帳 (`nexus.md`、`new_pipeline.md`、`ivm.md`) を表示します。 ومتهالك جزئيا مقارنة بالشفرة。

## 2. いいえ、いいえ。

### المرحلة A - المراقبة
1. **テレメトリア WSV + スナップショット**
   - API を使用して `state` (特性 `WorldStateSnapshot`) を使用して Sumeragi CLI を使用します。
   - `scripts/iroha_state_dump.sh` スナップショット `iroha state dump --format norito`。
2. **ジェネシス/ブートストラップ**
   - ジェネシス Norito (`iroha_core::genesis`)。
   - 解析/解析/解析/生成/生成/生成 WSV 解析 arm64/x86_64 (متابعة في `integration_tests/tests/genesis_replay_determinism.rs`)。
3.*******************
   - `integration_tests/tests/genesis_json.rs` は、WSV とパイプライン、ABI、ハーネスを表示します。
   - 足場 `cargo xtask check-shape` スキーマ ドリフト (バックログ、DevEx、開発、`scripts/xtask/README.md`)。

### المرحلة B - WSV وسطح الاستعلام
1. **重要な要素**
   - دمج `state/storage_transactions.rs` في محول معاملات يفرض ترتيب commit وكشف التعارض.
   - アセット/ワールド/トリガーを確認してください。
2. **اعادة هيكلة نموذج الاستعلام**
   - ページネーション/カーソルは、`crates/iroha_core/src/query/` を使用して実行されます。 Norito と `iroha_data_model` です。
   - スナップショット クエリは、トリガー、アセット、ロールを管理します (`crates/iroha_core/tests/snapshot_iterable.rs` للتغطية الحالية)。
3.*********
   - CLI `iroha ledger query` セキュリティ Sumeragi/フェッチャー。
   - 回帰の問題、CLI テスト、`tests/cli/state_snapshot.rs` (機能ゲート型テスト)。

### المرحلة C - خط انابيب Sumeragi
1.******
   - `EpochRosterProvider` 特性とステーク WSV のスナップショット。
   - `WsvEpochRosterAdapter::from_peer_iter` ベンチ/テスト。
2. **評価と評価**
   - `crates/iroha_core/src/sumeragi/*` バージョン: `pacemaker`、`aggregation`、`availability`、`witness` `consensus`。
   - テスト テスト テスト テスト Norito テスト テスト テスト (バックログ) Sumeragi)。
3. **レーン/プルーフ**
   - レーンプルーフ、DA、RBC、ゲートゲート。
   - エンドツーエンド `integration_tests/tests/extra_functional/seven_peer_consistency.rs` は RBC をサポートします。### المرحلة D - العقود الذكية ومضيفات pointer-ABI
1.******
   - ポインター型 (`ivm::pointer_abi`) および (`iroha_core::smartcontracts::ivm::host`)。
   - マニフェスト `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` و`ivm_host_mapping.rs` TLV ゴールデン。
2. **サンドボックストリガー**
   - 応答は、`TriggerExecutor` 応答をトリガーします。
   - 回帰分析、通話/時間の分析 (متابعة عبر `crates/iroha_core/tests/trigger_failure.rs`)。
3. **محاذاة CLI والعميل**
   - CLI (`audit`、`gov`、`sumeragi`、`ivm`) を使用してください。 `iroha` は、`iroha` です。
   - スナップショット JSON、CLI、`tests/cli/json_snapshot.rs`; JSON を使用して、JSON を使用してください。

### المرحلة E - コーデック Norito
1.******
   - Norito تحت `crates/norito/src/schema/` الميزات قانونية للانواع الاساسية.
   - ドキュメント テストのペイロード (`norito::schema::SamplePayload`)。
2. **黄金の備品**
   - ゴールデン フィクスチャ `crates/norito/tests/*` を表示し、WSV を表示します。
   - `scripts/norito_regen.sh` يعيد توليد ゴールデン JSON لنوريتو بشكل حتمي عبر المساعد `norito_regen_goldens`。
3. **IVM/Norito**
   - マニフェスト Kotodama マニフェスト Kotodama マニフェスト Norito メタデータ ポインター ABI。
   - `crates/ivm/tests/manifest_roundtrip.rs` يحافظ على تساوي Norito エンコード/デコード。

## 3. قضايا مشتركة
- ** テスト**: 単体テスト -> クレート テスト -> 統合テスト。ニュース ニュース ニュース ニュース ニュース ニュースありがとうございます。
- **التوثيق**: بعد كل مرحلة، حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذفそうです。
- **コメント*: ベンチ `iroha_core` و`ivm` و`norito`;回帰現象が発生しました。
- **機能フラグ**: ツールチェーン ツールチェーン (`cuda`, `zk-verify-batch`)。 SIMD は CPU の性能を向上させます。フォールバックは、フォールバックと呼ばれます。

## 4. いいえ
- 足場 A (スナップショット特性 + テレメトリア) - ロードマップの構築。
- 評価 `sumeragi` و`state` و`ivm` 評価:
  - `sumeragi`: デッドコードの表示変更、VRF の再生、テレメトリア EMA の許容量。レーン/プルーフのゲート付きゲートウェイを作成します。
  - `state`: `Cell` テレメトリア WSV テレメトリア WSV テレメトリアSoA/並列適用のバックログとパイプラインの並列適用 C.
  - `ivm`: CUDA を切り替えて、Halo2/Metal エンベロープと Halo2/Metal を切り替えます。 GPU を使用するカーネルとバックログ GPU が表示されます。
- RFC の承認と承認の承認。

## 5. 大事なこと
- هل يجب ان يبقى RBC اختياريا بعد P1، ام انه الزامي لخطوط دفتر Nexus؟ログインしてください。
- DS と P1 の構成可能性を確認し、レーン プルーフを確認します。
- ML-DSA-87 メッセージ名前: 木箱 `crates/fastpq_isi` (قيد الانشاء)。

---

_回答日: 2025-09-12_