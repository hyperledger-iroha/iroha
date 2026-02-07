---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visualização do fluxo de convite

## Objetivo

O elemento de roteiro **DOCS-SORA** cita a integração dos usuários e a visualização pública do programa de convites como os bloqueios anteriores antes da saída beta. Esta página decrita comentário ouvrir cada vago d'invitations, quels artefatos doivent etre livres antes de enviar os convites e comentar provar que o fluxo é auditável. Use-o com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para gerenciamento por refletor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de soma de verificação.
- [`devportal/observability`](./observability.md) para exportações de telemetria e ganchos de alerta.

## Plano de vagas

| Vago | Público | Critérios de entrada | Critérios de surtida | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores Docs/SDK validam o conteúdo do dia. | Equipe GitHub `docs-portal-preview` pessoa, gate checksum `npm run serve` em verde, Alertmanager silencioso 7 dias. | Todos os documentos P0 relus, backlog tage, nenhum incidente bloqueado. | Insira um validador para o fluxo; sem e-mail de convite, apenas parte da visualização dos artefatos. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii, relecteurs gouvernance sous NDA. | Termine W0, termos jurídicos aprovados, proxy Try-it en staging. | Sign-off Partners Collecte (issue ou formulaire signe), telemetrie montre <=10 relecteurs concurrents, pas de regressions securite pendente 14 dias. | Modelo de convite impostor + tickets de demanda. |
| **W2 - Comunauta** | Os colaboradores são selecionados a partir da lista de atendimento comunitário. | Termine W1, exercícios de incidentes repetidos, FAQ publique mise a jour. | Comentários adicionais, >=2 libera expedições de documentos por meio da visualização do pipeline sem reversão. | Limiter convida concorrentes (<=25) e batcher toda semana. |

Documente quelle vague está ativo em `status.md` e no rastreador de pré-visualização de demandas para que o governo voie o estatuto de um golpe de Estado.

## Pré-impressão da lista de verificação

Termine essas ações **avant** de planejar convites para uma vaga:

1. **Artefatos CI disponíveis**
   - Dernier `docs-portal-preview` + descritor de carga par `.github/workflows/docs-portal-preview.yml`.
   - Pin SoraFS nota em `docs/portal/docs/devportal/deploy-guide.md` (descritor de cutover presente).
2. **Aplicação da soma de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado via `npm run serve`.
   - Instruções `scripts/preview_verify.sh` testadas no macOS + Linux.
3. **Telemetria de linha de base**
   - `dashboards/grafana/docs_portal.json` montre un trafic Try it sain et l'alerte `docs.preview.integrity` est au vert.
   - Derniere annexe de `docs/portal/docs/devportal/observability.md` mise a jour com des gravam Grafana.
4. **Governança de artefatos**
   - Problema do rastreador de convite prete (um problema vago).
   - Modelo de registro de cópia de refletores (veja [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Approbations legales et SRE requer adidos a l'issue.

Registre o sucesso do pré-voo no rastreador de convites antes de enviar mais e-mails.

## Étapes du flux

1. **Selecionando os candidatos**
   - Tire a lista de contatos ou os parceiros de arquivo.
   - Certifique-se de que cada candidato tenha um modelo de demanda completo.
2. **Aprovar acesso**
   - Atribua um aprovador ao rastreador de problemas do convite.
   - Verificador de pré-requisitos (CLA/contrato, uso aceitável, breve segurança).
3. **Envie os convites**
   - Preencha os espaços reservados de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Junte o descritor + hash do arquivo, o URL de teste Try it e os canais de suporte.
   - Armazene o e-mail final (ou transcrição Matrix/Slack) na edição.
4. **Siga a integração**
   - Instale agora o rastreador de convites com `invite_sent_at`, `expected_exit_at` e o status (`pending`, `active`, `complete`, `revoked`).
   - Lier la demande d'entree du relecteur pour auditabilite.
5. **Vigilante da telemetria**
   - Vigilante `docs.preview.session_active` e os alertas `TryItProxyErrors`.
   - Abra um incidente no desvio de telemetria da linha de base e registre o resultado na parte de entrada do convite.
6. **Colete os comentários e classifique**
   - Fechar os convites quando o feedback chegar ou que `expected_exit_at` for confirmado.
   - Mettre a jour a questão da vaga com um currículo judicial (constatações, incidentes, ações prochaines) antes de passar para a coorte seguinte.

## Evidências e relatórios| Artefato | Ou estocador | Cadência de mise a jour |
| --- | --- | --- |
| Problema do rastreador de convite | Projeto GitHub `docs-portal-preview` | Mettre a jour apres cada convite. |
| Exportação de refletores de lista | Registre-se em `docs/portal/docs/devportal/reviewer-onboarding.md` | Hebdomadaire. |
| Telemetria de instantâneos | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar o pacote telemétrico) | Par vague + incidentes après. |
| Resumir feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (criar um dossiê por vago) | Nos 5 dias seguintes à saída de vaga. |
| Nota de reunião de governança | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Uma solução para cada controle de sincronização DOCS-SORA. |

Lancez `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
depois de cada lote para produzir um resumo listável por máquina. Participe do JSON rendendo a emissão de vaga para que os relecteurs gouvernance confirmem as contas de convites sem rejouer todo o log.

Acesse a lista de recomendações para `status.md` e algumas coisas vagas até que o roteiro principal possa ser feito rapidamente.

## Critérios de reversão e pausa

Faça uma pausa no fluxo de convites (e notifique o governo) durante os seguintes casos sobreviventes:

- Proxy de incidente Experimente antes de precisar de uma reversão (`npm run manage:tryit-proxy`).
- Fadiga de alertas: >3 páginas de alerta para visualização dos endpoints apenas durante 7 dias.
- Ecart de conformite: convite enviado sem assinatura ou sem registro do modelo de demanda.
- Risco de integridade: detecção de incompatibilidade de soma de verificação par `scripts/preview_verify.sh`.

Repita apenas depois de documentar a correção no rastreador de convites e confirme se a telemetria do painel está estável por menos de 48 horas.