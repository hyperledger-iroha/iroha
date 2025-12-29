Lab PoP descriptor + ROA bundle for SoraNet SNNet-15A validation drills.

Commands:
- Render FRR from the descriptor:
  - `cargo xtask soranet-pop-template --input fixtures/soranet_pop/lab_pop.json --output artifacts/soranet_pop/lab_frr.conf`
- Descriptors support optional `communities` (`drain`/`blackhole`); defaults are
  `no-export`/`no-advertise` and the lab fixture sets explicit values so the
  rendered FRR includes deterministic community-lists plus validation JSON that
  reports whether defaults were used.
- Build the full automation bundle (env/secrets/checklist/CI/sign-off) with an
  optional image tag:
  - `cargo xtask soranet-pop-bundle --input fixtures/soranet_pop/lab_pop.json --roa fixtures/soranet_pop/lab_roas.json --output-dir artifacts/soranet_pop/lab_bundle --skip-edns --skip-ds --image-tag <tag>`
  - `cargo xtask soranet-popctl ...` is an alias for the same bundle but matches
    the SN15-M0-2 docs/playbooks.
  - Outputs: `pop.env`, `secrets/secret_placeholders.toml`, `checklist.md`,
    `ci/pop_bringup.yml`, `attestations/signoff.json`, plus the FRR/resolver
    configs and bundle manifest.
- Validate descriptor (BFD/RPKI/ROA coverage) and emit JSON:
  - `cargo xtask soranet-pop-validate --input fixtures/soranet_pop/lab_pop.json --roa fixtures/soranet_pop/lab_roas.json --json-out -`
- Render and validate in one shot:
  - `cargo xtask soranet-pop-plan --input fixtures/soranet_pop/lab_pop.json --roa fixtures/soranet_pop/lab_roas.json --frr-out artifacts/soranet_pop/lab_frr.conf --json-out -`
- Render resolver template + EDNS/DS evidence in one run (writes to `artifacts/soradns/` by default when paths are provided):
  - `cargo xtask soranet-pop-template --input fixtures/soranet_pop/lab_pop.json --resolver-config artifacts/soradns/lab_resolver.toml --edns-out artifacts/soradns/edns_matrix.json --ds-out artifacts/soradns/ds_validation.json`
- Build a PoP provisioning bundle (FRR + resolver + PXE/profile stubs + manifest):
  - `cargo xtask soranet-pop-bundle --input fixtures/soranet_pop/lab_pop.json --roa fixtures/soranet_pop/lab_roas.json --output-dir artifacts/soranet_pop/lab_bundle --skip-edns --skip-ds`
- Descriptors optionally set `communities.fail_open` (defaults to
  `graceful-shutdown`) and per-neighbor `link_tier` (`core`/`edge`/`ix`/`transit`)
  to auto-fill tier-specific BFD profiles in the rendered FRR config and policy
  reports; community-lists now emit `SORANET-FAIL-OPEN` alongside drain/blackhole.
- Emit SN15-M0-3 policy/monitoring harness (BGP alert rules + Grafana dashboard + JSON summary):
  - `cargo xtask soranet-pop-policy-report --input fixtures/soranet_pop/lab_pop.json --roa fixtures/soranet_pop/lab_roas.json --output-dir artifacts/soranet_pop/policy`
  - Outputs: `bgp_alert_rules.yml`, `grafana_soranet_bgp.json`, `policy_report.json` keyed to the descriptor’s neighbors/prefixes/RPKI caches. The generated `health_probe.sh` now enforces neighbor/RPKI/prefix expectations after rollout.
