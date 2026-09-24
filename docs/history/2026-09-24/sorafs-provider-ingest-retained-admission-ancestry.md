# SoraFS provider-ingest retained admission ancestry

2026-09-24, `optimizations`. This bounded F05/G06 cut makes the resolver-facing
current-assignment read resolve the immutable job admission cursor against the
same retained provider-ingest archive chain as the visible committed head. The
reader first qualifies that archive against the exact Kura hash journal, V2
finality artifacts and result-bearing blocks, then fences the read with a second
visible-head and archive-generation check. The new check rejects an earlier
height with a substituted hash, a cursor outside retained coverage, a future
admission, a substituted current head, or a foreign network. It is used by the
current source-assignment and native StreamToken custody reads, as well as the
before/after assignment observation around already-supplied HTTPS evidence.

The check does not make an authorization DTO an admission certificate. It does
not authenticate council membership, signed envelope renewal/revocation,
adverts, governed HTTPS origins, StreamToken use statements, or DER TLS roots.
Those still lack a consensus-owned finalized producer, and the
`ProviderIngestGovernedHttpsGrantResolverV1` remains injected with no standard
production implementation. Existing Torii admission-directory and advert-cache
state must not be promoted to finalized authority. When archive retention has
pruned the old job cursor, this read fails closed; a future producer may need an
independently verified compact ancestry witness before that job can be sourced.
Grant issuance, source deployment, F05/G06, and release promotion remain open.

The focused test exercises a retained ancestor, earlier-height fork, below-floor
cursor, future anchor, substituted current head, and foreign network through
the archive's real exact-key index. It does not construct a new Kura/QC fixture;
the existing Apply/archive qualification tests cover that separate production
fence. No HSM or compatibility path is introduced.

On the combined checkout,
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib
current_assignment_binding_tracks_new_revision_and_rejects_stale_source -- --nocapture`
passed 1/1. The compiled daemon library's `current_assignment_` selector passed
3/3, including substituted Musubi and unavailable-head refusal. The binary
target has no tests for this module; the library results are the evidence here.

The subsequent combined daemon build corrected the signed-capture test fixture
to initialize its canonical State/lane journal before storing genesis in Kura.
The focused `signed_capture_reader_caches_exact_generation_across_archive_advance`
selector passed 1/1, and the full `sorafs_provider_ingest_finalized_query::tests`
module passed 12/12 on the same compiled daemon library. That fixture ordering
is test-only; production archive and grant authority did not change.
