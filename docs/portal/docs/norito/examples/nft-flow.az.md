<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: NFT-ni köçürün, köçürün və yandırın
description: NFT-nin həyat dövrünün sonundan sona qədər gedir: sahibinə zərbetmə, metadatanın ötürülməsi, etiketlənməsi və yandırılması.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT-nin həyat dövrünün sonundan sona qədər gedir: sahibinə zərbetmə, metadatanın ötürülməsi, etiketlənməsi və yandırılması.

## Ledger prospekti

- NFT tərifinin (məsələn, `n0#wonderland`) fraqmentdə istifadə edilən sahib/qəbuledici hesabları ilə yanaşı mövcud olduğundan əmin olun (Alice üçün `<i105-account-id>`, Bob üçün `<i105-account-id>`).
- NFT-ni hazırlamaq üçün `nft_issue_and_transfer` giriş nöqtəsini çağırın, onu Alicedən Boba köçürün və buraxılışı təsvir edən metadata bayrağı əlavə edin.
- Köçürməni yoxlamaq üçün `iroha_cli ledger nft list --account <id>` və ya SDK ekvivalentləri ilə NFT jurnalının vəziyyətini yoxlayın, sonra yandırma təlimatı işlədikdən sonra aktivin silindiyini təsdiqləyin.

## Əlaqədar SDK təlimatları

- [Rust SDK sürətli başlanğıc](/sdks/rust)
- [Python SDK sürətli başlanğıc](/sdks/python)
- [JavaScript SDK sürətli başlanğıc](/sdks/javascript)

[Kotodama mənbəyini endirin](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  #[access(read="*", write="*")]
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```