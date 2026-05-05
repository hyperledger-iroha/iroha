---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
כותרת: Invocar transferencia do host a partir de Kotodama
תיאור: נקודת כניסה Kotodama ותדריך לארח את `transfer_asset` עם תקינות מתקדמות.
מקור: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

יש לך נקודת כניסה Kotodama כדי לארח את `transfer_asset` עם תקינות מוטבעות.

## Roteiro do livro razao

- Financie a autoridade do contrato (por exemplo `<i105-account-id>`) com o ativo que ela transferira e conceda a autoridade o papel `CanTransfer` ou permissao equivalente.
- Chame o entrypoint `call_transfer_asset` עבור העברה של 5 יחידות קונטרה קונטרה עבור `<i105-account-id>`, refletindo como a automacao on-chain pode envolver chamadas do host.
- Verifique saldos via `FindAccountAssets` או `iroha_cli ledger assets list --account <i105-account-id>` e inspecione eventos para confirmar que o guard de metadados registrou o contexto da transferencia.

## Guias de SDK relacionados

- [התחלה מהירה ל-SDK Rust](/sdks/rust)
- [התחלה מהירה ל-SDK Python](/sdks/python)
- [התחלה מהירה ל-SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```