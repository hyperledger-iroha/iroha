---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
Название: نقل أصل بين الحسابات
описание: Создан для создания SDK-файла, созданного с помощью SDK.
источник: example/transfer/transfer.ko
---

Используйте его для создания SDK-файла.

## جولة دفتر الأستاذ

- В исполнении Элис Бэйлз Миссисипи (в роли Стоуна в фильме "`register and mint` أو") Используйте SDK).
- В 10-м году от Алисы Боба появился `do_transfer`. `AssetTransferRole`.
- استعلم عن الأرصدة (`FindAccountAssets`, `iroha_cli ledger assets list`) и في أحداث خط الأنابيب Будьте добры.

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```