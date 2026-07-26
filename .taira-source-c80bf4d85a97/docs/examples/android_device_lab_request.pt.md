---
lang: pt
direction: ltr
source: docs/examples/android_device_lab_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a8e6a4981a11faac56d9b04432773e94fd59f8e2524fa4c552be459291c7c39
source_last_modified: "2025-11-12T08:31:44.643013+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de solicitacao de reserva do laboratorio de dispositivos Android

Copie este modelo na fila do Jira `_android-device-lab` ao reservar hardware. Anexe links
para pipelines do Buildkite, artefatos de compliance e qualquer ticket de parceiro que dependa

do run.

```
Resumo: <Marco / carga> - <lane(s)> - <data/hora UTC>

Marco / Acompanhamento:
- Item de roadmap: AND6 / AND7 / AND8 (escolher)
- Ticket(s) relacionados: <link para issue ANDx>, <referencia partner-sla se houver>

Solicitante / Contato:
- Engenheiro principal:
- Engenheiro backup:
- Canal Slack / escalacao de pager:

Detalhes da reserva:
- Lanes requeridas: <pixel8pro-strongbox-a / pixel8a-ci-b / pixel7-fallback / firebase-burst / strongbox-external>
- Slot desejado: <YYYY-MM-DD HH:MM UTC> por <duracao>
- Tipo de carga: <CI smoke / attestation sweep / chaos rehearsal / partner demo>
- Tooling a executar: <nomes de jobs/scripts do Buildkite>
- Artefatos produzidos: <logs, bundles de attestation, dashboards>

Dependencias:
- Referencia de snapshot de capacidade: link para `android_strongbox_capture_status.md`
- Linhas da matriz de readiness tocadas: link para `android_strongbox_device_matrix.md`
- Vinculo de compliance (se houver): linha do checklist AND6, ID do log de evidencia

Plano de fallback:
- Se o slot primario nao estiver disponivel, o slot alternativo e:
- Precisa de pool de fallback / Firebase? (sim/nao)
- Retainer externo de StrongBox requerido? (sim/nao - incluir lead time)

Aprovacoes:
- Lider do Hardware Lab:
- TL de Android Foundations (quando lanes de CI impactadas):
- Lider de programa (se StrongBox retainer for acionado):

Checklist post-run:
- Anexar URL(s) do Buildkite:
- Atualizar linha do log de evidencia: <ID/data>
- Anotar desvios/excessos:
```
