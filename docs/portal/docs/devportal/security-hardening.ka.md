---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 703ab06de69620a6930c5aae80a1fa310010a938b4db2b366003ea0ddd62ae8a
source_last_modified: "2025-12-29T18:16:35.115254+00:00"
translation_last_reviewed: 2026-02-07
id: security-hardening
title: Security hardening & pen-test checklist
sidebar_label: Security hardening
description: Harden the developer portal before exposing the Try it sandbox outside the lab.
---

## Overview

Roadmap item **DOCS-1b** requires OAuth device-code login, strong content
security policies, and repeatable penetration tests before the preview portal
can run on non-lab networks. This appendix explains the threat model, the
controls implemented in the repo, and the go-live checklist that gate reviews
must execute.

- **Scope:** the Try it proxy, embedded Swagger/RapiDoc panels, and the custom
  Try it console rendered by `docs/portal/src/components/TryItConsole.jsx`.
- **Out-of-scope:** Torii itself (covered by Torii readiness reviews) and SoraFS
  publishing (covered by DOCS-3/7).

## Threat model

| Asset | Risk | Mitigation |
| --- | --- | --- |
| Torii bearer tokens | Theft or reuse outside the docs sandbox | Device-code login (`DOCS_OAUTH_*`) mints short-lived tokens, the proxy redacts headers, and the console auto-expires cached credentials. |
| Try it proxy | Abuse as an open relay or bypass of Torii rate limits | `scripts/tryit-proxy*.mjs` enforce origin allowlists, rate limiting, health probes, and explicit `X-TryIt-Auth` forwarding; no credentials are persisted. |
| Portal runtime | Cross-site scripting or malicious embeds | `docusaurus.config.js` injects Content-Security-Policy, Trusted Types, and Permissions-Policy headers; inline scripts are restricted to the Docusaurus runtime. |
| Observability data | Missing telemetry or tampering | `docs/portal/docs/devportal/observability.md` documents the probes/dashboards; `scripts/portal-probe.mjs` runs in CI before publishing. |

Adversaries include curious users viewing the public preview, malicious actors
testing stolen links, and compromised browsers attempting to scrape stored
credentials. All controls must work on commodity browsers without trusted
networks.

## Required controls

1. **OAuth device-code login**
   - Configure `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID`, and related knobs in the build environment.
   - The Try it card renders a sign-in widget (`OAuthDeviceLogin.jsx`) that
     fetches the device code, polls the token endpoint, and auto-clears tokens
     once they expire. Manual Bearer overrides remain available for emergency
     fallback.
   - Builds now fail when the OAuth configuration is missing or when the
     fallback TTLs drift outside the 300 s–900 s window mandated by DOCS-1b;
     set `DOCS_OAUTH_ALLOW_INSECURE=1` only for disposable local previews.
2. **Proxy guardrails**
   - `scripts/tryit-proxy.mjs` enforces allowed origins, rate limits, request
     size caps, and upstream timeouts while tagging traffic with
     `X-TryIt-Client` and redacting tokens from logs.
   - `scripts/tryit-proxy-probe.mjs` plus `docs/portal/docs/devportal/observability.md`
     define the liveness probe and dashboard rules; run them before every
     rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` now exports deterministic security headers:
     `Content-Security-Policy` (default-src self, strict connect/img/script
     lists, Trusted Types requirements), `Permissions-Policy`, and
     `Referrer-Policy: no-referrer`.
   - The CSP connect list whitelists the OAuth device-code and token endpoints
     (HTTPS only unless `DOCS_SECURITY_ALLOW_INSECURE=1`) so device login works
     without relaxing the sandbox for other origins.
   - The headers are embedded directly in the generated HTML so static hosts do
     not need extra configuration. Keep inline scripts limited to the
     Docusaurus bootstrap.
4. **Runbooks, observability, and rollback**
   - `docs/portal/docs/devportal/observability.md` describes the probes and
     dashboards that watch login failures, proxy response codes, and request
     budgets.
   - `docs/portal/docs/devportal/incident-runbooks.md` covers the escalation
     path if the sandbox is abused; combine it with
     `scripts/tryit-proxy-rollback.mjs` to flip endpoints safely.

## Pen-test & release checklist

Complete this list for every preview promotion (attach results to the release
ticket):

1. **Verify OAuth wiring**
   - Run `npm run start` locally with the production `DOCS_OAUTH_*` exports.
   - From a clean browser profile, open the Try it console and confirm the
     device-code flow mints a token, counts down the lifetime, and clears the
     field after expiry or sign-out.
2. **Probe the proxy**
   - `npm run tryit-proxy` against staging Torii, then execute
     `npm run probe:tryit-proxy` with the configured sample path.
   - Check logs for `authSource=override` entries and confirm rate limiting
     increments counters when you exceed the window.
3. **Confirm CSP/Trusted Types**
   - `npm run build` and open `build/index.html`. Ensure the `<meta
     http-equiv="Content-Security-Policy">` tag matches the expected directives
     and that DevTools shows no CSP violations when loading the preview.
   - Use `npm run probe:portal` (or curl) to fetch the deployed HTML; the probe
     now fails when the `Content-Security-Policy`, `Permissions-Policy`, or
     `Referrer-Policy` meta tags are missing or differ from the values declared
     in `docusaurus.config.js`, so governance reviewers can rely on the exit
     code instead of eyeballing curl output.
4. **Review observability**
   - Verify the Try it proxy dashboard is green (rate limits, error ratios,
     health probe metrics).
   - Run the incident drill in `docs/portal/docs/devportal/incident-runbooks.md`
     if the host changed (new Netlify/SoraFS deployment).
5. **Document the results**
   - Attach screenshots/logs to the release ticket.
   - Capture each finding in the remediation report template
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     so owners, SLAs, and retest evidence are easy to audit later.
   - Link back to this checklist so the DOCS-1b roadmap item stays auditable.

If any step fails, halt the promotion, file a blocking issue, and note the
remediation plan in `status.md`.
