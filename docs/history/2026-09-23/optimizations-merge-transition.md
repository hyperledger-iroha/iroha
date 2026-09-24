# Optimizations merge transition — 2026-09-23

The source was under an unresolved external merge while focused first-release
checks ran in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`. At
17:58:04 JST, another actor completed that merge as
`b45ec0457eb0664e46c2cdfd8a4b619caefd02b9`, with parents
`fb4a763853bf1d63cd44d1846210dbe354be3e2e` and
`a689ffb4a98d61244d7e4500323919e5ae4d5aa2`. The branch remained
`optimizations`, `MERGE_HEAD` disappeared, and `git ls-files -u` returned no
unmerged entries. This agent did not stage or create that merge commit.

Git reports signature status `N` for the merge commit. It is not the signed,
immutable first-release source candidate. Focused Core tests that compiled
before the merge remain component evidence for the source they read; the final
combined tree still requires a fresh build, tests, artifact regeneration and
candidate-specific qualification. The role-13 SoraFS DTO and test edits that
remained unstaged after the merge are being validated as one combined source.
