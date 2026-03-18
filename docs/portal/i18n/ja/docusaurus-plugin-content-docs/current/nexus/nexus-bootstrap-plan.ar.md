---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-bootstrap-plan
title: ソラ Nexus والمراقبة
説明: خطة تشغيلية لتشغيل عنقود المدققين الاساسي لـ Nexus قبل اضافة خدمات SoraFS وソラネット。
---

:::メモ
テストは `docs/source/soranexus_bootstrap_plan.md` です。重要な問題は、次のとおりです。
:::

# خطة اقلاع ومراقبة そら Nexus

## ああ
- تشغيل شبكة المدققين/المراقبين الاساسية لـ Sora Nexus مع مفاتيح الحوكمة واجهات Torii 。
- التحقق من الخدمات الاساسية (Torii، الاجماع، الاستمرارية) قبل تمكين عمليات نشر SoraFS/SoraNet です。
- CI/CD ワークフローとワークフロー。

## ああ、ああ
- مادة مفاتيح الحوكمة (マルチシグ للمجلس، مفاتيح اللجنة) متاحة في HSM او Vault。
- セキュリティ (Kubernetes サービス、ベアメタル サービス) サービス。
- ブートストラップ シンボル (`configs/nexus/bootstrap/*.toml`) のブートストラップ。

## いいえ
- 評価 Nexus 評価:
- **Sora Nexus (メインネット)** - بادئة شبكة انتاج `nexus` تستضيف الحوكمة القانونية وخدمات SoraFS/SoraNet المتراكبة (チェーン ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (テストネット)** - ステージング `testus` تعكس تكوين メインネット لاختبارات التكامل والتحقق قبل الاصدار (チェーン) UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- ジェネシス、ジェネシス、世界、そして世界。 Testus は SoraFS/SoraNet をテストし、Nexus をテストしました。
- CI/CD テストテスト 煙テストテスト Nexus テストテストああ。
- `configs/soranexus/nexus/` (メインネット) と `configs/soranexus/testus/` (テストネット) と `config.toml` و`genesis.json` وادلة قبول Torii نموذجية。

## الخطوة 1 - مراجعة التكوين
1. 回答:
   - `docs/source/nexus/architecture.md` (アイ18NT00000019X)。
   - `docs/source/nexus/deployment_checklist.md` (متطلبات البنية التحتية)。
   - `docs/source/nexus/governance_keys.md` (جراءات حفظ المفاتيح)。
2. ジェネシス (`configs/nexus/genesis/*.json`) の名簿、ステーキング。
3. メッセージ:
   - 定足数です。
   - ファイナリティ / ファイナリティ。
   - منافذ خدمة Torii وشهادات TLS。

## الخطوة 2 - ブートストラップ
1. 回答:
   - شر مثيلات `irohad` (مدققين) مع وحدات تخزين دائمة.
   - ログインして、Torii を確認してください。
2. Torii (REST/WebSocket) TLS を使用します。
3. نشر عقد مراقبة (قراءة فقط) لمرونة اضافية。
4. ブートストラップ (`scripts/nexus_bootstrap.sh`) のジェネシスとテストの結果。
5. 煙テスト:
   - Torii (`iroha_cli tx submit`)。
   - حقق من انتاج/نهائية الكتل عبر التليمتري.
   - فحص تكرار السجل بين المدققين/المراقبين.

## الخطوة 3 - الحوكمة وادارة المفاتيح
1. マルチシグ。重要な問題は、次のとおりです。
2. تخزين مفاتيح الاجماع/اللجنة بشكل امن; عداد نسخ احتياطية تلقائية مع تسجيل وصول。
3. ランブック (`docs/source/nexus/key_rotation.md`) ランブック。

## 4 - CI/CD
1. 評価:
   - 検証ツール/Torii (GitHub Actions および GitLab CI)。
   - التحقق التلقائي من التكوين (lint لـgenesis، تحقق من التواقيع)。
   - ヘルム/カスタマイズ) ステージング を担当します。
2. 喫煙テスト في CI (تشغيل عنقود مؤقت وتشغيل مجموعة المعاملات القانونية)。
3. Runbook のロールバック。

## 回答 5 - 回答
1. セキュリティ (Prometheus + Grafana + アラートマネージャー) 。
2. 回答:
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - ロキ/ELK は Torii です。
3. 説明:
   - صحة الاجماع (ارتفاع الكتلة، النهائية، حالة ピア)。
   - Torii API を使用してください。
   - ログインしてください。
4. 次のとおりです。
   - فوقف انتاج الكتل (>2 فواصل كتل)。
   - ピアの定足数。
   - 回答 Torii。
   - 最高のパフォーマンス。

## 6 - 世界 6 - 世界
1. エンドツーエンド:
   - ارسال مقترح حوكمة (مثل تغيير معلمة)。
   - 最高のパフォーマンスを見せてください。
   - 違いを確認してください。
2. ランブック (フェールオーバー スケーリング)。
3. SoraFS/SoraNet 認証;ピギーバック يمكنه الاشارة الى عقد Nexus。## قائمة تنفيذ
- [ ] 起源/構成情報。
- [ ] شر عقد المدققين والمراقبين مع اجماع سليم。
- [ ] تحميل مفاتيح الحوكمة واختبار المقترح。
- [ ] CI/CD バージョン (ビルド + デプロイ + スモーク テスト)。
- [ ] は、 المراقبة تعمل مع التنبيهات です。
- [ ] ダウンストリームへのハンドオフ。