---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
титул: Cunhar, Transferir e queimar um NFT
описание: Выполните цикл жизни NFT, чтобы начать работу: cunhagem para o dono, Transferencia, marcacao de Metadados e queima.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Выполните цикл жизни NFT, чтобы начать работу: использовать для передачи, переноса, выделения метаданных и получения.

## Ротейру до Ливро Разау

- Гарантия, что определенный NFT (например, `n0#wonderland`) существует вместе с данными донора/назначения, используемого без использования (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer` для запуска NFT, передачи Алисы для Боба и подключения к синальным метаданным, которые были получены и отправлены.
- Проверив состояние NFT-ливера с помощью `iroha_cli ledger nfts list --account <id>` или эквивалентов SDK для проверки передачи, необходимо подтвердить, что он активен, и удалить его, как только будет получено указание о роде.

## Рекомендации по использованию SDK

- [Краткий старт работы с SDK Rust](/sdks/rust)
- [Краткий старт работы с SDK Python](/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Вставьте шрифт Kotodama](/norito-snippets/nft-flow.ko)

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