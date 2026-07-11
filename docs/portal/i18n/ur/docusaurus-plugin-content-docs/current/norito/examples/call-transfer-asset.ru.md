---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/کال ٹرانسفر-اثاثہ
عنوان: Kotodama سے میزبان ٹرانسفر
تفصیل: یہ ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ بلٹ ان میٹا ڈیٹا چیکنگ کے ساتھ `transfer_asset` میزبان ہدایت کو کال کرسکتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/08_Call_transfer_asset.ko
---

ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ بلٹ ان میٹا ڈیٹا چیکنگ کے ساتھ `transfer_asset` میزبان ہدایت پر کال کرسکتا ہے۔

## قدم بہ قدم رجسٹری ٹریورسل

- کسی معاہدے کے اتھارٹی (جیسے `<i105-account-id>`) کو اثاثہ کے ساتھ تقویت بخشیں وہ اتھارٹی کو `CanTransfer` یا مساوی اجازت کی منتقلی اور جاری کرے گی۔
- کال انٹری پوائنٹ `call_transfer_asset` کو معاہدہ اکاؤنٹ سے 5 یونٹوں کو `<i105-account-id>` میں منتقل کرنے کے لئے ، اس بات کی عکاسی کرتی ہے کہ آن چین آٹومیشن میزبان کالوں کو کیسے لپیٹ سکتا ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس چیک کریں اور اس بات کی تصدیق کے ل investims واقعات کا جائزہ لیں کہ میٹا ڈیٹا گارڈ نے منتقلی کا سیاق و سباق ریکارڈ کیا ہے۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
- [کوئیک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/call-transfer-asset.ko)

```kotodama
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: Amount::from_i64(10),
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
