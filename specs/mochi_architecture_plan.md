# Mochi implementation boundaries

Mochi is an egui desktop over the shared Kagami managed workspace. It does not
own a second genesis renderer, child-process supervisor, signer vault, compiler
or deployment journal. See [the source ownership and regression checks](../mochi/README.md).

`iroha_deploy::managed` persists a workspace-scoped context under the operating
system's application-state directory, generates a four-validator network using
the canonical localnet generator, and authenticates its native background
worker. Aggregate Start/Stop retain identity and ledger; explicit stopped-network
Reset removes the generation. Closing the desktop leaves the worker running.

`mochi-core::developer::ManagedNetwork` captures a validated generation and its
exact client/operator authorities. State queries, transaction composition and
SDK block/event readers use the generated account. The operator key is separate
and must match all four generated Torii configs. A reset invalidates captured
observers/previews; no automatic switch to a replacement authority occurs.

Desktop deployment uses Musubi's presentation-free runtime, then the canonical
`iroha_contract_deploy` service. A review channel exposes the exact fees and
signed transaction hashes before dispatch. Closing the review cancels; successful
receipts require original-hash Applied evidence and matching artifact/alias
readback. Recovery reviews the retained plan without rebuilding it.

Native filesystem custody is owned by `iroha_fs`; process ownership and IPC by
`iroha_deploy`. Runtime secrets never appear in project files or public desktop
metadata. The installed matching binaries are required for lifecycle actions;
startup performs no build or dependency download.

Owner-private remote dataspace execution and parent-root publication are tracked
in [the shared implementation goals](kagami_mochi_devex_goals.md). The current
managed desktop localnet must not be presented as remote private attachment.
