---
lang: pt
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de slides de treinamento SNS

Este outline Markdown espelha os slides que os facilitadores devem adaptar para suas turmas por idioma. Copie estas secoes para Keynote/PowerPoint/Google Slides e localize os bullets, capturas e diagramas conforme necessario.

## Slide de titulo
- Programa: "Sora Name Service onboarding"
- Subtitulo: especifique sufixo + ciclo (ex., `.sora - 2026-03`)
- Apresentadores + afiliacoes

## Orientacao de KPI
- Screenshot ou embed de `docs/portal/docs/sns/kpi-dashboard.md`
- Lista de bullets explicando filtros de sufixo, tabela ARPU, rastreador de freeze
- Callouts para exportar PDF/CSV

## Ciclo de vida do manifest
- Diagrama: registrar -> Torii -> governance -> DNS/gateway
- Passos referenciando `docs/source/sns/registry_schema.md`
- Exemplo de trecho de manifest com anotacoes

## Exercicios de disputa e freeze
- Diagrama de fluxo para intervencao do guardian
- Checklist referenciando `docs/source/sns/governance_playbook.md`
- Linha do tempo de ticket de freeze de exemplo

## Captura de anexos
- Trecho de comando mostrando `cargo xtask sns-annex ... --portal-entry ...`
- Lembrete para arquivar o JSON do Grafana em `artifacts/sns/regulatory/<suffix>/<cycle>/`
- Link para `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Proximos passos
- Link de feedback de treinamento (veja `docs/examples/sns_training_eval_template.md`)
- Handles de canais Slack/Matrix
- Datas dos proximos marcos
