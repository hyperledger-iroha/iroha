---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Плейбук приглашений na visualização pública

## Esses programas

Isso é feito, como anônimo e fornece visualização pública também, como
fluxo de trabalho онбординга ревьюеров запущен. Para obter mais detalhes sobre os cartões DOCS-SORA,
garantia, o que é um programa de teste com artefatos de prova, instruções
para usar e usar seu canal.

- **Аудитория:** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  подписали политику uso aceitável para visualização.
- **Ограничения:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 дней, реакция на
  incidentes na tecnologia 24h.

## Чеклист gate перед запуском

Выполните эти задачи antes de abrir o programa:

1. Possível pré-visualização do arquivo criado no CI (`docs-portal-preview`,
   manifesto de soma de verificação, descritor, pacote SoraFS).
2. `npm run --prefix docs/portal serve` (checksum gate) protegido por esta tag.
3. Тикеты онбординга ревьюеров одобрены и связаны с волной приглашений.
4. Documentos sobre segurança, observabilidade e evidências de incidentes
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Forneça um formulário de feedback ou um modelo de problema (para gravidade, gravidade,
   скриншоты и информация об окружении).
6. O texto anunciado é Docs/DevRel + Governance.

## Pacote de pacote

A configuração do aplicativo é válida:

1. **Proverенные артефакты** — Faça uma pesquisa no manifesto/plano SoraFS ou no artefato GitHub,
   também o manifesto e o descritor da soma de verificação. Явно укажите команду верификации, чтобы
   ревьюеры могли запустить ее перед запуском сайта.
2. **Serviço de serviço** — Ative o comando de visualização com portão de soma de verificação:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться, а инциденты нужно сообщать немедленно.
4. **Canal обратной связи** — Crie uma consulta sobre o modelo/forma de emissão e obtenha a verificação durante o período de espera.
5. **Programas** — Selecione os dados necessários/programados, horário de expediente ou sincronização e atualização automática.

Primeira imagem em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования. Ativar espaços reservados (dados, URLs, contatos)
перед отправкой.

## Anfitrião de pré-visualização da publicação

Продвигайте preview host только после завершения онбординга утверждения change ticket.
Sim. [руководство по host de visualização de exposição](./preview-host-exposure.md) para testes de ponta a ponta
construir/publicar/verificar, instalado neste espaço.

1. **Build e упаковка:** Crie a tag de liberação e identifique os artefatos.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   Pino de criptografia para `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` em `artifacts/sorafs/`. Veja quais são as coisas que você precisa
   Por favor, este é o caso que você pode experimentar.

2. **Alias de visualização da versão:** Повторите команду без `--skip-submit`
   (укажите `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e доказательство alias de governança).
   Script privado do manifesto para `docs-preview.sora` e exibido
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` para pacote de evidências.

3. **Problema de implementação:** Убедитесь, esse alias é configurado e a soma de verificação é a tag
   перед отправкой приглашений.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Selecione `npm run serve` (`scripts/serve-verified-preview.mjs`) para obter um substituto,
   чтобы ревьюеры могли поднять локальную копию, если preview edge даст сбой.

## Comunicação Tamil

| Dia | Destino | Proprietário |
| --- | --- | --- |
| D-3 | Projeto final de texto, verificação de artefatos, verificação de simulação | Documentos/DevRel |
| D-2 | Aprovação da governança + ticket de alteração | Documentos/DevRel + Governança |
| D-1 | Definindo uma programação para o telefone, obtendo um rastreador com um serviço específico | Documentos/DevRel |
| D | Chamada inicial / horário comercial, monitoramento de telefonia | Documentos/DevRel + plantão |
| D+7 | Промежуточный resumo de feedback, triagem de problemas de bloqueio | Documentos/DevRel |
| D+14 | Abra o arquivo, abra o presente final, abra o resumo em `status.md` | Documentos/DevRel |

## Transporte e telemetria

1. Digite o valor da configuração, registro de data e hora e data exibida no registrador de feedback de visualização
   (см. [`preview-feedback-log`](./preview-feedback-log)), чтобы каждая волна разделяла
   один и тот же trilha de evidências:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```Fonte de alimentação: `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened` e `access-revoked`. Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json`; приложите его к тикету волны
   вместе с формами согласия. Use o resumo do auxiliar, чтобы подготовить аудируемый
   roll-up durante o período final:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Resumo JSON é projetado para ser enviado, aberto para comentários e comentários
   e timestamp são usados. Ajudante
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Esse fluxo de trabalho pode ser transferido localmente ou no CI. Use o resumo do resumo em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   recapitulação pública.
2. Тегируйте телеметрийные dashboards `DOCS_RELEASE_TAG`, использованным для волны, чтобы пики
   Você pode ser contratado com um programa de coorte.
3. Abra `npm run probe:portal -- --expect-release=<tag>` para implantar, instale-o,
   что preview среда объявляет корректную metadados de lançamento.
4. Deixe os incidentes registrados no runbook do projeto e selecione sua coorte.

## Feedback e avaliação

1. Obtenha feedback no documento ou no quadro de problemas. Marcar elementos `docs-preview/<wave>`,
   roteiro dos proprietários de чтобы могли быстро их найти.
2. Use o resumo do registrador de pré-visualização para abrir o seu site, faça a visualização do arquivo em
   `status.md` (uso, conjunto de chaves, planejamento de arquivos) e atualização `roadmap.md`,
   если marco DOCS-SORA изменился.
3. Следуйте шагам offboarding из
   [`reviewer-onboarding`](./reviewer-onboarding.md): отзовите доступ, архивируйте заявки и
   поблагодарите участников.
4. Abra a janela, abra os artefatos, configure os portões de soma de verificação e
   обновив шаблон приглашения с новыми датами.

Последовательное применение этого плейбука делает programa preview auditable и дает
Docs/DevRel fornece suporte ao usuário para obter mais informações sobre o portal GA.