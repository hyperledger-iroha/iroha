---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo de convites de visualização

## Propósito

O item do roteiro **DOCS-SORA** identifica o onboarding de revisores e o programa de convites de visualização pública como os bloqueios finais antes de o portal sair de beta. Esta página descreve como abrir cada formulário de convite, que os artefatos devem ser enviados antes de enviar convites e como demonstrar que o fluxo é auditável. Usala junto com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para o manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de soma de verificação.
- [`devportal/observability`](./observability.md) para exportações de telemetria e ganchos de alertas.

## Plano de olas

| Olá | Audiência | Critérios de entrada | Critérios de saída | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores do Docs/SDK validando o conteúdo do dia um. | Equipamento GitHub `docs-portal-preview` poblado, portão de checksum em `npm run serve` em verde, Alertmanager silencioso por 7 dias. | Todos os documentos P0 revisados, backlog marcados, sem incidentes bloqueadores. | Use-o para validar o fluxo; não há e-mail de convite, apenas compartilhe os artefatos de visualização. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii, revisores de governança sob NDA. | W0 cerrado, termos legais aprovados, proxy Try-it en staging. | Sign-off de parceiros (emissão de formulário firmado) reconhecido, telemetria mostra <=10 revisores simultâneos, sem regressões de segurança por 14 dias. | Aplicar planta de convite + tickets de solicitação. |
| **W2 - Comunidade** | Contribuidores selecionados da lista de espera da comunidade. | W1 cerrado, treinos de incidentes ensayados, FAQ publicado atualizado. | Feedback digitado, >=2 releases de documentação enviada via pipeline de preview sin rollback. | Limitar convites simultâneos (<=25) e agrupá-los semanalmente. |

Documento que ola está ativado em `status.md` e no rastreador de solicitações de visualização para que a governança e o estado de um vistazo.

## Checklist de pré-voo

Complete estas ações **antes** de programar convites para uma pessoa:

1. **Artefatos de CI disponíveis**
   - O último `docs-portal-preview` + descritor carregado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado em `docs/portal/docs/devportal/deploy-guide.md` (descritor de cutover presente).
2. **Aplicação da soma de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado via `npm run serve`.
   - Instruções de `scripts/preview_verify.sh` testadas em macOS + Linux.
3. **Linha de base de telemetria**
   - `dashboards/grafana/docs_portal.json` mostra o tráfego Try it saludable e o alerta `docs.preview.integrity` está em verde.
   - Último apêndice de `docs/portal/docs/devportal/observability.md` atualizado com links de Grafana.
4. **Artefatos de governo**
   - Emitir lista de rastreadores de convites (um problema por ola).
   - Planta de registro de revisores copiados (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovações legais e de SRE requeridas adjuntas à la issue.

Registre a finalização do comprovante no rastreador de convite antes de enviar qualquer correspondência.

## Passos do fluxo

1. **Selecionar candidatos**
   - Extraer de la hoja de espera o cola de parceiros.
   - Certifique-se de que cada candidato tenha a planta de solicitação completa.
2. **Aprovar acesso**
   - Atribuir um aprovador ao problema do rastreador de convites.
   - Verificar pré-requisitos (CLA/contrato, uso aceitável, brief de seguridad).
3. **Enviar convites**
   - Completar os placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Adicione o descritor + hash do arquivo, URL de teste do Try it e canais de suporte.
   - Guardar o e-mail final (ou transcrição do Matrix/Slack) na edição.
4. **Integração do Rastrear**
   - Atualizar o rastreador de convite com `invite_sent_at`, `expected_exit_at`, e estado (`pending`, `active`, `complete`, `revoked`).
   - Enlazar a solicitação de entrada do revisor para auditabilidade.
5. **Telemetria monitora**
   - Vigilar `docs.preview.session_active` e alertas `TryItProxyErrors`.
   - Abra um incidente se a telemetria se desviar da linha de base e registrar o resultado junto com a entrada do convite.
6. **Recolha feedback e feche**
   - Cerrar convites quando o feedback for enviado ou `expected_exit_at` se cumprir.
   - Atualizar o problema da ola com um resumo curto (hallazgos, incidentes, ações seguintes) antes de passar para a próxima coorte.

## Evidências e relatórios| Artefato | Onde salvar | Cadência de atualização |
| --- | --- | --- |
| Edição do rastreador de convite | Projeto GitHub `docs-portal-preview` | Atualizar após cada convite. |
| Exportação da lista de revisores | Registro enlaçado em `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Instantâneos de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar pacote de telemetria) | Por ola + após incidentes. |
| Resumo do feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (criar pasta por ola) | Dentro de 5 dias tras salir de la ola. |
| Nota de reunião de governo | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Conclua antes de cada sincronização de governo DOCS-SORA. |

Ejecuta `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
depois de cada lote para produzir um resumo legível por máquinas. Adicione o JSON renderizado à edição da ola para que os revisores de governança confirmem o conteúdo dos convites sem reproduzir todo o log.

Adicione a lista de evidências a `status.md` sempre que um terminal terminar para que a entrada do roadmap possa ser atualizada rapidamente.

## Critérios de reversão e pausa

Pause o fluxo de convites (e notifique a governança) quando ocorrer qualquer um destes casos:

- Um incidente de proxy Try it que requer rollback (`npm run manage:tryit-proxy`).
- Fatiga de alertas: >3 páginas de alerta para endpoints apenas de visualização dentro de 7 dias.
- Brecha de cumprimento: convite enviado sem término firmados ou sem registrador da planta de solicitação.
- Riesgo de integridade: incompatibilidade de soma de verificação detectada por `scripts/preview_verify.sh`.

Reanuda só depois de documentar a remediação no rastreador de convites e confirmar que o painel de telemetria está estável por pelo menos 48 horas.