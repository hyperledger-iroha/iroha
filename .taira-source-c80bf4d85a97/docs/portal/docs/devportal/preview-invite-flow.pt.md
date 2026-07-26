---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1e6e4dda03f047326084118775695b76067c683eaf382388147a87518e45691e
source_last_modified: "2025-12-19T22:35:07.713078+00:00"
translation_last_reviewed: 2026-01-01
---

# Fluxo de convites do preview

## Objetivo

O item do roadmap **DOCS-SORA** destaca o onboarding de revisores e o programa de convites do preview publico como os ultimos bloqueadores antes de o portal sair de beta. Esta pagina descreve como abrir cada onda de convites, quais artefatos devem ser enviados antes de mandar convites e como provar que o fluxo e auditavel. Use junto com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para o manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de checksum.
- [`devportal/observability`](./observability.md) para exports de telemetria e hooks de alertas.

## Plano de ondas

| Onda | Audiencia | Criterios de entrada | Criterios de saida | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Maintainers core** | Maintainers de Docs/SDK validando conteudo do dia um. | Time GitHub `docs-portal-preview` populado, gate de checksum `npm run serve` verde, Alertmanager silencioso por 7 dias. | Todos os docs P0 revisados, backlog tagueado, sem incidentes bloqueadores. | Usado para validar o fluxo; sem email de convite, apenas compartilhar os artefatos de preview. |
| **W1 - Partners** | Operadores SoraFS, integradores Torii, revisores de governanca sob NDA. | W0 encerrado, termos legais aprovados, proxy Try-it em staging. | Sign-off dos partners coletado (issue ou formulario assinado), telemetria mostra <=10 revisores concorrentes, sem regressoes de seguranca por 14 dias. | Aplicar template de convite + tickets de solicitacao. |
| **W2 - Comunidade** | Contribuidores selecionados da lista de espera da comunidade. | W1 encerrado, drills de incidentes ensaiados, FAQ publico atualizado. | Feedback digerido, >=2 releases de documentacao enviados via pipeline de preview sem rollback. | Limitar convites concorrentes (<=25) e agrupar semanalmente. |

Documente qual onda esta ativa em `status.md` e no tracker de solicitacoes de preview para que a governanca veja o estado rapidamente.

## Checklist de preflight

Conclua estas acoes **antes** de agendar convites para uma onda:

1. **Artefatos de CI disponiveis**
   - Ultimo `docs-portal-preview` + descriptor enviado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado em `docs/portal/docs/devportal/deploy-guide.md` (descriptor de cutover presente).
2. **Enforcement de checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado via `npm run serve`.
   - Instrucoes de `scripts/preview_verify.sh` testadas em macOS + Linux.
3. **Baseline de telemetria**
   - `dashboards/grafana/docs_portal.json` mostra trafego Try it saudavel e o alerta `docs.preview.integrity` esta verde.
   - Ultimo apendice de `docs/portal/docs/devportal/observability.md` atualizado com links do Grafana.
4. **Artefatos de governanca**
   - Issue do invite tracker pronta (uma issue por onda).
   - Template de registro de revisores copiado (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovacoes legais e de SRE requeridas anexadas a issue.

Registre a conclusao do preflight no invite tracker antes de enviar qualquer email.

## Etapas do fluxo

1. **Selecionar candidatos**
   - Puxar da planilha de espera ou fila de partners.
   - Garantir que cada candidato tenha o template de solicitacao completo.
2. **Aprovar acesso**
   - Atribuir um aprovador a issue do invite tracker.
   - Verificar prerequisitos (CLA/contrato, uso aceitavel, brief de seguranca).
3. **Enviar convites**
   - Preencher os placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Anexar o descriptor + hash do archive, URL de staging do Try it, e canais de suporte.
   - Guardar o email final (ou transcript do Matrix/Slack) na issue.
4. **Acompanhar onboarding**
   - Atualizar o invite tracker com `invite_sent_at`, `expected_exit_at`, e status (`pending`, `active`, `complete`, `revoked`).
   - Linkar a solicitacao de entrada do revisor para auditabilidade.
5. **Monitorar telemetria**
   - Observar `docs.preview.session_active` e alertas `TryItProxyErrors`.
   - Abrir um incidente se a telemetria desviar do baseline e registrar o resultado ao lado da entrada de convite.
6. **Coletar feedback e encerrar**
   - Encerrar convites quando o feedback chegar ou `expected_exit_at` expirar.
   - Atualizar a issue da onda com um resumo curto (achados, incidentes, proximas acoes) antes de passar para a proxima coorte.

## Evidencia e reporting

| Artefato | Onde armazenar | Cadencia de atualizacao |
| --- | --- | --- |
| Issue do invite tracker | Projeto GitHub `docs-portal-preview` | Atualizar apos cada convite. |
| Export do roster de revisores | Registro vinculado em `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Snapshots de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar bundle de telemetria) | Por onda + apos incidentes. |
| Digest de feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (criar pasta por onda) | Dentro de 5 dias apos a saida da onda. |
| Nota de reuniao de governanca | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Preencher antes de cada sync DOCS-SORA. |

Execute `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
apos cada lote para produzir um digest legivel por maquina. Anexe o JSON renderizado a issue da onda para que revisores de governanca confirmem as contagens de convite sem reproduzir todo o log.

Anexe a lista de evidencias a `status.md` sempre que uma onda terminar para que a entrada do roadmap possa ser atualizada rapidamente.

## Criterios de rollback e pausa

Pause o fluxo de convites (e notifique a governanca) quando qualquer um dos itens abaixo ocorrer:

- Incidente de proxy Try it que exigiu rollback (`npm run manage:tryit-proxy`).
- Fadiga de alertas: >3 alert pages para endpoints apenas de preview em 7 dias.
- Gap de compliance: convite enviado sem termos assinados ou sem registrar o template de solicitacao.
- Risco de integridade: mismatch de checksum detectado por `scripts/preview_verify.sh`.

Retome somente apos documentar a remediacao no invite tracker e confirmar que o dashboard de telemetria esta estavel por pelo menos 48 horas.
