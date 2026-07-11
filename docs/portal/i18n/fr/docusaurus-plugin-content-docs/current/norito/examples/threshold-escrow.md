<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 270c9c1079659b7c0f66d20b22a4453620d87bdb2877e66600db4a4b844c7924
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
slug: /norito/examples/threshold-escrow
title: Séquestre de seuil
description: Un dépôt à payeur unique qui accepte les recharges jusqu'à un montant cible exact, puis libère ou rembourse les fonds.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

Un dépôt à payeur unique qui accepte les recharges jusqu'à un montant cible exact, puis libère ou rembourse les fonds.

## Procédure pas à pas du grand livre

- Pré-créez le compte séquestre et la définition numérique de l'actif, puis approvisionnez le compte payeur qui soumettra les appels de contrat. L'exemple lie automatiquement ce payeur à `authority()` pendant `open_escrow`.
- Appelez `open_escrow(recipient, escrow_account, asset_definition, target_amount)` une fois pour enregistrer le payeur, le destinataire, le compte séquestre, la définition de l'actif, la cible exacte et les indicateurs d'ouverture/libération/remboursement dans un état de contrat durable.
- Appeler `deposit(amount)` du même payeur jusqu'à `funded_amount_value == target_amount_value` ; les dépôts doivent rester positifs et tout complément qui risquerait de surfinancer le séquestre est rejeté.
- Appelez `release_if_ready()` pour transférer les fonds bloqués vers le destinataire une fois l'objectif atteint, ou appelez `refund()` pendant que le dépôt est toujours ouvert pour restituer le montant financé au payeur.
- Inspecter les soldes avec `FindAssetById` / `iroha_cli ledger asset list` et inspecter l'état du contrat avec `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json`.

## Guides SDK associés

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/threshold-escrow.ko)

```kotodama
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to context::authority() when the escrow is opened.
seiyaku ThresholdEscrow {
    error enum EscrowError {
        AlreadyOpen = 1, AlreadyReleased = 2, AlreadyRefunded = 3, NotOpen = 4, UnauthorizedPayer = 5, NonPositiveTarget = 6, NonPositiveAmount = 7, TargetExceeded = 8, NotFullyFunded = 9,
    }

    const recipient_account_literal: string = "sorauﾛ1PｽNgｿﾘ9ﾏﾕ2ﾕ9ﾄZﾀﾃﾌWwNｸｾヰﾄﾂT3WｺTxｶｵﾎKﾓﾛmｷ4Y6PLN";
    const escrow_account_literal: string = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
    const escrow_asset_definition_literal: string = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    state payer_account: AccountId;
    state recipient_account: AccountId;
    state escrow_account_id: AccountId;
    state escrow_asset_definition: AssetDefinitionId;
    state target_amount_value: Amount;
    state funded_amount_value: Amount;
    state is_open: bool;
    state is_released: bool;
    state is_refunded: bool;
    hajimari() {
        let zero = Amount::from_i64(0);
        payer_account = context::authority();
        recipient_account = AccountId::parse(recipient_account_literal);
        escrow_account_id = AccountId::parse(escrow_account_literal);
        escrow_asset_definition = AssetDefinitionId::parse(escrow_asset_definition_literal);
        target_amount_value = zero;
        funded_amount_value = zero;
        is_open = false;
        is_released = false;
        is_refunded = false;
    }

    fn assert_unopened() {
        require(!is_open, EscrowError::AlreadyOpen);
        require(!is_released, EscrowError::AlreadyReleased);
        require(!is_refunded, EscrowError::AlreadyRefunded);
    }

    fn assert_open() {
        require(is_open, EscrowError::NotOpen);
        require(!is_released, EscrowError::AlreadyReleased);
        require(!is_refunded, EscrowError::AlreadyRefunded);
    }

    fn assert_payer() {
        require(context::authority() == payer_account, EscrowError::UnauthorizedPayer);
    }

    // NOTE:
    // This sample uses authorize("Admin") because it releases and refunds funds
    // from the configured escrow account. The recipient, escrow account, and
    // asset definition are fixed literals so the compiler can emit a complete
    // first-release access set without manual annotations.
    kotoage fn open_escrow(target_amount: Amount) authorize("Admin") {
        assert_unopened();
        let zero = Amount::from_i64(0);
        require(target_amount > zero, EscrowError::NonPositiveTarget);
        payer_account = context::authority();
        recipient_account = AccountId::parse(recipient_account_literal);
        escrow_account_id = AccountId::parse(escrow_account_literal);
        escrow_asset_definition = AssetDefinitionId::parse(escrow_asset_definition_literal);
        target_amount_value = target_amount;
        funded_amount_value = zero;
        is_open = true;
        is_released = false;
        is_refunded = false;
    }

    kotoage fn deposit(amount: Amount) authorize("Admin") {
        assert_open();
        assert_payer();
        require(amount > Amount::from_i64(0), EscrowError::NonPositiveAmount);
        let next_funded = funded_amount_value + amount;
        require(next_funded <= target_amount_value, EscrowError::TargetExceeded);
        ledger::asset::transfer(
            source: context::authority(),
            destination: AccountId::parse(escrow_account_literal),
            asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
            amount: amount,
            dataspace: DataSpaceId::parse("0"),
        );
        funded_amount_value = next_funded;
    }

    kotoage fn release_if_ready() authorize("Admin") {
        assert_open();
        require(funded_amount_value == target_amount_value, EscrowError::NotFullyFunded);
        ledger::asset::transfer(
            source: AccountId::parse(escrow_account_literal),
            destination: AccountId::parse(recipient_account_literal),
            asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
            amount: funded_amount_value,
            dataspace: DataSpaceId::parse("0"),
        );
        is_open = false;
        is_released = true;
    }

    kotoage fn refund() authorize("Admin") {
        assert_open();
        assert_payer();
        let funded = funded_amount_value;
        if (funded > Amount::from_i64(0)) {
            ledger::asset::transfer(
                source: AccountId::parse(escrow_account_literal),
                destination: context::authority(),
                asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
                amount: funded,
                dataspace: DataSpaceId::parse("0"),
            );
        }
        is_open = false;
        is_refunded = true;
    }
}
```
