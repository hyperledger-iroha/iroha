---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Hardening de seguranca e checklist de pen-test

## Visao geral

O item do roadmap **DOCS-1b** exige login OAuth device-code, politicas fortes de seguranca de conteudo
e testes de penetracao repetiveis antes de o portal preview rodar em redes fora do laboratorio. Este
apendice explica o modelo de ameacas, os controles implementados no repo e a checklist de go-live que
os gate reviews devem executar.

- **Escopo:** o proxy Try it, paineis Swagger/RapiDoc embutidos e a console Try it custom renderizada por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fora do escopo:** Torii em si (coberto por reviews de readiness do Torii) e publicacao SoraFS
  (coberta por DOCS-3/7).

## Modelo de ameacas

| Ativo | Risco | Mitigacao |
| --- | --- | --- |
| Tokens bearer do Torii | Roubo ou reuso fora do sandbox de docs | O login device-code (`DOCS_OAUTH_*`) emite tokens de curta duracao, o proxy redige headers e a console expira credenciais em cache automaticamente. |
| Proxy Try it | Abuso como relay aberto ou bypass de limites de Torii | `scripts/tryit-proxy*.mjs` aplica allowlists de origem, rate limiting, health probes e forwarding explicito de `X-TryIt-Auth`; nenhuma credencial e persistida. |
| Runtime do portal | Cross-site scripting ou embeds maliciosos | `docusaurus.config.js` injeta headers Content-Security-Policy, Trusted Types e Permissions-Policy; scripts inline ficam restritos ao runtime do Docusaurus. |
| Dados de observabilidade | Telemetria ausente ou adulterada | `docs/portal/docs/devportal/observability.md` documenta probes/dashboards; `scripts/portal-probe.mjs` roda em CI antes de publicar. |

Adversarios incluem usuarios curiosos vendo o preview publico, atores maliciosos testando links roubados e
browsers comprometidos tentando extrair credenciais armazenadas. Todos os controles devem funcionar em
browsers comuns sem redes confiaveis.

## Controles requeridos

1. **OAuth device-code login**
   - Configure `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` e knobs relacionados no ambiente de build.
   - O card Try it renderiza um widget de sign-in (`OAuthDeviceLogin.jsx`) que
     busca o device code, faz polling no token endpoint e limpa automaticamente
     tokens quando expiram. Overrides manuais de Bearer permanecem disponiveis
     para fallback de emergencia.
   - Os builds agora falham quando a configuracao OAuth esta ausente ou quando os
     TTLs de fallback saem da janela 300-900 s exigida pelo DOCS-1b; ajuste
     `DOCS_OAUTH_ALLOW_INSECURE=1` apenas para previews locais descartaveis.
2. **Guardrails do proxy**
   - `scripts/tryit-proxy.mjs` aplica allowed origins, rate limits, caps de tamanho de request
     e timeouts upstream enquanto marca o trafego com `X-TryIt-Client` e
     remove tokens dos logs.
   - `scripts/tryit-proxy-probe.mjs` mais `docs/portal/docs/devportal/observability.md`
     definem a sonda de liveness e regras de dashboard; execute-os antes de cada
     rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` agora exporta headers de seguranca deterministas:
     `Content-Security-Policy` (default-src self, listas estritas de connect/img/script,
     requisitos de Trusted Types), `Permissions-Policy`, e
     `Referrer-Policy: no-referrer`.
   - A lista de connect do CSP whitelist os endpoints OAuth de device-code e token
     (somente HTTPS a menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para que o device login
     funcione sem relaxar o sandbox para outras origens.
   - Os headers sao embedados diretamente no HTML gerado, entao hosts estaticos nao precisam
     de configuracao extra. Mantenha scripts inline limitados ao bootstrap do Docusaurus.
4. **Runbooks, observabilidade e rollback**
   - `docs/portal/docs/devportal/observability.md` descreve os probes e dashboards que
     monitoram falhas de login, codigos de resposta do proxy e budgets de request.
   - `docs/portal/docs/devportal/incident-runbooks.md` cobre o caminho de escalacao
     se o sandbox for abusado; combine com
     `scripts/tryit-proxy-rollback.mjs` para virar endpoints com seguranca.

## Checklist de pen-test e release

Complete esta lista para cada promocao de preview (anexe resultados ao ticket de release):

1. **Verificar wiring OAuth**
   - Execute `npm run start` localmente com os exports `DOCS_OAUTH_*` de producao.
   - A partir de um perfil de browser limpo, abra a console Try it e confirme que o
     fluxo device-code emite um token, conta a duracao e limpa o campo apos expirar
     ou fazer sign-out.
2. **Provar o proxy**
   - `npm run tryit-proxy` contra Torii staging, depois execute
     `npm run probe:tryit-proxy` com o sample path configurado.
   - Verifique logs por entradas `authSource=override` e confirme que o rate limiting
     incrementa counters quando voce excede a janela.
3. **Confirmar CSP/Trusted Types**
   - `npm run build` e abra `build/index.html`. Garanta que a tag `<meta
     http-equiv="Content-Security-Policy">` corresponde as diretivas esperadas
     e que o DevTools nao mostra violacoes CSP ao carregar o preview.
   - Use `npm run probe:portal` (ou curl) para buscar o HTML deployado; a probe
     agora falha quando as meta tags `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` estao ausentes ou diferem dos valores declarados em
     `docusaurus.config.js`, assim reviewers de governanca podem confiar no
     exit code em vez de inspecionar o output do curl.
4. **Revisar observabilidade**
   - Verifique se o dashboard do proxy Try it esta verde (rate limits, error ratios,
     metricas de health probe).
   - Execute o drill de incidentes em `docs/portal/docs/devportal/incident-runbooks.md`
     se o host mudou (novo deployment Netlify/SoraFS).
5. **Documentar resultados**
   - Anexe screenshots/logs ao ticket de release.
   - Capture cada finding no template de relatorio de remediacao
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que owners, SLAs e evidencia de retest sejam faceis de auditar depois.
   - Linke de volta para este checklist para que o item do roadmap DOCS-1b continue auditable.

Se algum passo falhar, pare a promocao, abra uma issue bloqueante e anote o plano de remediacao em `status.md`.
