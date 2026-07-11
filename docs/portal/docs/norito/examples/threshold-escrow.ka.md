<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54b6d543cff8df6e8fd50632cfed6265770edc33855f06912be603457c5b517e
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-08
translator: machine-google-reviewed
---

---
slug: /norito/examples/threshold-escrow
title: ზღურბლის ესქრო
description: ერთჯერადი გადამხდელის ესქრო, რომელიც იღებს შევსებას ზუსტი სამიზნე ოდენობით, შემდეგ ათავისუფლებს ან აბრუნებს თანხებს.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

ერთჯერადი გადამხდელის ესქრო, რომელიც იღებს შევსებას ზუსტი სამიზნე ოდენობით, შემდეგ ათავისუფლებს ან აბრუნებს თანხებს.

## ლეჯერის გზამკვლევი

- წინასწარ შექმენით ესქროუ ანგარიში და რიცხვითი აქტივის განმარტება, შემდეგ დააფინანსეთ გადამხდელის ანგარიში, რომელიც წარადგენს კონტრაქტის ზარებს. ნიმუში ავტომატურად აკავშირებს გადამხდელს `authority()`-თან `open_escrow`-ის დროს.
- ერთხელ დარეკეთ `open_escrow(recipient, escrow_account, asset_definition, target_amount)`-ზე, რათა ჩაწეროთ გადამხდელი, მიმღები, ესქროული ანგარიში, აქტივის განსაზღვრა, ზუსტი მიზანი და ღია/გამოშვებული/ანაზღაურებადი დროშები გრძელვადიანი კონტრაქტის მდგომარეობაში.
- დარეკეთ `deposit(amount)` იმავე გადამხდელიდან `funded_amount_value == target_amount_value`-მდე; დეპოზიტები უნდა დარჩეს დადებითი და ნებისმიერი შევსება, რომელიც გადააჭარბებს ესქროს, უარყოფილია.
- დარეკეთ `release_if_ready()`-ზე გადატანილი თანხების მიმღებზე გადასატანად მიზნის მიღწევის შემდეგ, ან დარეკეთ `refund()` სანამ ესქრო ჯერ კიდევ ღიაა დაფინანსებული თანხის გადამხდელს დასაბრუნებლად.
- შეამოწმეთ ნაშთები `FindAssetById` / `iroha ledger asset list all --verbose`-ით და შეამოწმეთ კონტრაქტის მდგომარეობა `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json`-თან.

## დაკავშირებული SDK სახელმძღვანელო

- [Rust SDK სწრაფი დაწყება] (/sdks/rust)
- [Python SDK სწრაფი დაწყება] (/sdks/python)
- [JavaScript SDK სწრაფი დაწყება] (/sdks/javascript)

[ჩამოტვირთეთ Kotodama წყარო](/norito-snippets/threshold-escrow.ko)

```kotodama
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to context::authority() when the escrow is opened.
seiyaku ThresholdEscrow {
    error enum EscrowError {
        AlreadyOpen = 1, AlreadyReleased = 2, AlreadyRefunded = 3, NotOpen = 4, UnauthorizedPayer = 5, NonPositiveTarget = 6, NonPositiveAmount = 7, TargetExceeded = 8, NotFullyFunded = 9,
    }

    const string recipient_account_literal = "sorauﾛ1PｽNgｿﾘ9ﾏﾕ2ﾕ9ﾄZﾀﾃﾌWwNｸｾヰﾄﾂT3WｺTxｶｵﾎKﾓﾛmｷ4Y6PLN";
    const string escrow_account_literal = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
    const string escrow_asset_definition_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    state AccountId payer_account;
    state AccountId recipient_account;
    state AccountId escrow_account_id;
    state AssetDefinitionId escrow_asset_definition;
    state quantity target_amount_value;
    state quantity funded_amount_value;
    state bool is_open;
    state bool is_released;
    state bool is_refunded;
    hajimari() {
        let quantity zero = 0;
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
    kotoage fn open_escrow(quantity target_amount) authorize("Admin") {
        assert_unopened();
        let quantity zero = 0;
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

    kotoage fn deposit(quantity amount) authorize("Admin") {
        assert_open();
        assert_payer();
        require(amount > 0, EscrowError::NonPositiveAmount);
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
        if (funded > 0) {
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
