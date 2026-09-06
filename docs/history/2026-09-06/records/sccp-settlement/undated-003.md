# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-1d25e8ccf244c77e90a59582c1e58605b3934f5f455f548cbe906932fa60e1bb"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP helper CLI diagnostics must stay bounded before operator logs enter
  release artifacts: source, destination, receipt-proof, EVM live, Solana/TON/TRON
  live, and TRON source bridge helpers must redact sensitive top-level failures
  to fixed evidence categories, with adversarial `secret-token` regressions pinned
  in the release public scalar-text source inventory. That inventory now pins
	  the actual adversarial token inputs for EVM receipt-proof, EVM live/source-live,
	  TON live, and TRON live parser/transport failures, not only generic
	  `secret-token` absence assertions. Direct EVM destination and EVM-family
	  source-bridge scalar/runtime hex parsers must convert helper `SystemExit`,
	  `RuntimeError`, `TypeError`, and `ValueError` drift into fixed hex
	  blockers. ETH/BSC source-bridge, EVM destination, Solana
	  destination/source-state, and TON destination/source-state fixed-hex/fixed-byte
	  parser helpers must also keep exact boolean `nonzero` controls before zero
	  source or destination material can be accepted or rejected. EVM destination
	  route-canary `used_message_proof` and JSON `expected_matches` controls must
	  likewise reject non-boolean aliases before evidence hashes or copied
	  summaries are derived. Solana destination route-canary
	  `program_immutable` and JSON `expected_matches` controls must keep the same
	  exact boolean boundary before evidence hashes or copied summaries are
	  derived. TON destination JSON `expected_matches` controls must follow the
	  same exact boolean boundary before copied summaries are derived. EVM
	  receipt-proof compact trie path `leaf` controls must reject non-boolean
	  aliases before receipt proof bytes are encoded. Solana live
	  upgradeable-loader account `executable` controls must likewise reject
	  non-boolean aliases before Program and ProgramData account shapes are
	  accepted. TRON
	  source-bridge fixed-hex/fixed-byte, runtime-bytecode whitespace, and
	  exact-u64 positive-policy controls, plus TRON live exact-hex/exact-hex32
	  and optional-hex `nonzero`, block-header `txTrieRoot` nonzero,
	  route-canary witness, route-allowlist destination-binding pin, and runtime
	  boolean default controls, must keep the same exact boolean
	  boundary. All-lanes aggregate decimal positive-policy, route-canary decimal
	  comment, and public destination-binding prefixed-hex controls must also
	  reject non-boolean aliases before copied readiness metadata is evaluated.
	  Release-bundle decimal positive-policy, optional integer, route-canary
	  integer, and output-directory force controls must keep the same exact
	  boolean boundary before public bundle files or diagnostics are emitted.
	  Release verifier canonical decimal, integer, u64, and copied decimal-text
	  positive-policy controls must keep the same exact boolean boundary before
	  release-bundle validation errors are derived.
	  Release-readiness decimal positive-policy, phase-evidence requirement, and
	  native-artifact unsafe-path redaction controls must keep the same exact
	  boolean boundary before public readiness JSON or Markdown is rendered.
	  EVM
	  receipt-proof, EVM live/source-live RPC and copied scalar hex, and TRON
	  source-bridge fixed/runtime/address hex parser
	  helper exits, plus TRON live
	  metadata bytecode, contract-address, transaction-address, and
	  trigger-request address parser boundaries, must also classify `TypeError`
	  helper drift into fixed categories before public evidence output is
	  rendered. TRON live source-event log topic, route-canary log topic, and
	  metadata runtime-bytecode parser wrappers must also classify helper
	  `ValueError`s into the same fixed non-match or malformed-metadata
	  blockers. The all-lanes aggregate CLI must also catch helper
	  `ArgumentTypeError` and `SystemExit` exits from evidence loading or
	  validation and collapse sensitive details into the fixed all-lanes
	  validation failure category before public JSON or stderr can expose
	  operator paths. The all-lanes evidence CLI must keep that top-level
	  redaction on decoded bounded HTML-entity and URL-percent text too, so
	  encoded `secret-token`, `private_key`, and `recovery-phrase` loader
	  failures stay fixed all-lanes validation-failed errors before release
	  readiness consumes copied summaries. All-lanes and lane-specific SCCP
	  evidence CLIs must use the same decoded top-level exception boundary and
	  expanded secret-marker vocabulary across EVM receipt/source/live, ETH/BSC
	  source bridge, EVM destination, Solana source/destination/live, TON
	  source/destination/live, and TRON source/live helpers; encoded
	  `secret-token`, `private_key`, `api key`, `client secret`,
	  `recovery-phrase`, and `seed phrase` helper failures must remain fixed
	  collection/rendering categories before stderr or copied summaries can
	  preserve operator text. Safe top-level CLI details must remain printable
		  ASCII only: non-ASCII text, newline/tab splices, and DEL/control bytes
		  must use the fixed fallback category. Lane-specific helpers must also
		  apply decoded unsafe-text fallback before preserving otherwise safe details,
		  so encoded newline, RTL/non-ASCII, pipe, or angle-bracket details cannot
		  survive into stderr. The lane helper, all-lanes, release-bundle, and
		  readiness-report CLI regression suites now pin caught `SystemExit` values as
		  opaque fixed fallback categories, not `str(SystemExit(...))`, so
		  parser/helper exits cannot choose stderr detail even when the payload omits
		  obvious secret markers.
	  categories. EVM source-live, EVM destination-live, Solana live, TON live,
	  and TRON live public JSON summaries must also omit runtime RPC/API endpoint
	  text where present and unknown copied summary roots before output, because
	  provider URLs and TRON endpoint path overrides can carry tokens even when
	  the collection itself is read-only.
	  EVM destination-live, source-live, and receipt-proof `--rpc-url` inputs
	  must also be exact URL text and public-DNS HTTPS unless loopback HTTP is
	  used for local development, so credentialed, query/fragment-bearing,
	  localhost, IP-literal, single-label, `.local`, malformed DNS-label,
	  padded, or control-bearing provider URLs cannot reach request construction.
	  TRON live `--tron-node-url` inputs must also be exact URL text and
	  public-DNS HTTPS unless loopback HTTP is used for local development, so
	  credentialed, query/fragment-bearing, localhost, IP-literal, single-label,
	  `.local`, malformed DNS-label, padded, or control-bearing node URLs cannot
	  reach request construction.
	  EVM receipt-proof collection
	  helper `TypeError`s must also use the fixed top-level CLI fallback before
	  stderr is emitted. EVM receipt-proof CLI transaction-hash parsing and
	  receipt RPC hex decoding must also convert helper `SystemExit`,
	  `RuntimeError`, `TypeError`, and `ValueError` drift into fixed
	  transaction-hash or canonical-RPC-hex blockers before stderr or public
	  evidence output is emitted, and all-lanes evidence load/validation helper
	  `TypeError`s must follow the same fallback rule. All-lanes copied hex
	  helper `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError`
	  failures must collapse to the existing malformed-value blockers before
	  public summaries are rendered. EVM live RPC hex decoding and copied
	  route-canary exact-hex reparsing must also convert helper `SystemExit`,
	  `RuntimeError`, `TypeError`, and `ValueError` failures into fixed
	  canonical-hex blockers before public output is rendered. EVM source-live deployment receipt
	  transaction-hash, contract-address, and block-hash reparsing must also
	  convert `ValueError` helper drift into the same category-only receipt
	  blockers as `TypeError` and `RuntimeError`. EVM source-live receipt
	  readiness must also fail closed on imported source/receipt address helper
	  `SystemExit` and `RuntimeError` without exposing helper detail or doing an
	  unguarded second receipt block-hash parse. Solana live summary validation
	  must likewise convert imported parser-helper `SystemExit`, `RuntimeError`,
	  and `ValueError` drift for verifier program ids, ProgramData addresses,
	  verifier code hashes, executable base64, and ProgramData account decoding
	  into fixed metadata blockers.
	  EVM live generated full-TOML argument parser failures must likewise collapse
	  helper `ArgumentTypeError`, `SystemExit`, `RuntimeError`, `TypeError`, and
	  `ValueError` details into the fixed generated-TOML diagnostic before
	  release artifacts can render operator-facing output.
	  EVM live default domain lookups must also convert helper `SystemExit` and
	  `RuntimeError` drift into the fixed canonical chain-id or network-id
	  argparse blockers before CLI defaults or public summaries are derived.
	  Direct EVM destination evidence
	  helper `TypeError`s must also use the fixed top-level CLI fallback
	  before stderr is emitted, and ETH/BSC source bridge evidence helper
	  `TypeError`s must follow that same fixed top-level CLI fallback before
	  stderr is emitted. Direct Solana destination/source-state, direct TON
	  destination/source-state, TRON source bridge, EVM live/source-live,
	  Solana/TON live, and TRON live collection evidence helper `TypeError`s
	  must also use the fixed top-level CLI fallback before stderr is emitted.
	  Release bundle and readiness-report helper `TypeError`s must follow the
	  same fixed top-level CLI fallback before release stderr is emitted.
	  EVM live/source-live, EVM receipt-proof, Solana live, all-lanes
	  validation, release bundle, and readiness-report CLI regressions must also
	  cover secret-bearing `ValueError`s at the same public boundary.
  Release-readiness and strict bundle verifier hex predicate helper
  `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError` failures must
  also fail closed as canonical hex blockers before active launch metadata or
  public release evidence is rendered. Bundle-builder and strict verifier
  Solana route-canary pubkey parser `SystemExit`, `RuntimeError`, `TypeError`,
  and `ValueError` failures must likewise fail closed as canonical base58-address
  blockers before public release evidence is rendered.
  EVM route-canary log address/topic parser `SystemExit`, `RuntimeError`,
  `TypeError`, and `ValueError` failures must be treated as non-matching logs
  before copied event evidence can escape parser details. TRON source-event
  log address/topic/data and route-canary log address/topic parser
  `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError` failures must
  likewise be treated as non-matching logs or fixed public blockers, never as
  leaked parser details.


