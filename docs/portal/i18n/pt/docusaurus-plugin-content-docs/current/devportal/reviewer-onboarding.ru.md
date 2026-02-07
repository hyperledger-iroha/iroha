---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Pré-visualização de Онбординг ревьюеров

##Obzor

DOCS-SORA отслеживает поэтапный запуск портала разработчиков. Сборки с gate por checksum
(`npm run serve`) e усиленные потоки Experimente, clique aqui: Passo онбординг проверенных
ревьюеров до широкого открытия visualização pública. Isso foi descrito, como você pode fazer isso,
verifique a solução, verifique o fornecimento e certifique-se de usá-lo. Sim.
[visualizar fluxo de convite](./preview-invite-flow.md) para planejar o evento, каденции приглашений
e transporte telemétrico; não há foco no destino após sua recuperação.

- **В рамках:** ревьюеры, которым нужен доступ к preview docs (`docs-preview.sora`,
  сборки GitHub Pages ou бандлы SoraFS) do GA.
- **No Ramo:** Operadores Torii ou SoraFS (покрываются собственными onboarding-китами) и
  portal de produção (см.
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Роли и требования

| Papel | Типичные цели | Artefactos de trabalho | Nomeação |
| --- | --- | --- | --- |
| Mantenedor principal | Faça novos testes, faça testes de fumaça. | Identificador do GitHub, contatado no Matrix, fornecido pelo CLA. | Você está no comando GitHub `docs-preview`; Se você quiser fazer isso, este será o seu áudio. |
| Revisor parceiro | Verifique SDK-сниппеты ou conteúdo para ser atualizado para a versão pública. | E-mail corporativo, POC legal, termos de visualização do pedido. | Além disso, você pode trabalhar com telemetria e trabalhar em dias. |
| Voluntário comunitário | Dê feedback sobre o uso de dados. | Identificador do GitHub, contato de contato, endereço de e-mail, conexão com CoC. | Держите когорты небольшими; приоритет ревьюерам, подписавшим contrato de contribuição. |

Quais são os tipos de recompensas:

1. Verifique a política de download do arquivo de visualização.
2. Pesquisa de segurança/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Verifique o `docs/portal/scripts/preview_verify.sh` antes do tempo, como
   обслуживать любой локальный instantâneo.

## Ingestão de Proцесс

1. Faça login no site
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   forma (ou вставить ее в issue). Зафиксируйте как минимум: личность, способ связи,
   Identificador do GitHub, dados de planejamento atualizados e configurações de segurança de segurança.
2. Faça login no rastreamento `docs-preview` (problema do GitHub ou atualização do tíquete)
   e назначьте aprovador.
3. Prover o treinamento:
   - CLA / contratante de contrato de parceria (ou consulta de contrato de parceria).
   - Verifique a segurança do documento no site.
   - Риск-оценка завершена (por exemplo, партнерские ревьюеры одобрены Legal).
4. O aprovador pode verificar o problema de rastreamento e resolver o problema de rastreamento com a melhor avaliação
   gerenciamento de mudanças (exemplo: `DOCS-SORA-Preview-####`).

## Provisionamento e instrumentos

1. **Передать артефакты** — Предоставьте последний descritor de visualização + arquivo из
   Fluxo de trabalho CI ou pino SoraFS (artigo `docs-portal-preview`). Напомните ревьюерам запустить:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```2. **Servir com soma de verificação de aplicação** — Укажите команду с gate по checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Este é o teste `scripts/serve-verified-preview.mjs`, que não é construído
   não запускался случайно.

3. **Entre em contato com o GitHub (opционально)** — Não há nada que você possa precisar, faça
   revisado no comando GitHub `docs-preview` no período de revisão e configuração da configuração
   em sua casa.

4. **Коммуникация каналов поддержки** — Permite contato de plantão (Matrix/Slack) e
   Processe os dados de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** — Напомните, что собирается анонимизированная аналитика
   (veja [`observability`](./observability.md)). Dê feedback de formulário ou problema-шаблон,
   указанный в приглашении, и зафиксируйте событие через helper
   [`preview-feedback-log`](./preview-feedback-log), este item está configurado corretamente.

## Verifique a revisão

Antes de enviar a pré-visualização, os resultados serão exibidos:

1. Verifique os artefatos de digitalização (`preview_verify.sh`).
2. Abra o portal `npm run serve` (ou `serve:verified`), sua soma de verificação de proteção está ativada.
3. Verifique seus parâmetros de segurança e observabilidade.
4. Verifique o OAuth/Try it para obter o login do código do dispositivo (primeiro) e não verifique
   produção de tokens.
5. Фиксировать находки в согласованном трекере (issue, общий документ или форма) и тегировать
   é uma pré-visualização do tema.

## Ответственности мейнтейнеров e offboarding

| Faz | Destino |
| --- | --- |
| Início | Убедиться, что ingestão чеклист приложен к заявке, поделиться артефактами + инструкциями, добавить запись `invite-sent` через [`preview-feedback-log`](./preview-feedback-log), e запланировать промежуточный sincronização, если ревью длится более não. |
| Monitoramento | Abra o telefone de visualização (tráfego desnecessário, experimente, experimente) e selecione o indicador-ранбуку при подозрениях. Para obter a solução `feedback-submitted`/`issue-opened` com mais frequência, esses valores métricos serão mais eficientes. |
| Desativação | Faça o download do GitHub ou SoraFS, identifique `access-revoked`, arquivo de arquivo (você pode obter feedback resumido + открытые действия), e обновить реестр ревьюеров. Попросить ревьюера удалить локальные сборки и приложить digest из [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Use este processo de rotação para obter mais voltas. Сохранение следов в репозитории
(issue + шаблоны) помогает DOCS-SORA оставаться аудируемым и позволяет governança подтвердить,
что доступ к preview следовал документированным контролям.

## Шаблоны приглашений e трекинг- Начинайте каждое обращение с файла
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Na física mínima do sistema, instruções para visualização e verificação da soma de verificação,
  что ревьюеры признают политику допустимого использования.
- Ao redigitar a chave, instale os conectores `<preview_tag>`, `<request_ticket>` e canais elétricos.
  Solicite uma cópia final da solicitação de ingresso, revisores, aprovadores e auditores
  Você pode se conectar a esse texto.
- Você pode obter uma planilha de rastreamento de rastreamento ou emitir um carimbo de data/hora `invite_sent_at`
  e ожидаемой датой завершения, чтобы отчет
  [visualizar fluxo de convite](./preview-invite-flow.md) автоматически подхватил когорту.