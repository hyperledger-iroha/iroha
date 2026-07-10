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

- Garanta que a definicao do NFT (לדוגמה `n0#wonderland`) exista junto com as contas de dono/destinatario usadas no trecho (`<i105-account-id>`, `<i105-account-id>`).
- הזמנת נקודת כניסה `nft_issue_and_transfer` ל-NFT, העברה של אליס למען בוב והוספה לסירוגין של המטאדים לסירוגין.
- Inspecione o estado do livro razao de NFT com `iroha_cli ledger nfts list --account <id>` או os equivalentes do SDK para verificar a transferencia, depois confirme que o ativo e removido quando a instrucao de queima roda.

## Guias de SDK relacionados

- [התחלה מהירה ל-SDK Rust](/sdks/rust)
- [התחלה מהירה ל-SDK Python](/sdks/python)
- [התחלה מהירה ל-SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
        ledger::nft::transfer(owner, nft, to);
        ledger::nft::set_metadata(nft, Name::parse("issued"), Json::parse("{\"issued\":\"demo\"}"));
        ledger::nft::burn(nft);
    }
}
```
