# C# local confidential wallet validation — 2026-09-28

The typed `ConfidentialProver` facade uses the existing Core-owned C bridge.
It selects the relation and canonical key natively, accepts one or two actual
notes with bounded commitment history or one path per actual input, and creates
a retained native job before dispatching proof work to the ordinary .NET thread
pool. Disposing the parent rejects new jobs while accepted jobs retain their
own key owner. No caller-selected key, alternative prover, or ledger submission
path was added.

Private managed owners clear their live arrays on disposal and finalization.
Accepted note/tree owners and temporary native argument copies are cleared;
the caller retains responsibility for original arrays, stored change openings,
and scalar/string copies. A job is closed even if dispatch throws before native
consumption. `ConfidentialChangeNote.ToInput(index)` uses Core's default
diversifier after the caller restores the opening and authenticates its index.

## Actual checks

- Exact .NET SDK **8.0.419**, runtime **8.0.25**, macOS arm64. Scoped builds have
  zero warnings/errors.
- **9 tests pass, 0 skipped, 169.693 s**: six managed ownership/boundary controls
  and three actual native controls. The native workflow proves a nondefault-owner
  input of 7, creates change of 2 while redeeming 5, restores that change, and
  proves full redemption of 2 after parent disposal. Invalid membership consumes
  its job while the parent remains usable.
- The public executable example passes through normal P/Invoke loading, producing
  **14,215-byte** change and **13,741-byte** full-redemption proofs. Entropy fills
  preallocated buffers inside the cleanup guard.
- **26 artifact-script tests pass**. The current C# lane also successfully probes
  the actual native ABI and all mandatory wallet, derivation, existing C#, and
  privacy exports. This is an actual artifact check, not only mocked inventory.
- The full artifact-record CLI correctly rejects the dirty source tree. No
  clean-source manifest, loader resolver, release signature, or provenance was
  fabricated to turn that refusal into acceptance.

The exact result JSON backend is `halo2/ipa` for all three relations. The
circuit identity is inside the native self-verified proof envelope. Managed
result checks enforce backend, relation, root, lengths, cardinalities, unique
JSON fields and lowercase hex; a circuit ID in the backend field is rejected.

## Commands and artifact identity

The focused runner is xUnit's executable class selector, because this test
project uses Microsoft.Testing.Platform:

```sh
dotnet build csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj --no-restore
dotnet csharp/tests/Hyperledger.Iroha.Sdk.Tests/bin/Debug/net8.0/Hyperledger.Iroha.Sdk.Tests.dll \
  -class Hyperledger.Iroha.Sdk.Tests.ConfidentialProverTests \
  -class Hyperledger.Iroha.Sdk.Tests.ConfidentialProverNativeTests
dotnet run --project csharp/samples/ConfidentialRedemption/ConfidentialRedemption.csproj --no-restore
python3 -m pytest scripts/tests/check_native_sdk_artifact_test.py scripts/tests/package_csharp_native_artifacts_test.py -q
```

Native execution uses the normal loader with `DYLD_LIBRARY_PATH` pointing to
the retained normal C-bridge build. Its SHA-256 is
`25edc97661608af7b4bb863ea61da3bb4d5dadcdff6d2e0bfb68362de0f6275e`
(146,170,864 bytes, ABI 24). The underlying frozen SDK/native source receipt is
retained with `dist/zk-remediation/2026-09-28/jni-wallet/`. The C# run receipt,
exact commands/logs, final managed assembly hashes, real export inventory,
and initial failed attempts are under
`dist/zk-remediation/csharp-wallet-20260928/`; run-receipt SHA-256:
`7260004b570152377a140f8e7ae84aa9268da11ef7c5fa32dac7713fafa0f243`.

Nine-control source snapshot hashes:

