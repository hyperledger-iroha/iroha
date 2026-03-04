---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
כותרת: Выпустить, перевести и сжечь NFT
תיאור: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метадианс.
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метадиансы.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (נוסח `n0#wonderland`) существует вместе с аккаунтами владельца/полич, в сниппете (`ih58...`, `ih58...`).
- צור קשר עם `nft_issue_and_transfer`, משתמש ב-NFT, פועל גם עם אליס עם בוב ו-Prикрепить флаг мет описывающий выпуск.
- התקן את ה-NFT-reestra через `iroha_cli ledger nfts list --account <id>` או эквиваленты SDK. удаляется после выполнения инструкции לשרוף.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("ih58...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("ih58...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```