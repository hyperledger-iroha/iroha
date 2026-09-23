# Exact account read permission at Torii routing

`torii_authorize_signed_query_routes` now honors the exact native
`CanReadAccountData { account }` permission for a classified `TargetAccount`
signed query. Previously Core authorized that grant, but Torii removed the
account's restricted routes before Core could execute the request. A reader
holding only the delegated account grant could not reach that private account.

The existing permission helper checks direct and role grants. The change does
not grant dataspace-wide or ledger-wide reads, alter grant/revoke authority, or
replace Core's mandatory query validation. Alias queries retain their separate
exact alias permission. No native balance policy or wire format changes.

The regression
`signed_query_authorization_exact_account_grant_reads_only_its_restricted_target`
checks wrong-account denial, exact grant success and denial after committed
revocation. It exercises account lookup, an explicitly scoped asset lookup and
account-asset query route selection; history and alias routing remain denied.
The mutation and regression are in `crates/iroha_torii/src/lib.rs` and
`crates/iroha_torii/src/tests/lib_runtime_handlers/part_2.rs`.

This is a routing-authorization regression, not end-to-end query evidence.
Current bounded iterable fanout remains unsupported, so successful route
selection for `FindAssetsByAccountId` does not prove enumeration through
`/query`. Exact `FindAssetById` uses the singular route. The BOI consumer uses
one explicit DS definition and balance scope for those exact reads.

The focused regression passed (one passed, none failed or ignored) in the
persistent `boi-is2-design` lane on 2026-09-22:

```sh
cargo iroha-fast --target-slot boi-is2-design --stable-local-metadata -- \
  test --locked --offline -p iroha_torii --lib \
  signed_query_authorization_exact_account_grant_reads_only_its_restricted_target
```

The build completed in 46m 24s and the test ran in 3.63s. The shared checkout
advanced from the initial mutable `6153ef...` candidate to
`8209033dea...` during this build. This record makes no whole-commit or release
qualification claim. Current model enum/schema compilation failures reported
by a separate BOI native consumer are tracked with their native owners.