<a id="record-5bc37b2696934c1a6a106cd1057c519fe26a34ff4e49b5c00ff970c52f4af0f8"></a>

- SCCP duplicate-JSON diagnostics must stay fixed and traceback-safe before
  public operator output is emitted: EVM receipt, EVM live/source-live, Solana
  live, and TON live helpers must report method or endpoint categories while
  suppressing duplicate-key exception chains. TRON live duplicate-key API
  regressions now also pin explicit Python context suppression beside the fixed
  endpoint-category diagnostic, so duplicate-key parser context cannot leak if a
  traceback is rendered by a caller.
  Ethereum receipt and TON live source inventories now also pin the no-raw-key,
  no-cause, and suppressed-context assertions beside their fixed duplicate-key
  diagnostics, so sparse inventory checks fail if those traceback-safety
  assertions are removed.
  The all-lanes fallback TOML parser now raises sanitized structural errors with
  suppressed Python context as well, with duplicate-key and unsupported-section
  regressions pinned in the source-material role-validation inventory.


<a id="record-11724e64369c1b88696843ca0347dda00f7e135f85164b462bc04690707c9b10"></a>

- SCCP release-bundle public JSON roots must be regular, non-symlinked bundle
  files before parsing or canonical-serialization checks run. The strict
  verifier now routes readiness-report and all-lanes-summary roots through a
  shared public JSON loader, keeps the underlying duplicate-key JSON helpers
  plus the release-bundle builder's duplicate-key JSON loader behind the same
  non-symlinked regular-file preflight, including symlinked parent directories,
  and sparse source inventory plus adversarial symlink/directory regressions
  must keep those roots from being followed or read as directories before fixed
  `cannot load ... JSON` blockers are emitted.


