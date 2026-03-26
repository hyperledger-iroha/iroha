---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
כותרת: Cunhar, transferir e queimar um NFT
תיאור: Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.

## Roteiro do livro razao

- Garanta que a definicao do NFT (לדוגמה `n0#wonderland`) exista junto com as contas de dono/destinatario usadas no trecho (`soraカタカナ...`, `soraカタカナ...`).
- הזמנת נקודת כניסה `nft_issue_and_transfer` ל-NFT, העברה של אליס למען בוב והוספה לסירוגין של המטאדים לסירוגין.
- Inspecione o estado do livro razao de NFT com `iroha_cli ledger nfts list --account <id>` או os equivalentes do SDK para verificar a transferencia, depois confirme que o ativo e removido quando a instrucao de queima roda.

## Guias de SDK relacionados

- [התחלה מהירה ל-SDK Rust](/sdks/rust)
- [התחלה מהירה ל-SDK Python](/sdks/python)
- [התחלה מהירה ל-SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

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