---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visualização do programa de pré-visualização

## Abençoado

Пункт дорожной карты **DOCS-SORA** указывает onboarding ревьюеров и программу приглашений visualização pública как последние блокеры antes de abrir o portal da versão beta. Esta é a descrição de como abrir o que está acontecendo, como os artefatos estão disponíveis antes рассылкой и как доказать, что поток аудируем. Use-o em:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para tarefas com problemas de recuperação.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para soma de verificação garantida.
- [`devportal/observability`](./observability.md) para transporte de cabos e ganchos.

## Plano de ação

| Volna | Auditoria | Critérios de entrada | Critérios de avaliação | Nomeação |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores Docs/SDK, valide o conteúdo hoje. | O comando GitHub `docs-portal-preview` foi instalado, soma de verificação do portão em `npm run serve` foi instalado, Alertmanager até 7 de dezembro. | Se o P0 for implementado, o backlog será processado, o bloqueio de eventos não será feito. | Используется для проверки потока; email-инвайтов нет, только обмен preview артефактами. |
| **W1 - Parceiros** | Operador SoraFS, integrador Torii, renova governança do NDA. | W0 é confiável, юридические условия утверждены, Try-it proxy no teste. | Собран sign-off партнеров (issue или подписанная форма), телеметрия показывает <=10 одновременных ревьюеров, нет регрессий безопасности 14 de dezembro. | Обязать шаблон приглашения + solicitar ingressos. |
| **W2 - Comunidade** | Избранные участники na lista de espera da comunidade. | W1 foi aprovado, conforme comprovado, FAQ publicado. | Фидбек обработан, >=2 документационных релиза прошли через pipeline de visualização sem reversão. | Ограничить одновременные приглашения (<=25) e батчить еженедельно. |

Ao documentar o volume ativo em `status.md` e no rastreador de solicitação de visualização, esse status de governança é exibido com a permissão anterior.

## Teste de comprovação

Выполните эти действия **перед** planejamento de programa para волны:

1. **Documentos de CI **
   - Последний `docs-portal-preview` + descritor загружен `.github/workflows/docs-portal-preview.yml`.
   - Pino SoraFS colocado em `docs/portal/docs/devportal/deploy-guide.md` (transferência do descritor присутствует).
2. **Soma de verificação de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` é usado para `npm run serve`.
   - Instruções `scripts/preview_verify.sh` de proteção para macOS + Linux.
3. **Telemetria de segurança**
   - `dashboards/grafana/docs_portal.json` показывает здоровый трафик Experimente e alerte `docs.preview.integrity` зеленый.
   - Use a configuração `docs/portal/docs/devportal/observability.md` para obter a configuração Grafana.
4. **Governança de artefatos**
   - Emitir rastreador de convite готов (одна issue на волну).
   - Шаблон реестра ревьюеров скопирован (см. [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovações Юридические и SRE приложены к emissão.

Verifique o comprovante de simulação no rastreador de convites antes de visualizar a imagem.

## Шаги потока

1. **Выбор кандидатов**
   - Verifique a planilha da lista de espera ou a fila do parceiro.
   - Убедиться, что у каждого кандидата заполнен modelo de solicitação.
2. **Envio de compra**
   - Aprovador de emissão de rastreador de convite de emissão.
   - Проверить требования (CLA/контракт, uso aceitável, resumo de segurança).
3. **Programação de transferência**
   - Instale os conectores em [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Приложить descritor + hash архива, Experimente o URL de teste e os canais possíveis.
   - Сохранить финальное письмо (ou transcrição Matrix/Slack) na edição.
4. **Integração de integração**
   - Обновить rastreador de convites `invite_sent_at`, `expected_exit_at`, e status (`pending`, `active`, `complete`, `revoked`).
   - Solicitação de entrada privada ревьюера для аудитабельности.
5. **Monitorização de telemetria**
   - Confirme o `docs.preview.session_active` e os alertas `TryItProxyErrors`.
   - Deixe o incidente fora da linha de base e verifique o resultado da operação com a configuração correta.
6. **Сбор фидбека и выход**
   - Abra a configuração de segurança ou instalação `expected_exit_at`.
   - Обновить issue волны кратким резюме (находки, инциденты, следующие шаги) перед переходом к следующей bom.

## Evidência e descoberta| Artefato | Где хранить | Desenvolvimento de castas |
| --- | --- | --- |
| Emitir rastreador de convite | Projeto GitHub `docs-portal-preview` | Обновлять после каждого приглашения. |
| Escalação do Export ревьюеров | Реестр, связанный em `docs/portal/docs/devportal/reviewer-onboarding.md` | Muito bem. |
| Telemetria telefônica | `docs/source/sdk/android/readiness/dashboards/<date>/` (pacote de telemetria de transferência) | На каждую волну + после инцидентов. |
| Resumo de feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (pacote de volume) | Na técnica de 5 de janeiro, você pode sair de casa. |
| Nota da reunião de governança | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Faça o download da sincronização de governança DOCS-SORA. |

Resolver `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
после каждого батча, чтобы получить машинно-читабельный resumo. Прикрепите рендеренный JSON к issue волны, чтобы ревьюеры governança pode ser capaz de fornecer uma conexão segura воспроизведения всего лога.

Прикрепляйте список evidencia к `status.md` при завершении каждой волны, чтобы дорожную карту можно было быстро обновить.

## Critérios de reversão e pausas

Приостановите поток приглашений (e уведомите governança), если происходит что-либо из следующего:

- Инцидент Experimente proxy, потребовавший rollback (`npm run manage:tryit-proxy`).
- Adicione alertas: >3 páginas de alerta para endpoints somente de visualização na tecnologia 7 de dezembro.
- Пробел комплаенса: приглашение отправлено без подписанных условий или без записи request template.
- Риск целостности: incompatibilidade de soma de verificação, обнаруженный `scripts/preview_verify.sh`.

Você pode usar a correção de documentação no rastreador de convites e manter a estabilidade do telefone no mínimo 48 часов.