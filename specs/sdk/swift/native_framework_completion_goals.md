# Swift native framework completion goals

The Swift SDK requires the real ABI-25 NoritoBridge native implementation.
The build and artifact contract is [NoritoBridge release packaging](../../../docs/norito_bridge_release.md).

| Goal | Owner | Completion criteria |
| --- | --- | --- |
| SW1: Build the native framework | Bridge/build | Build all five supported Apple targets from the current source and locked graph; preserve the warm build lane; pass whole-archive consumer, ABI, symbol and artifact checks. |
| SW2: Make SDK installation usable | Swift/package | Integrate the authenticated framework through the supported package path; validate SwiftPM and relevant packaged consumers; correct broken build or installation instructions. |
| SW3: Qualify the Swift SDK | Swift/tests | Run the complete Swift package suite with native support and fix failures; verify query, signing, codec and native lifecycle behavior without disabled tests or replacement implementations. |
| SW4: Keep delivery claims accurate | SDK/docs | Record the actual artifact and checks, update current blockers, and distinguish host/simulator validation from physical-device and signed public-release qualification. |

Implementation and current artifacts are authoritative. Routine validation
commands and results belong in the change report; generated artifacts stay
untracked.

Current state: SW1 is active in the fixed local integration build lane. SW2's
Cargo configuration isolation, SwiftPM export retention and workflow lock
handoffs are implemented; actual native consumer qualification remains open.
SW3 and SW4 await the completed framework and Swift validation. A local
integration artifact cannot authorize a signed public release.
