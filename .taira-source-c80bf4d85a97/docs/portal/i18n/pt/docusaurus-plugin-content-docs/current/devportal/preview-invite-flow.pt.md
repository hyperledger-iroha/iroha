---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo de convites do preview

##Objetivo

O item do roadmap **DOCS-SORA** destaca o onboarding de revisores e o programa de convites do preview público como os últimos bloqueadores antes de o portal sair de beta. Esta página descreve como abrir cada onda de convites, quais artefatos devem ser enviados antes de enviar convites e como provar que o fluxo é auditável. Use junto com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para o manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de soma de verificação.
- [`devportal/observability`](./observability.md) para exportações de telemetria e ganchos de alertas.

## Plano de ondas

| Onda | Audiência | Critérios de entrada | Critérios de disseda | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores do Docs/SDK validando o conteúdo do dia um. | Time GitHub `docs-portal-preview` preenchido, portão de checksum `npm run serve` verde, Alertmanager silencioso por 7 dias. | Todos os documentos P0 revisados, backlog tagueado, sem incidentes bloqueados. | Usado para validar o fluxo; sem email de convite, apenas compartilhe os artefatos de visualização. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii, revisores de governança sob NDA. | W0 encerrado, termos legais aprovados, proxy Try-it em staging. | Sign-off dos parceiros coletados (emissão ou formulário contratado), telemetria mostra <=10 revisores concorrentes, sem regressos de segurança por 14 dias. | Aplicar template de convite + tickets de solicitação. |
| **W2 - Comunidade** | Contribuidores selecionados da lista de esperança da comunidade. | W1 encerrado, treinos de incidentes ensaiados, FAQ publicado atualizado. | Feedback digitado, >=2 releases de documentação enviados via pipeline de preview sem rollback. | Limitar convites concorrentes (<=25) e agrupar semanalmente. |

Documente qual onda está ativa em `status.md` e no tracker de solicitações de visualização para que a governança veja o estado rapidamente.

## Checklist de pré-voo

Conclua estas ações **antes** de agendar convites para uma onda:

1. **Artefatos de CI disponíveis**
   - Ultimo `docs-portal-preview` + descritor enviado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado em `docs/portal/docs/devportal/deploy-guide.md` (descritor de cutover presente).
2. **Aplicação da soma de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado via `npm run serve`.
   - Instruções de `scripts/preview_verify.sh` testadas em macOS + Linux.
3. **Linha de base de telemetria**
   - `dashboards/grafana/docs_portal.json` mostra trafego Try it saudavel e o alerta `docs.preview.integrity` esta verde.
   - Último apêndice de `docs/portal/docs/devportal/observability.md` atualizado com links do Grafana.
4. **Artefatos de governança**
   - Issue do rastreador de convites pronto (uma issue por onda).
   - Modelo de registro de revisores copiados (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovações legais e de SRE requeridas anexadas a questão.

Registre a conclusão do pré-voo no rastreador de convite antes de enviar qualquer e-mail.

## Etapas do fluxo

1. **Selecionar candidatos**
   - Puxar a planilha de espera ou fila de parceiros.
   - Garantir que cada candidato tenha o modelo de solicitação completo.
2. **Aprovar acesso**
   - Atribuir um aprovador de emissão do rastreador de convites.
   - Verificar pré-requisitos (CLA/contrato, uso aceitavel, brief de segurança).
3. **Enviar convites**
   - Preencher os placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Anexar o descritor + hash do arquivo, URL de staging do Try it, e canais de suporte.
   - Guardar o email final (ou transcrição do Matrix/Slack) na edição.
4. **Acompanhar onboarding**
   - Atualizar o rastreador de convite com `invite_sent_at`, `expected_exit_at`, e status (`pending`, `active`, `complete`, `revoked`).
   - Linkar a solicitação de entrada do revisor para auditabilidade.
5. **Monitorar telemetria**
   - Observar `docs.preview.session_active` e alertas `TryItProxyErrors`.
   - Abrir um incidente se a telemetria desviar da linha de base e registrar o resultado ao lado da entrada de convite.
6. **Coletar feedback e encerrar**
   - Cerrar convites quando o feedback chegar ou `expected_exit_at` expirar.
   - Atualizar a edição da onda com um resumo curto (achados, incidentes, próximos atos) antes de passar para a próxima coorte.

## Evidência e relatórios| Artefato | Onde armazenar | Cadência de atualização |
| --- | --- | --- |
| Problema do rastreador de convites | Projeto GitHub `docs-portal-preview` | Atualizar após cada convite. |
| Exportação da lista de revisores | Registro registrado em `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Instantâneos de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar pacote de telemetria) | Por onda + após incidentes. |
| Resumo do feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (criar macarrão por onda) | Dentro de 5 dias após a saida da onda. |
| Nota de reunião de governança | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Pré-encher antes de cada sincronização DOCS-SORA. |

Execute `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
após cada lote para produzir um resumo legítimo por maquina. Anexe o JSON renderizado a issue da onda para que revisores de governança confirmem as contagens de convite sem reproduzir todo o log.

Anexe a lista de evidências a `status.md` sempre que uma onda terminar para que a entrada do roadmap possa ser atualizada rapidamente.

## Critérios de rollback e pausa

Pause o fluxo de convites (e notifique a governança) quando qualquer um dos itens abaixo ocorrer:

- Incidente de proxy Try it que ocorreu rollback (`npm run manage:tryit-proxy`).
- Fadiga de alertas: >3 páginas de alerta para endpoints apenas de visualização em 7 dias.
- Lacuna de conformidade: convite enviado sem termos contratados ou sem registrador o modelo de solicitação.
- Risco de integridade: incompatibilidade de soma de verificação detectada por `scripts/preview_verify.sh`.

Retome somente após documentar a remediação no rastreador de convite e confirmar que o painel de telemetria está estabelecido por pelo menos 48 horas.