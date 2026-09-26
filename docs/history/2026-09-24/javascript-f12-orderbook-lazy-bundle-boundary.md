# F12 optional SoraFS orderbook JavaScript bundle boundary

The Torii eager browser entry exceeded its unchanged 797 KiB ceiling at 819,019
bytes. A virtual-source comparison against the JavaScript files before the F11
ballot update measured the same eager size, so that update did not cause the
overage. The optional SoraFS orderbook submission helpers were still imported
statically by the Torii client even though that path is async and already has
an optional-module loader.

The Torii client now loads those helpers through the existing `toriiOptional`
boundary before orderbook submission side effects. The ambiguity error class
has a tiny eager module and is re-exported by the submission module, preserving
its public identity and cause fields. This is the same canonical implementation,
not a second protocol or compatibility path.

`npm run build:dist` and the complete `npm run bundle:check` pass. The Torii
eager entry is 811,612 bytes (under 797 KiB), optional lazy closure 251,910
bytes (under 322 KiB), Sumeragi lazy closure 65,896 bytes (under 72 KiB), and
combined 1,129,418 bytes (under 1,191 KiB) after the F11 strict public-cast
boundary was added. The focused pure tests pass 7/7,
including direct/public ambiguity class identity and the lazy helper surface;
scoped ESLint and `git diff --check` pass. Native-backed orderbook tests await
rebuilding the same-source host module; the source-provenance guard correctly
rejects the older module. Final cross-SDK and five-target package gates remain
open.
