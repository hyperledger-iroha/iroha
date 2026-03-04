---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Hardening de segurança e checklist de pen-test

## Visão geral

O item do roadmap **DOCS-1b** exige login OAuth device-code, políticas fortes de segurança de conteúdo
e testes de penetração repetitivos antes do portal preview rodar em redes fora do laboratório. Este
apêndice explica o modelo de ameacas, os controles implementados no repo e a checklist de go-live que
os gate reviews devem ser executados.

- **Escopo:** o proxy Try it, painéis Swagger/RapiDoc embutidos e um console Try it custom renderizado por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fora do escopo:** Torii em si (coberto por avaliações de prontidão do Torii) e publicação SoraFS
  (coberta por DOCS-3/7).

## Modelo de ameacas

| Ativo | Risco | Mitigação |
| --- | --- | --- |
| Portador de tokens do Torii | Roubo ou reuso fora do sandbox de documentos | O login device-code (`DOCS_OAUTH_*`) emite tokens de curta duração, o proxy redige headers e o console expira credenciais em cache automaticamente. |
| Proxy Experimente | Abuso como relé aberto ou bypass de limites de Torii | `scripts/tryit-proxy*.mjs` aplica listas de permissões de origem, limitação de taxa, investigações de integridade e encaminhamento explícito de `X-TryIt-Auth`; sem credencial e persistente. |
| Tempo de execução do portal | Cross-site scripting ou incorporações maliciosas | `docusaurus.config.js` injeta cabeçalhos Content-Security-Policy, Trusted Types e Permissions-Policy; scripts inline ficam restritos ao tempo de execução do Docusaurus. |
| Dados de observabilidade | Telemetria ausente ou adulterada | `docs/portal/docs/devportal/observability.md` sondas/painel de documentação; `scripts/portal-probe.mjs` roda em CI antes de publicar. |

Adversarios incluem usuários curiosos vendo o preview publico, fatores maliciosos testando links vazados e
navegadores comprometidos tentando extrair credenciais armazenadas. Todos os controles devem funcionar em
navegadores comuns sem redes confiáveis.

## Controles necessários

1. **Login com código de dispositivo OAuth**
   - Configurar `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` e botões relacionados no ambiente de construção.
   - O card Try it renderiza um widget de login (`OAuthDeviceLogin.jsx`) que
     busca o código do dispositivo, faz polling no token endpoint e limpa automaticamente
     tokens quando expirarem. Substitui manuais de Bearer permanecem disponíveis
     para reserva de emergência.
   - Os builds agora falham quando a configuração OAuth está ausente ou quando os
     TTLs de fallback saem da janela 300-900 s utilizados pelo DOCS-1b; ajuste
     `DOCS_OAUTH_ALLOW_INSECURE=1` apenas para visualizações locais descartaveis.
2. **Guarda-corpos fazem proxy**
   - `scripts/tryit-proxy.mjs` aplicações permitidas origens, limites de taxa, limites de tamanho de solicitação
     e timeouts upstream enquanto marca o trafego com `X-TryIt-Client` e
     remova tokens dos logs.
   -`scripts/tryit-proxy-probe.mjs` mais `docs/portal/docs/devportal/observability.md`
     definir uma sonda de liveness e regras de dashboard; execute-os antes de cada
     lançamento.
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` agora exporta cabeçalhos de segurança deterministas:
     `Content-Security-Policy` (default-src self, listas de limites de connect/img/script,
     requisitos de Trusted Types), `Permissions-Policy`, e
     `Referrer-Policy: no-referrer`.
   - Uma lista de conexão do CSP whitelist dos endpoints OAuth de device-code e token
     (somente HTTPS a menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para que o login do dispositivo
     funcionar sem relaxar o sandbox para outras origens.
   - Os cabeçalhos são incorporados diretamente no HTML gerado, então hosts estaticos não precisam
     de configuração extra. Mantenha scripts inline limitados ao bootstrap do Docusaurus.
4. **Runbooks, observabilidade e reversão**
   - `docs/portal/docs/devportal/observability.md` descreve os probes e dashboards que
     monitoramos falhas de login, códigos de resposta do proxy e orçamentos de solicitação.
   - `docs/portal/docs/devportal/incident-runbooks.md` cobre o caminho de escalada
     se o sandbox for abusado; combinar com
     `scripts/tryit-proxy-rollback.mjs` para virar endpoints com segurança.

## Checklist de pen-test e release

Complete esta lista para cada promoção de pré-visualização (anexo de resultados ao ticket de lançamento):1. **Verificar fiação OAuth**
   - Execute `npm run start` localmente com os exports `DOCS_OAUTH_*` de produção.
   - A partir de um perfil de navegador limpo, abra o console Try it e confirme que o
     fluxo device-code emite um token, conta a duração e limpa o campo após expirar
     ou fazer logout.
2. **Provar o proxy**
   - `npm run tryit-proxy` contra Torii teste, depois de executar
     `npm run probe:tryit-proxy` com o caminho de amostra configurado.
   - Verifique os logs das entradas `authSource=override` e confirme que o rate limiting
     incrementa contadores quando você excede a janela.
3. **Confirmar CSP/tipos confiáveis**
   - `npm run build` e abra `build/index.html`. Garanta que a tag `<meta
     http-equiv="Content-Security-Policy">` corresponde às disposições esperadas
     e que o DevTools não mostra violações CSP ao carregar o preview.
   - Utilize `npm run probe:portal` (ou curl) para buscar o HTML implantado; uma sonda
     agora falha quando as meta tags `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` está ausente ou muito dos valores declarados em
     `docusaurus.config.js`, assim revisores de governança podem confiar no
     código de saída em vez de funcionar ou saída do curl.
4. **Revisar observabilidade**
   - Verifique se o dashboard do proxy Try it esta verde (limites de taxa, taxas de erro,
     métricas de investigação de saúde).
   - Execute o exercício de incidentes em `docs/portal/docs/devportal/incident-runbooks.md`
     se o host mudou (nova implantação Netlify/SoraFS).
5. **Resultados do Documento**
   - Capturas de tela/logs do anexo ao ticket de lançamento.
   - Capture cada achado no modelo de relatorio de remediação
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que proprietários, SLAs e evidências de reteste sejam auditados depois.
   - Link de volta para este checklist para que o item do roadmap DOCS-1b continue auditável.

Se alguma etapa falhar, pare a promoção, abra um problema bloqueador e anote o plano de remediação em `status.md`.