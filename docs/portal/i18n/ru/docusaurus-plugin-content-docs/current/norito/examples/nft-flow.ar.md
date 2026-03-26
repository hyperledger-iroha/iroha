---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
титул: Сэнсэй Уоррен NFT
Описание: Создано в NFT в честь события: السك للمالك، النقل, ووسم بيانات التعريف, والحرق.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Создан для NFT в Новом году: Стоун-Луи, Нью-Йорк, Нью-Йорк التعريف, والحرق.

## جولة دفتر الأستاذ

- Создано для NFT (с кодом `n0#wonderland`) и используется для восстановления/разгрузки. Установите флажок (`i105...`, `i105...`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` لسك NFT ونقله من من من إلى Bob علامة بيانات تعريف تصف الإصدار.
- Создайте NFT-файл `iroha_cli ledger nfts list --account <id>` и установите SDK в нужном месте. Он сказал, что хочет сделать это.

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```