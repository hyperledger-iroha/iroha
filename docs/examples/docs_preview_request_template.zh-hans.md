---
lang: zh-hans
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-12-29T18:16:35.072823+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 文档门户预览访问请求（模板）

在授予访问权限之前捕获审阅者详细信息时，请使用此模板
公共预览环境。将 markdown 复制到问题或请求表中，然后
替换占位符值。

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

## 社区特定问题 (W2+)
- 预览访问的动机（一句话）：
- 主要审查重点（SDK、治理、Norito、SoraFS、其他）：
- 每周时间承诺和可用窗口（UTC）：
- 本地化或可访问性需求（是/否+详细信息）：
- 社区行为准则 + 预览可接受使用附录已确认（是/否）：