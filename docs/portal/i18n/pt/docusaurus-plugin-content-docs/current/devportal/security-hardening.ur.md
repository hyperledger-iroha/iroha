---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سیکیورٹی ہارڈننگ اور pen-test چیک لسٹ

## جائزہ

روڈمیپ آئٹم **DOCS-1b** Login de código de dispositivo OAuth, políticas de segurança de conteúdo,
Testes de penetração repetíveis کا تقاضا کرتا ہے اس سے پہلے کہ visualização
O que você precisa saber یہ ضمیمہ modelo de ameaça, repo میں implementar کئے گئے controles, اور go-live
چیک لسٹ بیان کرتا ہے جسے gate reviews کو executar کرنا ہوتا ہے۔

- **اسکوپ:** Experimente proxy, painéis Swagger/RapiDoc incorporados, ou console de teste personalizado ou
  `docs/portal/src/components/TryItConsole.jsx` para renderizar ہوتی ہے۔
- **آؤٹ آف اسکوپ:** Torii خود (Torii avaliações de prontidão میں کور) اور SoraFS publicação
  (DOCS-3/7 میں کور).

## Modelo de ameaça

| Ativo | Risco | Mitigação |
| --- | --- | --- |
| Tokens ao portador Torii | docs sandbox کے باہر roubo e reutilização | login com código de dispositivo (`DOCS_OAUTH_*`) tokens de curta duração mint کرتا ہے، cabeçalhos de proxy کو redigir کرتا ہے، اور credenciais armazenadas em cache do console کو expiração automática کرتی ہے۔ |
| Experimente proxy | open relay کے طور پر abuso یا Torii limites de taxa کا bypass | Listas de permissões de origem `scripts/tryit-proxy*.mjs`, limitação de taxa, investigações de integridade e `X-TryIt-Auth` کی encaminhamento explícito impor کرتا ہے؛ As credenciais persistem |
| Tempo de execução do portal | scripts entre sites e incorporações maliciosas | `docusaurus.config.js` Content-Security-Policy, Tipos confiáveis, e cabeçalhos de Permissions-Policy injetam کرتا ہے؛ scripts inline Docusaurus tempo de execução تک محدود ہیں۔ |
| Dados de observabilidade | falta de telemetria یا adulteração | Documento de sondas/painel `docs/portal/docs/devportal/observability.md` کرتا ہے؛ `scripts/portal-probe.mjs` publicar سے پہلے CI میں چلتا ہے۔ |

Adversários میں visualização pública دیکھنے e usuários curiosos, چوری شدہ links کو teste کرنے والے atores maliciosos,
Para navegadores comprometidos شامل ہیں جو raspagem de credenciais armazenadas کرنے کی کوشش کرتے ہیں۔ تمام controles کو
redes confiáveis کے بغیر navegadores de commodities پر کام کرنا چاہیے۔

## Controles necessários

1. **Login com código de dispositivo OAuth**
   -`DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` اور متعلقہ botões e ambiente de construção میں configure کریں۔
   - Experimente o cartão ایک widget de login (`OAuthDeviceLogin.jsx`) renderizar کرتا ہے جو busca de código do dispositivo کرتا ہے،
     endpoint de token پر poll کرتا ہے، اور expiração پر tokens کو auto-clear کرتا ہے۔ Substituições manuais do portador
     substituto de emergência کے لئے دستیاب رہتے ہیں۔
   - Configuração OAuth ausente ہو یا TTLs substitutos DOCS-1b کے Janela de 300-900 s سے باہر ہوں تو compilações falham ہوتے ہیں؛
     `DOCS_OAUTH_ALLOW_INSECURE=1` صرف visualizações locais descartáveis ​​کے لئے set کریں۔
