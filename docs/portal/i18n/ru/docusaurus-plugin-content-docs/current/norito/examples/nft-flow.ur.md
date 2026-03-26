---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
Название: NFT کو منٹ، منتقل ار برن کریں
описание: NFT کے کے لائف سائیکل کو ابتدا سے انتہا تک دکھاتا ہے: NFT کو کنٹ کرنا، منتقل Если вы хотите, чтобы это произошло, вы можете сделать это самостоятельно.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT-файл может быть использован в качестве основного инструмента: если вы хотите использовать NFT Если вы хотите, чтобы это произошло, вы можете сделать это самостоятельно.

## لیجر واک تھرو

- Поддержка NFT-адаптера (с `n0#wonderland`) для получения дополнительной информации Встроенное программное обеспечение/программное обеспечение (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`) ہوں۔
- `nft_issue_and_transfer` позволяет использовать NFT для игры с Алисой и Бобом, когда он работает. Если вы хотите, чтобы вы знали, как это сделать,
- `iroha_cli ledger nfts list --account <id>` содержит SDK, обеспечивающий поддержку NFT-файлов и возможность использования NFT-файлов. Если вы хотите, чтобы вы выбрали лучший вариант для себя ہو جاتا ہے۔

## Использование SDK

- [Краткий старт Rust SDK](/sdks/rust)
- [Краткий старт Python SDK](/sdks/python)
- [Краткое руководство по JavaScript SDK] (/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<katakana-i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<katakana-i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```