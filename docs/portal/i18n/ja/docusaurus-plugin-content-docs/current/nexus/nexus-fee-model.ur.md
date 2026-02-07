---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
title: Nexus فیس ماڈل اپ ڈیٹس
説明: `docs/source/nexus_fee_model.md` レーン決済の受領書と調整面の表示
---

:::note ٩ینونیکل ماخذ
یہ صفحہ `docs/source/nexus_fee_model.md` کی عکاسی کرتا ہے۔ جاپانی، عبرانی، ہسپانوی، پرتگالی، فرانسیسی، روسی، عربی اور اردو ترجمے 移行 ہونے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Nexus فیس ماڈل اپ ڈیٹس

決済ルータ、レーン、確定的な領収書、ガスの引き落とし、Nexus、確定的な領収書、和解する

- ルータのアーキテクチャ、バッファ ポリシー、テレメトリ マトリックス、ロールアウト シーケンス、`docs/settlement-router.md` および `docs/settlement-router.md`パラメータ NX-3 ロードマップの成果物 SRE 本番環境 ルータ パラメータऔर देखें
- ガス資産構成 (`pipeline.gas.units_per_gas`) `twap_local_per_xor` 10 進数 `liquidity_profile` (`tier1`、`tier2`、`tier3`) `volatility_class` (`stable`、`elevated`、`dislocated`)フラグ決済ルータ フィード ہیں تاکہ حاصل ہونے والی XOR 引用 正規 TWAP レーン کے ヘアカット層 سے میل کھائے۔
- ガスの支払い トランザクション ایک `LaneSettlementReceipt` ریکارڈ کرتی ہے۔レシートの発信者 ソース識別子 ローカルの微額 金額 XOR ヘアカット 期待される XOR 分散 (`xor_variance_micro`)ブロックのタイムスタンプ (ミリ秒)
- ブロック実行レシート レーン/データスペース 集計 集計 `/v1/sumeragi/status` میں `lane_settlement_commitments` ذریعے شائع کرتا ❁❁❁❁合計 `total_local_micro`、`total_xor_due_micro`、`total_xor_after_haircut_micro` ブロック数 夜間調整エクスポート数فراہم ہوتے ہیں۔
- 番号 `total_xor_variance_micro` カウンター トラック 番号 番号 安全マージン番号 (XOR によるヘアカット後の期待値) `swap_metadata` 決定論的変換パラメータ (TWAP、イプシロン、流動性プロファイル、ボラティリティ クラス) 監査実行時設定 監査の実行時設定 見積入力 評価ああ

消費者 `lane_settlement_commitments` レーン データスペース コミットメント スナップショット ساتھ دیکھ سکتے ہیں تاکہ یہ تصدیق ہو کہ 料金バッファー ヘアカットスワップ実行が構成された層 Nexus 料金モデル