---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/ٹرانسفر اثاثہ
عنوان: اکاؤنٹس کے مابین اثاثہ کی منتقلی
تفصیل: ایک سادہ اثاثہ منتقلی کا ورک فلو جو SDK اسپرنٹ اور لیجر راؤنڈ کی عکاسی کرتا ہے۔
ماخذ: مثالوں/منتقلی/منتقلی.KO
---

سادہ اثاثہ منتقلی کا ورک فلو جو SDK جمپ اسٹارٹس اور لیجر راؤنڈ کی عکاسی کرتا ہے۔

## لیجر ٹور

- پہلے ہی ہدف اثاثہ کے ساتھ ایلس کو فنڈ (جیسے اسنیپیٹ `register and mint` یا SDK کوئیک اسٹارٹ فلو کے ذریعے)۔
- انٹری پوائنٹ `do_transfer` پر عمل کریں `AssetTransferRole` مطمئن کے ساتھ ایلس سے باب میں 10 یونٹ منتقل کریں۔
- بیلنس (`FindAccountAssets` ، `iroha_cli ledger assets list`) کے بارے میں پوچھ گچھ کریں یا منتقلی کے نتائج کو دیکھنے کے لئے پائپ لائن واقعات کو سبسکرائب کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر SDK کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public kotoage declaration to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", ),
            destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
