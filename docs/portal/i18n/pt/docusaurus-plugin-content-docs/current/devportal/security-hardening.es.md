---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Endurecimento de segurança e checklist de pen-test

## Resumo

O item do roteiro **DOCS-1b** requer login OAuth com código do dispositivo, políticas de segurança de conteúdo forte e
testes de caneta repetitivos antes de o portal de visualização poder funcionar em redes fora do laboratório. Este apêndice
explica o modelo de amenazas, os controles implementados no repositório e a lista de verificação de go-live que deve ser executada
as revisões do portão.

- **Alcance:** o proxy de Try it, painéis Swagger/RapiDoc incorporados e o console Try it custom renderizado por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fora de alcance:** Torii em si (cubierto por revisões de prontidão de Torii) e a publicação de SoraFS
  (cuberto por DOCS-3/7).

## Modelo de Amenazas

| Ativo | Riesgo | Mitigação |
| --- | --- | --- |
| Portador de tokens de Torii | Robo ou reutilização fora do sandbox de documentos | O código do dispositivo de login (`DOCS_OAUTH_*`) emite tokens de curta vida, o proxy redige cabeçalhos e o console expira credenciais cacheadas automaticamente. |
| Proxy de Experimente | Abuso como relé aberto ou bypass de limites de Torii | `scripts/tryit-proxy*.mjs` aplica listas de permissões de origem, limitação de taxa, investigações de integridade e encaminhamento explícito de `X-TryIt-Auth`; não se persistem credenciais. |
| Tempo de execução do portal | Cross-site scripting ou incorporações maliciosas | `docusaurus.config.js` cabeçalhos inyecta Content-Security-Policy, Trusted Types e Permissions-Policy; Os scripts inline são limitados ao tempo de execução de Docusaurus. |
| Dados de observação | Telemetria sem manipulação ou manipulação | `docs/portal/docs/devportal/observability.md` documenta os probes/dashboards; `scripts/portal-probe.mjs` corre em CI antes de publicar. |

Os adversários incluem usuários curiosos vendo a prévia pública, atores maliciosos procurando links roubados e
navegadores comprometidos que pretendem obter credenciais armazenadas. Todos os controles devem funcionar em
navegadores de uso comuns sem redes confiáveis.

## Controles necessários

1. **Login com código de dispositivo OAuth**
   - Configurar `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` e botões relacionados ao ambiente de construção.
   - A tarjeta Try it renderiza um widget de login (`OAuthDeviceLogin.jsx`) que
     obtenha o código do dispositivo, faça a pesquisa no endpoint do token e limpe automaticamente os tokens
     uma vez que expire. Os manuais de substituição do Bearer continuam disponíveis para
     substituto de emergência.
   - As compilações agora falham quando falta a configuração OAuth ou quando os TTLs de
     fallback se salen de la ventana 300-900 s utilizado por DOCS-1b; ajustar
     `DOCS_OAUTH_ALLOW_INSECURE=1` solo para visualizações de locais descartaveis.
2. **Guardas do proxy**
   - `scripts/tryit-proxy.mjs` aplica origens permitidas, limites de taxa, limites de tamanho de solicitação
     e timeouts upstream enquanto etiqueta o tráfego com `X-TryIt-Client` e redige tokens
     dos registros.
   - `scripts/tryit-proxy-probe.mjs` mas `docs/portal/docs/devportal/observability.md`
     definir a sonda de vivacidade e as regras do painel; ejecutá-los antes de cada
     lançamento.
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` agora exporta cabeçalhos de segurança deterministas:
     `Content-Security-Policy` (default-src self, listas restritas de connect/img/script,
     requisitos de Tipos Confiáveis), `Permissions-Policy` e
     `Referrer-Policy: no-referrer`.
   - A lista de conexão do CSP permite os endpoints OAuth device-code e token
     (somente HTTPS menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para fazer login no dispositivo
     funcionar sem relançar o sandbox para outras origens.
   - Os cabeçalhos são inseridos diretamente no HTML gerado, por meio dos hosts
     estaticos não necessitam de configuração extra. Manter os scripts inline
     limitados ao bootstrap de Docusaurus.
4. **Runbooks, observabilidade e reversão**
   - `docs/portal/docs/devportal/observability.md` descreve os testes e painéis que
     observe falhas de login, códigos de resposta de proxy e orçamentos de solicitações.
   - `docs/portal/docs/devportal/incident-runbooks.md` cobre a rota de escalada
     se o sandbox for abusado; combinar com
     `scripts/tryit-proxy-rollback.mjs` para alterar endpoints de forma segura.

## Checklist de pen-test e lançamento

Complete esta lista para cada promoção de pré-visualização (resultados adicionais ao ticket de lançamento):1. **Verificar fiação OAuth**
   - Ejecuta `npm run start` localmente com as exportações `DOCS_OAUTH_*` de produção.
   - Desde um perfil de navegador limpo, abra o console Try it e confirme que o flujo
     o código do dispositivo emite um token, conta regressivamente o tempo de vida e limpa o campo
     após expirar ou encerrar a sessão.
2. **Procurar proxy**
   - `npm run tryit-proxy` contra Torii encenação, luego ejecuta
     `npm run probe:tryit-proxy` com o caminho de amostra configurado.
   - Revisa os registros das entradas `authSource=override` e confirma a limitação da taxa
     incrementa contadores quando excede a janela.
3. **Confirmar CSP/tipos confiáveis**
   - `npm run build` e abre `build/index.html`. Certifique-se de que a tag `<meta
     http-equiv="Content-Security-Policy">` coincide com as diretrizes esperadas
     e o DevTools não mostra violações de CSP ao carregar a visualização.
   - Use `npm run probe:portal` (ou curl) para obter o HTML desplegado; a sonda
     agora falha quando as meta tags `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` não há diferenças nos valores declarados em
     `docusaurus.config.js`, assim como os revisores de governança podem confiar no
     código de saída em vez de inspecionar a saída de curl.
4. **Revisar observabilidade**
   - Verifique se o painel de proxy Try it está verde (limites de taxa, taxas de erro,
     métricas de investigação de saúde).
   - Execute o exercício de incidentes em `docs/portal/docs/devportal/incident-runbooks.md`
     se mudar o host (novo despliegue Netlify/SoraFS).
5. **Documente os resultados**
   - Adiciona capturas de tela/logs ao ticket de lançamento.
   - Captura de cada hallazgo na planta de relatório de remediação
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que proprietários, SLAs e evidências de reteste sejam fáceis de auditar após.
   - Vá para esta lista de verificação para que o item do roteiro DOCS-1b seja auditável.

Se alguma coisa falhar, detene a promoção, abra um problema bloqueador e anote o plano de remediação em `status.md`.