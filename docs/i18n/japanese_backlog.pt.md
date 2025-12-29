---
lang: pt
direction: ltr
source: docs/i18n/japanese_backlog.md
status: complete
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: 569668c78c5fe322f602bfeb31944cde7c1f10c4e78cfae66b31cdfcca845885
source_last_modified: "2025-11-06T15:33:45.173464+00:00"
translation_last_reviewed: 2025-11-14
---

# Backlog de tradução de documentação para japonês

Este arquivo acompanha a documentação em japonês que ainda está marcada com
`status: needs-translation`, agrupada por categoria para ajudar no planejamento dos
próximos lotes. Arquivos que combinam com `CHANGELOG.*`, `status.*` e `roadmap.*` são
tratados como temporários e excluídos do backlog. Execute
`python3 scripts/sync_docs_i18n.py --dry-run` para atualizar a lista antes de iniciar um
novo lote.

## Visão geral

- Arquivos pendentes: 0 (atualizado em 2025-10-26)
- Exclusões de tradução: continuar ignorando `CHANGELOG.*`, `status.*` e `roadmap.*`.
- Não há backlog aberto no momento. Quando novos documentos em inglês forem adicionados,
  execute `scripts/sync_docs_i18n.py --dry-run` para detectar o delta e enfileirar o
  próximo lote.

### Traduções recentes para japonês

A lista abaixo enumera os arquivos em japonês que já foram traduzidos, para apoiar o
planejamento de lotes futuros e evitar retrabalho.

## Sugestão de lotes

> Como não há itens pendentes, mantemos o plano de lotes como material de referência
> para a próxima leva de documentos.

### Lote A – Documentação operacional de ZK / Torii
- Objetivo: manter atualizados, em japonês, os runbooks de anexação e verificação ZK.
- Escopo: quaisquer `docs/source/zk/*.ja.md` restantes (tanto `lifecycle` quanto
  `prover_runbook` já estão completos).
- Entregável: cobertura operacional completa de ZK mais um glossário anexado aos
  runbooks.

### Lote B – Design central de IVM / Kotodama / Norito
- Objetivo: melhorar a descobribilidade para desenvolvedores traduzindo a documentação
  central de VM/códec.
- Escopo: `docs/source/ivm_*.ja.md`, `docs/source/kotodama_*.ja.md`,
  `docs/source/norito_*.ja.md`.
- Entregável: resumos concisos e índices de palavras‑chave para cada documento.

### Lote C – Sumeragi / operações de rede
- Objetivo: fornecer runbooks operacionais para consenso, pipeline e camada de rede.
- Escopo: `docs/source/sumeragi*.ja.md`, `docs/source/governance_api.ja.md`,
  `docs/source/pipeline.ja.md`, `docs/source/p2p.ja.md`,
  `docs/source/state_tiering.ja.md`.
- Entregável: a primeira edição em japonês do manual de operação.

### Lote D – Ferramentas / exemplos / referências
- Objetivo: fornecer a desenvolvedores material de referência e exemplos localizados.
- Escopo: quaisquer `docs/source/query_*.ja.md` pendentes (e referências novas).
- Entregável: garantir que as referências principais de CLI e API permaneçam alinhadas à
  fonte em inglês.

## Lembretes de fluxo de trabalho de tradução

1. Escolha os arquivos alvo e trabalhe em um branch dedicado.
2. Após finalizar a tradução, defina `status: complete` no front matter e preencha o campo
   `translator` se for o caso.
3. Execute `python3 scripts/sync_docs_i18n.py --dry-run` para confirmar que não restam
   stubs.
4. Alinhe a terminologia com a documentação japonesa existente, por exemplo,
   `README.ja.md`.

## Notas

- Cuide da formatação Markdown (indentação, tabelas, blocos cercados) em vez de rodar
  `cargo fmt --all`.
- Quando o documento fonte for alterado, atualize a edição japonesa no mesmo PR sempre que
  possível.
- Siga as convenções de terminologia adotadas na documentação publicada; agende revisões
  quando o texto exigir consenso.
- Execute periodicamente `python3 scripts/sync_docs_i18n.py --dry-run` e atualize este
  backlog se surgirem novos gaps.
