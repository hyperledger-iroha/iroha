---
lang: pt
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

# Solicitacao de acesso a preview do portal de docs (Modelo)

Use este modelo ao registrar os detalhes do reviewer antes de liberar acesso ao ambiente de
preview publico. Copie o markdown em um issue ou formulario de solicitacao e substitua
os placeholders.

```markdown
## Resumo da solicitacao
- Solicitante: <nome completo / org>
- Usuario do GitHub: <username>
- Contato preferido: <email/Matrix/Signal>
- Regiao e fuso horario: <UTC offset>
- Datas propostas de inicio / fim: <YYYY-MM-DD -> YYYY-MM-DD>
- Tipo de reviewer: <Core maintainer | Partner | Community volunteer>

## Checklist de conformidade
- [ ] Assinou a politica de uso aceitavel da preview (link).
- [ ] Revisou `docs/portal/docs/devportal/security-hardening.md`.
- [ ] Revisou `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] Reconheceu coleta de telemetria e analise anonimizada (sim/nao).
- [ ] Alias do SoraFS solicitado (sim/nao). Nome do alias: `<docs-preview-???>`

## Necessidades de acesso
- URL(s) de preview: <https://docs-preview.sora.link/...>
- Escopos de API necessarios: <Torii read-only | Try it sandbox | none>
- Contexto adicional (testes de SDK, foco de revisao de docs, etc.):
  <detalhes aqui>

## Aprovacao
- Reviewer (maintainer): <nome + data>
- Ticket de governance / solicitacao de mudanca: <link>
```

---

## Perguntas especificas para a comunidade (W2+)
- Motivacao para acesso de preview (uma frase):
- Foco principal de revisao (SDK, governance, Norito, SoraFS, outro):
- Compromisso semanal de tempo e janela de disponibilidade (UTC):
- Necessidades de localizacao ou acessibilidade (sim/nao + detalhes):
- Codigo de Conduta da Comunidade + adendo de uso aceitavel de preview reconhecido (sim/nao):
