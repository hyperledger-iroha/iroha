---
lang: pt
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

# Notas de descoberta de SLA para parceiro Android - Modelo

Use este modelo para cada sessao de descoberta de SLA AND8. Guarde a copia preenchida em
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` e anexe artefatos de
suporte (respostas do questionario, confirmacoes, anexos) no mesmo diretorio.

```
Partner: <Nome>                      Data: <YYYY-MM-DD>  Hora: <UTC>
Contatos principais: <nomes, cargos, email>
Participantes Android: <Program Lead / Partner Eng / Support Eng / Compliance>
Link de reuniao / ticket: <URL ou ID>
```

## 1. Agenda e contexto

- Objetivo da sessao (escopo piloto, janela de release, expectativas de telemetria).
- Docs de referencia compartilhados antes da chamada (support playbook, calendario de release,
  dashboards de telemetria).

## 2. Visao geral de carga

| Topico | Notas |
|--------|-------|
| Cargas alvo / chains | |
| Volume esperado de transacoes | |
| Janelas criticas de negocio / periodos de blackout | |
| Regimes regulatorios (GDPR, MAS, FISC, etc.) | |
| Idiomas requeridos / localizacao | |

## 3. Discussao de SLA

| Classe SLA | Expectativa do parceiro | Delta vs baseline? | Acao requerida |
|------------|--------------------------|--------------------|----------------|
| Correcao critica (48 h) | | Sim/Nao | |
| Alta severidade (5 business days) | | Sim/Nao | |
| Manutencao (30 days) | | Sim/Nao | |
| Aviso de cutover (60 days) | | Sim/Nao | |
| Cadencia de comunicacao de incidentes | | Sim/Nao | |

Documente qualquer clausula adicional de SLA solicitada pelo parceiro (ex. ponte telefonica

dedicada, exports extras de telemetria).

## 4. Requisitos de telemetria e acesso

- Necessidades de acesso Grafana / Prometheus:
- Requisitos de exportacao de logs/traces:
- Expectativas de evidencia offline ou dossier:

## 5. Notas de compliance e legais

- Requisitos de notificacao jurisdicional (estatuto + timing).
- Contatos legais requeridos para updates de incidentes.
- Restricoes de residencia de dados / requisitos de armazenamento.

## 6. Decisoes e acoes

| Item | Owner | Data | Notas |
|------|-------|------|-------|
| | | | |

## 7. Acknowledgement

- Parceiro reconheceu SLA base? (S/N)
- Metodo de confirmacao de follow-up (email / ticket / assinatura):
- Anexe email de confirmacao ou ata da reuniao a este diretorio antes de encerrar.
