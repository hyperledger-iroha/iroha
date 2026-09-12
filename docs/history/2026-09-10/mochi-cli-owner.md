# Mochi command-line ownership (2026-09-10)

`mochi/mochi-ui-egui/src/gui/cli_options.rs` owns command selection, validated
startup overrides, environment precedence, profile parsing and usage text.
The GUI and headless sandbox consume the same override context. Rendering,
stream lifetimes, configuration persistence and supervisor startup remain in
their runtime owners. Parser helpers have private visibility; only entry points
and context fields consumed by the GUI module have `pub(super)` visibility.

The GUI decreases from 11,901 to 11,200 lines. Its existing no-growth ceiling
tightens from 11,900 to 11,200; the production limit remains 5,000. The new owner
has 717 lines, its parser tests 462, and GUI integration tests 128. There are
no new size exceptions or numbered source fragments.

All 362 original GUI production function bodies remain token-identical across
their owners. All 48 original CLI test bodies remain token-identical: 39 parser
and precedence tests move alongside the parser, while nine tests of startup,
readiness and supervisor composition stay with the GUI. Existing GUI fixtures
use canonical private owner paths for their environment parsing and binary
configuration. No assertion or validation rule is removed.

Evidence is retained under
`target/architecture-redesign/model-base-extraction-v1/`:

- `mochi-cli-decomposition-v2/` records before/after images and complete function
  and test preservation checks. The first staging attempt failed before source
  application because its test-attribute scanner split a conditional test.
- `mochi-cli-scope-fix-v1/` records two GUI preset-parser calls and fourteen
  environment-fixture calls corrected after the first compilation check.
- `chain-consumers-check-3` passes all targets for 43 packages, including GUI
  and developer binaries. Its 9,273 selected inputs remain unchanged, source
  `cf9fe919e353815fd653572e8e1071278d6c4576221b82aaa7dc9a8a3b62b91d`.
- `mochi-cli-source-budget-1` reports 249 outstanding findings and 170 existing
  exceptions, with the new Mochi growth finding resolved.
- Workspace formatting passes. The source-budget/environment tooling passes
  54 tests in the task's Python environment; the default Python lacks pytest.
  The refreshed environment inventory matches current source. Generated-source
  validation passes for 281 outputs using a private index with the 22 new task
  sources; the real Git index remains unchanged.

`mochi-cli-build-1` and `mochi-cli-runtime-1` pass all **181 GUI tests**, with no
failures or ignored tests, on four ordinary workers. The 7,422 selected inputs
remain unchanged, source
`bd6e12688ce55e14a41f7f430a8297e233552329c644e0a3dee374391e41ac71`;
the retained executable SHA-256 is
`321e88e094ce4b239d629f26fedd53732d77fdda9de8dd2ebfeb24aa662e4bb0`.
`mochi-cli-clippy-1` also passes strict binary/test Clippy on that same source.
All 181 test names and outcomes match the preceding qualified GUI suite; the
39 parser test module paths change with their owner. The build takes 339.92
seconds; test execution takes 26.90 seconds. These
selected checks do not establish whole-workspace or four-validator release
qualification.

The subsequent topology caller migration exposed one further GUI size increase.
Navigation choice, stable persisted identities, labels and tab order now belong
to `gui/navigation.rs`; the GUI retains configuration storage and view rendering.
The two enums and six existing methods retain their exact tokens apart from
necessary parent-module visibility. The GUI decreases from 11,201 to 11,129 lines
and its existing ceiling tightens from 11,200 to 11,129. The new 117-line owner
adds two tests for every persisted identity, label and activity-tab order.

`mochi-navigation-build-1`, `mochi-navigation-runtime-1` and
`mochi-navigation-clippy-1` pass on one unchanged source:
`5f08caedb5e17edfda815d2954a32b26588745d5189416640ec9139def912f33`,
with 7,429 selected inputs. All **183 GUI tests** pass on four ordinary workers;
strict binary/test Clippy also passes. Executable SHA-256 is
`1be5feb654a5c780da7295f1ef137b12a4f57067da16fe2081d1d116e30aa6ef`.
`mochi-navigation-test-comparison-1.json` confirms all 181 previous names and
outcomes remain present and unchanged, with only the two new navigation tests
added. `mochi-navigation-v1/token-review.json` records exact source preservation.
The source budget retains its prior 249 affected paths and 170 exceptions;
full GUI decomposition and broader release qualification remain outstanding.
