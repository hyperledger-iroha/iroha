---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/ٹرانسفر اثاثہ
عنوان: اکاؤنٹس کے مابین اثاثہ منتقل کریں
تفصیل: اثاثہ کی منتقلی کا آسان بہاؤ جو SDK کوئیک اسٹارٹ اور رجسٹری واک کا آئینہ دار ہے۔
ماخذ: مثالوں/منتقلی/منتقلی.KO
---

اثاثوں کی منتقلی کا آسان بہاؤ جو SDK کوئیک اسٹارٹ اور رجسٹری واک کا آئینہ دار ہے۔

## رجسٹری براؤزنگ

- ہدف اثاثہ کے ساتھ پری فنڈ ایلس (جیسے اسنیپٹ `register and mint` یا SDK کوئیک اسٹارٹ فیڈ کے ذریعے)۔
- انٹری پوائنٹ `do_transfer` پر عمل کریں تاکہ 10 یونٹوں کو ایلس سے باب میں منتقل کیا جاسکے ، اجازت نامہ `AssetTransferRole`۔
- استفسار بیلنس (`FindAccountAssets` ، `iroha_cli ledger assets list`) یا منتقلی کے نتائج کا مشاہدہ کرنے کے لئے پائپ لائن واقعات کو سبسکرائب کریں۔

## متعلقہ SDK گائیڈز

- [کوئک اسٹارٹ SDK مورچا] (/sdks/rust)
- [کوئک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"), AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), Amount::from_i64(10), DataSpaceId::parse("0"));
    }
}
```
