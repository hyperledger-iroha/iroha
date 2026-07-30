# Docs Portal Preview Access Request (Template)

Use this template when capturing reviewer details before granting access to the
public preview environment. Copy the markdown into an issue or request form and
replace the placeholder values.

```markdown
## Request Summary
- Requester: <full name / org>
- GitHub handle: <username>
- Preferred contact: <email/Matrix/Signal>
- Region & timezone: <UTC offset>
- Proposed start / end dates: <YYYY-MM-DD → YYYY-MM-DD>
- Reviewer type: <Core maintainer | Partner | Community volunteer>

## Compliance Checklist
- [ ] Signed the preview acceptable-use policy (link).
- [ ] Reviewed `docs/portal/docs/devportal/security-hardening.md`.
- [ ] Reviewed `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] Acknowledged telemetry collection & anonymised analytics (yes/no).
- [ ] SoraFS alias requested (yes/no). Alias name: `<docs-preview-???>`

## Access Needs
- Preview URL(s): <https://docs-preview.sora.link/...>
- Required API scopes: <Torii read-only | Try it sandbox | none>
- Additional context (SDK tests, documentation review focus, etc.):
  <details here>

## Approval
- Reviewer (maintainer): <name + date>
- Governance ticket / change request: <link>
```

---

## Community-Specific Questions (W2+)
- Motivation for preview access (one sentence):
- Primary review focus (SDK, governance, Norito, SoraFS, other):
- Weekly time commitment & availability window (UTC):
- Localization or accessibility needs (yes/no + details):
- Community Code of Conduct + preview acceptable-use addendum acknowledged (yes/no):
