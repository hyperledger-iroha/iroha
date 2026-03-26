# Walletless Follow Game Demo

A static bundle that demonstrates the walletless Twitter follow flow: generate a fresh session key
in the browser, capture the keyed Twitter hash, and emit a ready-to-run Torii submit command for a
sponsor account to pay/submit. No browser extensions or middleware are required.

## What this ships

- `index.html` / `app.js` / `styles.css` — local WebCrypto keygen, binding/amount form, and
  prebuilt Node command that calls the JS SDK recipe.
- `javascript/iroha_js/recipes/walletlessFollowGame.mjs` — Node helper that builds
  `SendToTwitter` + `ClaimTwitterFollowReward` instructions and submits them directly to Torii.
- `scripts/social/package_walletless_bundle.sh` — deterministic packer that tars the bundle for
  `PublishContentBundle` uploads.
- `dashboards/grafana/social_follow_game.json` — follow-game burn/abuse dashboard using the new
  `iroha_social_*` counters (`social_rejections_total`, `social_open_escrows`,
  `social_events_total`, promo/halt gauges).

## Quickstart

1. Open `index.html` in a local HTTP server (or publish via the content lane). Click **Regenerate
   session key** to rotate the ephemeral signer.
2. Paste the keyed Twitter binding hash JSON (or digest hex), fill in Torii URL, chain ID, sponsor
   account, and optional session account.
3. Click **Build Torii command**. The UI renders:
   - A Node command that calls the JS recipe and submits directly to Torii (sponsor pays fees).
   - A JSON payload mirror if you want to wire the transaction manually.
4. Run the Node command from repository root. Override any flag with environment variables:

   ```bash
  TORII_URL=http://127.0.0.1:8080 \
  CHAIN_ID=dev-chain \
  SPONSOR_ID=sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7 \
  SESSION_KEY_HEX=<private key hex> \
  node javascript/iroha_js/recipes/walletlessFollowGame.mjs \
     --binding '{"KeyedHash":{"pepper_id":"twitter_follow","digest_hex":"abcd...","uaid_hex":"...","tweet_id":"12345"}}' \
     --amount 10
   ```

## Session-key rotation

- Rotate the session key per promo wave. The browser always regenerates keys locally; keep the
  private key only long enough to complete a send+claim, then discard it.
- Use the `session_account` metadata field (or set `--session-account` in the Node script) to help
  operators spot stale sessions in telemetry.

## CI packaging (example)

Use the packer to ship the static bundle into the repo’s content lane workflow:

```yaml
name: walletless-follow-bundle
on: [push]
jobs:
  pack:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - name: Package walletless bundle
        run: scripts/social/package_walletless_bundle.sh
      - name: Publish via iroha_cli
        run: |
          iroha_cli content publish \
            --bundle artifacts/walletless_follow_bundle.tar.gz \
            --dataspace web \
            --lane gov \
            --account "${{ secrets.SORANET_PUBLISHER }}" \
            --key "${{ secrets.SORANET_PUBLISHER_KEY }}"
```

The packer writes `artifacts/walletless_follow_bundle.tar.gz` plus a `.sha256` sidecar so the same
artefact can be pinned via `PublishContentBundle`. Update the GitHub Action to reference the target
dataspace/lane used by your deployment.

## CI/workflow hook

- `ci/check_walletless_follow_bundle.sh` repackages the bundle and asserts the tar + `.sha256`
  exist; wire it into your pipeline to guard regressions.
- `.github/workflows/walletless-follow-bundle.yml` runs the packer and uploads the artefacts
  on demand; extend it with your publisher secrets to auto-publish through `iroha_cli content publish`.