<a id="record-141b045b794e9509a88c6ca82568438d27f2d03f244ebdcd70dc0d8ebdef76e8"></a>

- SCCP release-bundle public Markdown roots must also be regular,
  non-symlinked bundle files before UTF-8 decoding or render-invariant checks
  run. The strict verifier now routes readiness Markdown and release-notes
  attachment roots through a shared public text loader, and the public Markdown
  source inventory plus symlink/directory regressions must keep those roots
  from being followed or read as directories before fixed Markdown load
  blockers are emitted. The shared public text loader must also keep direct
  ancestor-symlink coverage pinned for any future nested public text roots.


<a id="record-275bc72ea70db580b55bf35764f36fff9f67fb51aad1cdaabefc37d99dd57b4b"></a>

- SCCP imported live metadata reparsing must stay category-only before TOML or
  release summaries are produced: EVM live/source-live hex fields, Solana live
  verifier program id, ProgramData address, verifier code hash, executable
  ProgramData, embedded Program account ProgramData address, and TON live
  address/transaction-LT plus code-BoC fields must suppress lower-level parser
  exception chains. EVM source-live deployment receipt `transactionHash`,
  `contractAddress`, and `blockHash` reparsing must also convert helper
  `SystemExit`, `TypeError`, `RuntimeError`, and `ValueError` failures into the
  fixed receipt-field blockers before public output is rendered. TON live
  accountStates address collection, live hash/base64 decoders, live code-BoC
  collection, copied verifier and account address reparsing, copied
  `last_transaction_lt`, and copied code-BoC base64 reparsing must convert
  helper `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError` failures
  into the same fixed public metadata blockers as malformed parser values.
  EVM source-live deployment transaction readback, receipt block-header
  readback, finalized-block hash, and generated receipt-block hash metadata
  reparsing must also convert helper `SystemExit`, `RuntimeError`, `TypeError`,
  and `ValueError` failures into fixed deployment-readback blockers before
  public output is rendered.
  EVM destination, EVM live, EVM source-live, and TON destination copied-metadata
  parser-detail regressions now also pin explicit context suppression beside the
  category-only messages in release inventory.
  TRON live metadata bytecode, constant-call ABI word,
  source-event result-byte, contract-address, transaction-address, trigger
  request, and destination full-TOML
  runtime-bytecode parser helpers must keep the same
  `SystemExit`/`RuntimeError` category-only redaction instead of echoing helper
  detail into public summaries or TOML blockers. TRON generated full-TOML
  argument parser failures must also collapse helper `ArgumentTypeError`,
  `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError` details into the
  fixed generated-TOML diagnostic before operator-facing TOML output is
  rendered.
	  TRON live source-bridge and destination-verifier collectors must also require
	  an exact boolean `include_contract_metadata` gate before deciding whether
	  `/wallet/getcontract` metadata is mandatory or skipped. The direct collector
	  `no_getcontract`, `solid`, and `full_toml` flags must likewise reject
	  non-boolean values before endpoint selection or metadata-skipping decisions.
	  TRON witness-schedule mismatch handling must also require exact boolean
	  `allow_expected_mismatch` controls before a transition proof can relax the
	  active expected-hash match.
	  Solana live RPC account-data and copied-metadata base64 decoding must likewise
	  convert decoder `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError`
	  helper failures into the fixed invalid-base64 categories before public
  evidence output is rendered.


