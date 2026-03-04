---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
Название: Invoquer le Transfert Hôte depuis Kotodama
описание: Демонстрируйте комментарий к точке входа Kotodama, который может вызвать инструкцию Hôte `transfer_asset` с проверкой метадоннеев на линии.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Прокомментируйте точку входа Kotodama, чтобы получить инструкцию по `transfer_asset` с проверкой метадонов на линии.

## Парк регистрации

- Утвердить полномочия по контракту (например, `ih58...`) с учетом действий, которые выполняются при передаче и в соответствии с ролью `CanTransfer` или эквивалентным разрешением.
- Нажмите точку входа `call_transfer_asset` для передачи 5 единиц счета по контракту версии `ih58...`, и обратите внимание на то, что автоматизация в цепочке не может инкапсулировать апелляции.
- Проверьте продажи через `FindAccountAssets` или `iroha_cli ledger assets list --account ih58...` и проверьте события для подтверждения того, что охрана метадонников и журналистский контекст передачи.

## Руководства для партнеров SDK

- [Быстрый запуск SDK Rust](/sdks/rust)
- [Быстрый запуск SDK Python] (/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Зарядите источник Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```