2. **Proteções de proxy**
   - `scripts/tryit-proxy.mjs` origens permitidas, limites de taxa, limites de tamanho de solicitação, e tempos limite de upstream impõem کرتا ہے
     جبکہ `X-TryIt-Client` کے ساتھ tag de tráfego کرتا ہے اور logs سے tokens redigir کرتا ہے۔
   - Sonda de vivacidade `scripts/tryit-proxy-probe.mjs` e `docs/portal/docs/devportal/observability.md`
     As regras do painel definem کرتے ہیں؛ Implementação de lançamento
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` para exportação de cabeçalhos de segurança determinísticos:
     `Content-Security-Policy` (default-src self; listas de connect/img/script; requisitos de tipos confiáveis)
     `Permissions-Policy`, e `Referrer-Policy: no-referrer`۔
   - Lista de conexão CSP Código de dispositivo OAuth e endpoints de token کو lista de permissões کرتی ہے
     (صرف HTTPS, جب تک `DOCS_SECURITY_ALLOW_INSECURE=1` نہ ہو) Login do dispositivo کام کرے
     اور دوسرے origens کے لئے sandbox relax نہ ہو۔
   - cabeçalhos براہ راست HTML gerado میں incorporar ہوتے ہیں, اس لئے hosts estáticos کو اضافی configuração کی ضرورت نہیں۔
     scripts inline کو Docusaurus bootstrap تک محدود رکھیں۔
4. **Runbooks, observabilidade e reversão**
   - `docs/portal/docs/devportal/observability.md` sonda e painéis de controle بیان کرتا ہے جو falhas de login, códigos de resposta de proxy,
     Para solicitar orçamentos پر نظر رکھتے ہیں۔
   - Caminho de escalonamento `docs/portal/docs/devportal/incident-runbooks.md` کور کرتا ہے اگر abuso de sandbox ہو؛
     `scripts/tryit-proxy-rollback.mjs` کے ساتھ combine کریں تاکہ endpoints محفوظ طریقے سے switch ہوں۔

## Pen-test e lista de verificação de lançamento

ہر pré-visualização da promoção کے لئے یہ فہرست مکمل کریں (resultados کو liberar ticket کے ساتھ anexar کریں):1. **Verificação de fiação OAuth کریں**
   - `npm run start` کو produção `DOCS_OAUTH_*` exportações کے ساتھ چلائیں۔
   - صاف perfil do navegador سے Experimente o console کھولیں اور confirmar کریں کہ token de fluxo do código do dispositivo mint کرتا ہے،
     contagem regressiva vitalícia کرتا ہے، اور expiração یا sair کے بعد limpar campo کرتا ہے۔
2. **Teste de proxy **
   - `npm run tryit-proxy` کو staging Torii کے خلاف چلائیں, پھر
     Caminho de amostra configurado `npm run probe:tryit-proxy` کے ساتھ چلائیں۔
   - registra میں entradas `authSource=override` دیکھیں اور confirmar کریں کہ janela de limite de taxa excede ہونے پر
     contadores aumentam ہوتے ہیں۔
3. **CSP/Tipos confiáveis confirmam کریں**
   - `npm run build` چلائیں e `build/index.html` کھولیں۔ یقینی بنائیں کہ `<meta
     http-equiv="Content-Security-Policy">` tag diretivas esperadas سے match کرتا ہے
     Carregamento de visualização e DevTools e violações de CSP.
   - `npm run probe:portal` (یا curl) سے busca HTML implantada کریں؛ sondar ou falhar ou falhar
     `Content-Security-Policy`, `Permissions-Policy`, یا `Referrer-Policy` meta tags غائب ہوں یا
     `docusaurus.config.js` میں valores declarados سے مختلف ہوں, لہذا código de saída dos revisores de governança پر
     اعتماد کر سکتے ہیں بجائے saída curl دیکھنے کے۔
4. **Avaliação de observabilidade**
   - Experimente o painel proxy سبز ہو (limites de taxa, taxas de erro, métricas de investigação de integridade)۔
   - O host não é (implantação Netlify/SoraFS) ou `docs/portal/docs/devportal/incident-runbooks.md`
     Exercício de incidente
5. **Documento نتائج کریں**
   - capturas de tela/logs کو liberar ticket کے ساتھ anexar کریں۔
   - ہر localização کو modelo de relatório de remediação میں captura کریں
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Proprietários de proprietários, SLAs e evidências de reteste بعد میں آسانی سے auditoria ہو سکے۔
   - اس checklist کو link کریں تاکہ DOCS-1b roadmap item auditável رہے۔

اگر کوئی قدم falhar ہو جائے تو promoção روک دیں, ایک problema de bloqueio فائل کریں, اور `status.md` میں plano de remediação نوٹ کریں۔