---
lang: pt
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
translator: manual
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Tradução para português de docs/examples/docs_preview_feedback_form.md (Docs preview feedback form) -->

# Formulário de feedback para o preview de docs (onda W1)

Use este template ao coletar feedback de revisores da onda W1. Duplique o arquivo por
parceiro, preencha os metadados e salve a cópia completa em
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Metadados do revisor

- **ID do parceiro:** `partner-w1-XX`
- **Ticket de solicitação:** `DOCS-SORA-Preview-REQ-PXX`
- **Convite enviado (UTC):** `YYYY-MM-DD hh:mm`
- **Checksum reconhecido (UTC):** `YYYY-MM-DD hh:mm`
- **Áreas principais de foco:** (por exemplo _docs do orquestrador SoraFS_,
  _fluxos ISO do Torii_)

## Confirmações de telemetria e artefatos

| Item de checklist | Resultado | Evidência |
| --- | --- | --- |
| Verificação de checksums | ✅ / ⚠️ | Caminho para o log (ex.: `build/checksums.sha256`) |
| Smoke test do proxy Try it | ✅ / ⚠️ | Trecho de `npm run manage:tryit-proxy …` |
| Revisão de dashboard no Grafana | ✅ / ⚠️ | Caminho(s) para captura(s) de tela |
| Revisão do relatório de probe do portal | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Adicione linhas para quaisquer outros SLOs que o revisor inspecionar.

## Log de feedback

| Área | Severidade (info/minor/major/blocker) | Descrição | Correção sugerida ou pergunta | Issue de rastreio |
| --- | --- | --- | --- | --- |
| | | | | |

Faça referência ao issue no GitHub ou ao ticket interno na última coluna para que o
tracker de preview consiga vincular os itens de remediação a este formulário.

## Resumo da pesquisa

1. **Quão confiante você está nas orientações de checksums e no processo de convite?**
   (1–5)
2. **Quais docs foram mais/menos úteis?** (resposta curta)
3. **Houve bloqueios ao acessar o proxy Try it ou os dashboards de telemetria?**
4. **É necessário conteúdo adicional de localização ou acessibilidade?**
5. **Algum outro comentário antes do GA?**

Registre respostas curtas e anexe exports brutos do questionário se estiver usando um
formulário externo.

## Checagem de conhecimento

- Pontuação: `__/10`
- Questões incorretas (se houver): `[#1, #4, …]`
- Ações de follow‑up (se a nota for < 9/10): call de remediação agendada? s/n

## Assinatura

- Nome do revisor e timestamp:
- Nome do revisor de Docs/DevRel e timestamp:

Armazene a cópia assinada junto com os artefatos associados para que auditores possam
reconstituir a onda sem contexto adicional.
