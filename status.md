# Status

## Latest Updates
- Restored `iroha_torii::test_utils` helpers for Torii integration tests, including queue-apply utilities and JSON request builders.
- Moved `run_test_dump.py` into `scripts/` to keep the repo root tidy.
- Translated the SoraFS orchestrator configuration guide across portal docs and portal i18n locales (ar/es/fr/he/ja/pt/ru/ur).
- Translated the SoraFS multi-source rollout runbook across portal docs and portal i18n locales (ar/es/fr/he/ja/pt/ru/ur).
- Re-ran the seven-peer DA consistency test and stabilized the integration check by waiting for RBC delivery at or above the mint height and polling asset convergence (added a helper test for the RBC session filter).
- Translated the SoraFS orchestrator tuning guide across portal docs and portal i18n locales (ar/es/fr/he/ja/pt/ru/ur).
- Translated the SoraFS orchestrator ops runbook across portal docs and portal i18n locales (ar/es/fr/he/ja/pt/ru/ur).
- Normalized trybuild-generated `Cargo.toml` paths for `norito_derive` UI tests to use relative paths so target-codex artifacts avoid developer-specific absolute paths.