<a id="record-686b5ebd893eda0adffdbc97fabb083feecc2f13fb11365421cf4a596422b821"></a>

- SCCP direct Solana/TON destination verifier identity reparsing must stay
  category-only before TOML or JSON summary rendering, with parser exception
  chains suppressed and adversarial `secret-token` regressions pinned. Release
  public scalar-text source inventory must pin the direct destination
  parser-detail payloads themselves for both Solana and TON. Direct Solana
  verifier program base64 decoding, direct Solana/TON source/destination hex
  parsing, and direct TON code-BoC base64/url decoding must also convert
  decoder/parser `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError`
  helper failures into the fixed parser categories before output rendering.
  The release verifier now pins the widened direct Solana/TON helper-exit
  regression names and exception loops in both source-material role-validation
  and public scalar-text inventories.
  Direct TON destination `account_status`, `last_transaction_lt`, TOML LT, and
  copied code-BoC base64 metadata reparsing must apply the same helper-exit
  redaction before JSON, TOML, or release summaries are rendered.


<a id="record-9b939a96c93fe4df9e3ee41b8738a0f55b6ad220d5744f26f1a8de370ab989f2"></a>

- SCCP all-lanes Solana live ProgramData and route-canary base64 comment
  diagnostics must stay category-only before aggregate blockers are emitted; raw
  canonical-base64 parser details must not be copied into release readiness
  output. Release public scalar-text source inventory must pin the Solana
  all-lanes adversarial base64 and ProgramData parser payloads themselves, not
  only generic absence assertions. Aggregate all-lanes EVM, TRON, TON, and
  Solana copied live-metadata parser boundaries must also convert `TypeError`
  helper drift into the same fixed blockers before public readiness output is
  rendered, including the Solana live ProgramData and route-canary base64
  caller wrappers that feed aggregate blockers.


<a id="record-1bc10864b615d183ebe346a22b8c64ccee0714e5f7ac743a2b5018a921722861"></a>

- SCCP TRON solid-block header proof canonicalization failures must stay
  category-only before live-evidence summaries or full-TOML blockers are
  emitted; lower-level proof encoder exception text, including `SystemExit`
  helper exits and `TypeError` parser/canonicalizer details, must not be copied
  into public readiness output. The same category-only rule covers TRON
  witness-schedule hash/transition, witness-schedule payload decoder,
  transition active/parent/next schedule payload and hash helpers, witness-seal,
  witness-seal schedule-hash, signer-bitmap shape, message/seal hash-shape, and
  transaction-source-proof hash-shape/helper failures, including
  duplicate-address and total-weight-overflow malformed schedule payloads. The
  TRON live malformed-schedule regressions now explicitly pin that duplicate
  witness and total-weight overflow details, including witness indexes and raw
  overflow weights, cannot appear in public summaries, and the release
  public-scalar inventory pins those regressions.


<a id="record-1f4f9a87e5f27deb036922cb139e6f6eb3a4de9d82ad0d1b70e350a336596568"></a>

- The current SCCP release-evidence matrix enumerates exactly Ethereum, BSC,
  TRON, and TON mainnet. Solana and TON testnet clauses retained in the
  pre-release implementation history below are non-normative and cannot
  satisfy SCCP V1 production readiness.