| File | SHA-256 |
| --- | --- |
| `csharp/src/Hyperledger.Iroha.Sdk/Privacy/ConfidentialProver.cs` | `8e12b177e4e104528d3213d4bf8bc0d2767f8f46e83e8246649959c78c6d9f34` |
| `csharp/src/Hyperledger.Iroha.Sdk/Privacy/ConfidentialWalletNative.cs` | `bbb16ba73f2d0e645ccf4bf425673eb328639389e9da97fe6522eb89c1942f2c` |
| `csharp/src/Hyperledger.Iroha.Sdk/Privacy/ConfidentialWalletTypes.cs` | `566135a321f0f9f66b485a95e7203a849f56533b87f181440fab429f36b18e89` |
| `csharp/tests/Hyperledger.Iroha.Sdk.Tests/ConfidentialProverNativeTests.cs` | `64f6abb413b61a16e363bad3e4e52581cf167f1da14af32ec1527ffaae96afb7` |
| `csharp/tests/Hyperledger.Iroha.Sdk.Tests/ConfidentialProverTests.cs` | `af61271aabf11d5c58bad391240b4c870a50b4a7909a71c929caf2a2e9a8e66c` |
| `csharp/samples/ConfidentialRedemption/ConfidentialRedemption.csproj` | `bc9754f6b78aa1610ca4df96f69bf053a1bd3ca3296112c4270ce082d2cbdfbc` |
| `csharp/samples/ConfidentialRedemption/Program.cs` | `15e3397787ebc527b17619d3e099b5c9979d36cedc3b386da86a1cc4346d3606` |
| `csharp/README.md` | `ea3ebff5afac78eeec3ae71fbb117349effb38274c588859ce3a29cf3f813b6a` |
| `csharp/global.json` | `ee7b696ad4c4d01b92aac63d3840414c34c3464e2cd6020ac39a501d98d5a732` |
| `scripts/check_native_sdk_artifact.py` | `e3143d624a3fa554d8cdbc91a35f196b28c93180273fcc5d2d87fcd187e92c8f` |
| `scripts/tests/check_native_sdk_artifact_test.py` | `ab21b83fc3862fbad4156ae0d6637c01adee7627bcf5efdac8d31665ef998ce1` |

## Limits and preserved failures

An initial `dotnet test --filter` invocation was incorrectly scoped: the platform
ignored the VSTest filter and ran **3,342 tests: 1,521 failed, 1,821 passed,
0 skipped**, without the native loader environment. Those failures were not
comprehensively diagnosed by this wallet task; this is not full-suite success.
Two preliminary focused native attempts also failed the new managed result
decoder because it confused the backend field with the circuit identity. Their
logs remain alongside the corrected passing run.

This is **macOS arm64 host evidence** against the recorded normal native build.
It does not qualify five NuGet runtime assets, later unrelated Core/X509 changes,
physical devices, release packaging, or ledger spending authorization. Full
release packaging remains subject to the clean-source, ABI and provenance gates.

## Final collection-bound repair

A final review found repeated `IReadOnlyList.Count` reads between shape validation
and allocation. The tree/path factories and prover now capture each public
count once and use only that admitted value for allocation and iteration. A
regression supplies lists whose second count read returns `int.MaxValue`, covers
all four list boundaries, and completes a mocked transfer after bounded capture.

The fresh final selection passes **10 tests, 0 skipped, 69.250 s** (seven managed
and three actual native). The same-source executable again passes with 14,215-byte
change and 13,741-byte full-redemption proofs. The earlier nine-control receipt
and its exact sources remain unchanged. Final source/assembly hashes and logs
are in `count-bound-run-receipt.json`, SHA-256
`23fd54d91717b20c4bebb52792c1fd5b2ac24175eac6e3e1631a35eae7557290`.

Changed source hashes relative to the nine-control capture:

| File | SHA-256 |
| --- | --- |
| `csharp/src/Hyperledger.Iroha.Sdk/Privacy/ConfidentialProver.cs` | `c5684d9a4fe72b94b5701c313ffbe2c3e2b23eadbd8fe0b2ed04a99d308e97b6` |
| `csharp/src/Hyperledger.Iroha.Sdk/Privacy/ConfidentialWalletTypes.cs` | `35c02802be4664212b7a58e8d5e3a1c474fa408877b00fe0c6d4aa10c8bdb7c7` |
| `csharp/tests/Hyperledger.Iroha.Sdk.Tests/ConfidentialProverTests.cs` | `492c6ec122bcf496aae8ef984fbe2b656af0e968c627fa8035206ede4e109ce0` |

## Separate backend-label regression

The retained current test executable also passes all **87** cases in
`Hyperledger.Iroha.Sdk.Tests.VerifyingKeyBackendTagTests`, with no failures/skips
in 0.395 s. This selection covers exact discriminants/labels, retired aliases
and the replay-binding identity; it is separate from the ten wallet controls.
`verifying-key-backend-receipt.json` retains the exact source and executable hashes.
