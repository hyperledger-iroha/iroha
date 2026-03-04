---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Use a caneta e a caneta de segurança

##Obzor

Пункт дорожной карты **DOCS-1b** требует OAuth device-code login, сильных политик
Obtenha conteúdo e teste de penetração avançados para ver o portal de visualização do site
trabalhe em uma série de laboratórios. Este modelo de descrição do modelo é real, realizável
nos controles de repositório e na verificação de ativação, você receberá comentários sobre o portão.

- **No рамках:** прокси Try it, встроенные панели Swagger/RapiDoc и кастомная
  консоль Experimente, рендеримая `docs/portal/src/components/TryItConsole.jsx`.
- **No рамок:** сам Torii (avaliações de prontidão Torii) e publicação SoraFS
  (покрывается DOCS-3/7).

## Modelo угроз

| Ativo | Risco | Mitigação |
| --- | --- | --- |
| Torii tokens de portador | Projeto ou serviço avançado usado em docs sandbox | login de código de dispositivo (`DOCS_OAUTH_*`) usa tokens de código de dispositivo, cabeçalhos de edição de proxy, um ícone de console automático créditos de crédito. |
| Experimente. Злоупотребление как открытым реле или обход лимитов Torii | `scripts/tryit-proxy*.mjs` listas de permissões de origem impostas, limitação de taxa, investigações de integridade e encaminhamento `X-TryIt-Auth`; os créditos não são garantidos. |
| Portal de tempo de execução | Cross-site scripting ou incorporados | `docusaurus.config.js` contém cabeçalhos Content-Security-Policy, Trusted Types e Permissions-Policy; scripts embutidos executam o tempo de execução Docusaurus. |
| Dannые наблюдаемости | Telemetria ou instalação | `docs/portal/docs/devportal/observability.md` sondas/painéis de documentação; `scripts/portal-probe.mjs` foi enviado para CI antes da publicação. |

Противники включают любопытных пользователей публичного visualização, злоумышленников, тестирующих украденные ссылки,
e скомпрометированные браузеры, пытающиеся вытащить сохраненные kреды. Quantos controles funcionam
no seu braseiro não há mais de um ano.

## Controle de controle

1. **Login com código de dispositivo OAuth**
   -Caixa `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` e botões de ajuste na operação de construção.
   - Cartão Experimente criar widget de login (`OAuthDeviceLogin.jsx`), который
     inserir o código do dispositivo, configurar o endpoint do token e transferir tokens automaticamente
     após a história. Ручные Bearer substitui остаются доступными для экстренного fallback.
   - A configuração padrão do OAuth não é a configuração OAuth ou TTLs substitutos
     выходят за окно 300-900 s, предписанное DOCS-1b; instalar
     `DOCS_OAUTH_ALLOW_INSECURE=1` é uma opção para pré-visualização local.
2. **Proteções de proxy**
   - `scripts/tryit-proxy.mjs` применяет origens permitidas, limites de taxa, limites máximos
     e tempos limite de upstream, alterando o tráfego `X-TryIt-Client` e редактируя токены
     na verdade.
   - `scripts/tryit-proxy-probe.mjs` mais `docs/portal/docs/devportal/observability.md`
     teste de vivacidade e painel de controle; запускайте их перед каждым rollout.
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` теперь экспортирует детерминированные cabeçalhos de segurança:
     `Content-Security-Policy` (default-src self, área de script connect/img/script,
     tipos confiáveis), `Permissions-Policy`, e
     `Referrer-Policy: no-referrer`.
   - Identificação da lista de permissões CSP, código de dispositivo OAuth e endpoints de token
     (para HTTPS, exceto `DOCS_SECURITY_ALLOW_INSECURE=1`), login do dispositivo
     работал без ослабления sandbox para других origens.
   - Esses cabeçalhos são criados em HTML de geração de dados, com status de status
     não é necessário configurar a configuração. Crie scripts in-line organizados
     Inicialização Docusaurus.
4. **Runbooks, observabilidade e reversão**
   - `docs/portal/docs/devportal/observability.md` sondas de análise e painéis, которые
     resolva falhas de login, códigos de resposta de proxy e orçamentos de solicitação.
   - `docs/portal/docs/devportal/incident-runbooks.md` описывает путь эскалации
     при злоупотреблении sandbox; combinar com
     `scripts/tryit-proxy-rollback.mjs` para endpoints de configuração segura.

## Verifique a caneta e a versão

Выполните этот список для каждого продвижения pré-visualização (utilize os resultados para liberar o ticket):1. **Proveritь fiação OAuth**
   - Запустите `npm run start` локально с produção `DOCS_OAUTH_*` exportações.
   - Seu perfil de usuário é aberto Experimente-o консоль e убедитесь, что fluxo de código do dispositivo
     выдает токен, считает время жизни e очищает поле после истечения или sair.
2. **Processar processo**
   - `npm run tryit-proxy` protив staging Torii, затем выполните
     `npm run probe:tryit-proxy` é o caminho de amostra definido.
   - Prover logs em `authSource=override` e atualizar, esta limitação de taxa
     увеличивает счетчики при превышении окна.
3. **Definir CSP/Tipos Confiáveis**
   - `npm run build` e откройте `build/index.html`. Убедитесь, что тег `<meta
     http-equiv="Content-Security-Policy">` соответствует ожидаемым директивам
     e este DevTools não contém violações de CSP durante a visualização.
   - Use `npm run probe:portal` (ou curl), use HTML de alta qualidade; sonda
     теперь падает, если meta tags `Content-Security-Policy`, `Permissions-Policy` или
     `Referrer-Policy` foi removido ou removido de dentro
     `docusaurus.config.js`, assim como a governança dos revisores pode ser útil
     código de saída вместо ручной проверки saída curl.
4. ** Observabilidade de avaliação **
   - Убедитесь, что painel Experimente proxy зеленый (limites de taxa, taxas de erro,
     métricas de investigação de integridade).
   - Faça um exercício de incidente em `docs/portal/docs/devportal/incident-runbooks.md`,
     exceto a configuração (nova configuração Netlify/SoraFS).
5. **Resultados de avaliação**
   - Faça capturas de tela/logs para liberar o ticket.
   - Зафиксируйте каждую находку в шаблоне relatório de remediação
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Proprietários de itens, SLAs e evidências para reteste são uma boa opção de audição.
   - Сошлитесь на этот чеклист, чтобы пункт дорожной карты DOCS-1b é auditável.

Se isso for comprovado, verifique o procedimento, resolva o problema de bloqueio e planeje um plano de remediação em `status.md`.