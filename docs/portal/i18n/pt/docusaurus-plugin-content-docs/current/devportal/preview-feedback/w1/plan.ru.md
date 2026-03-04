---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: Plano de comprovação para parceiros W1
sidebar_label: Plano W1
description: Você, владельцы e чек-лист доказательств для партнерской preview-когорты.
---

| Ponto | Detalhes |
| --- | --- |
| Volna | W1 - Parceiros e Integradores Torii |
| Céu aberto | 2º trimestre de 2025, 3º trimestre |
| Тег артефакта (plан) | `preview-2025-04-12` |
| Trecker | `DOCS-SORA-Preview-W1` |

##Céli

1. Pré-visualização da política de privacidade e governança.
2. Tente experimentá-lo proxy e телеметрические снимки para pacета приглашений.
3. Abra o checksum-верифицированный pré-visualize o artefato e teste os resultados.
4. Finalize a configuração do parceiro e feche-a para obter a garantia.

## Разбивка задач

| ID | Bem | Владелец | Croco | Status | Nomeação |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Получить юридическое одобрение на дополнение условий visualização | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | ✅ Melhor | Юридический тикет `DOCS-SORA-Preview-W1-Legal` lançado em 05/04/2025; Guia PDF para o caminho. |
| W1-P2 | Зафиксировать staging-окно Experimente proxy (2025-04-10) e forneça o proxy | Documentos/DevRel + Operações | 06/04/2025 | ✅ Melhor | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` lançado em 06/04/2025; транскрипт CLI e arquivos `.env.tryit-proxy.bak`. |
| W1-P3 | Selecione o arquivo de visualização (`preview-2025-04-12`), use `scripts/preview_verify.sh` + `npm run probe:portal`, descritor de arquivo/somas de verificação | PortalTL | 08/04/2025 | ✅ Melhor | O artefato e a lógica são fornecidos em `artifacts/docs_preview/W1/preview-2025-04-12/`; você sonda a sonda приложен к трекеру. |
| W1-P4 | Verifique os números de admissão de fornecedores (`DOCS-SORA-Preview-REQ-P01...P08`), envie contatos e NDA | Ligação para governação | 07/04/2025 | ✅ Melhor | Você está fazendo isso em 11/04/2025; ссылки на одобрения в трекере. |
| W1-P5 | Use o programa de texto (na base `docs/examples/docs_preview_invite_template.md`), use `<preview_tag>` e `<request_ticket>` para o proprietário | Líder do Documentos/DevRel | 08/04/2025 | ✅ Melhor | Черновик приглашения отправлен 2025-04-12 15:00 UTC вместе сссылками на артефакт. |

## Teste de comprovação

> Совет: запустите `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`, чтобы автоматически выполнить шаги 1-5 (build, проверка checksum, portal probe, link checker e обновление Try it proxy). O script é definido como JSON-LOG, mas pode ser uma questão de rastreamento de problema.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para operação `build/checksums.sha256` e `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e архивировать `build/link-report.json` são descritores.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou seja, um novo alvo é `--tryit-target`); Use `.env.tryit-proxy` e sofra `.bak` para reversão.
6. Verifique o problema W1 através dos logs (descritor de soma de verificação, teste de teste, verificação de proxy Try it e instantâneos Grafana).

## Чек-list доказательств

- [x] Подписанное юридическое одобрение (PDF или ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] telas Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descritor e soma de verificação `preview-2025-04-12` são exibidos em `artifacts/docs_preview/W1/`.
- [x] Tabela de lista listada com o número `invite_sent_at` (com o log W1 no rastreamento).
- [x] Артефакты обратной связи отражены em [`preview-feedback/w1/log.md`](./log.md) com uma estrutura diferente no parceiro (обновлено 2025-04-26 данными escalação/telemetria/edições).

Обновляйте этот plano por mais produção; трекер ссылается на него, чтобы сохранить roteiro de auditabilidade.

## Prossiga seu trabalho

1. Para o revisor дублировать шаблон
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   заполнить метаданные e сохранить готовую копию em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Política de segurança, pontos de verificação de telemetria e problemas de resolução no log atual
   [`preview-feedback/w1/log.md`](./log.md), esses revisores de governança podem ser avaliados
   полностью, не покидая репозиторий.
3. Para fornecer uma verificação de conhecimento de esportes ou operações, verifique seu artefato, указанному в логе,
   e связывать с issue трекера.