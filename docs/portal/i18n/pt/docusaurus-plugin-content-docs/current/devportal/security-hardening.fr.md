---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Duração segura e lista de verificação do pen-test

## Vista do conjunto

O item de roteiro **DOCS-1b** exige um código de dispositivo OAuth de login, políticas de segurança de conteúdo fortes
e testes de intrusão repetíveis antes que a visualização do portal possa girar em sites fora do laboratório.
Este apêndice explica o modelo de ameaça, os controles implementados no repositório e na lista de verificação de go-live
que les gate revisa o executor doivent.

- **Perímetro:** o proxy Try it, os painéis Swagger/RapiDoc embarques, e o console Try it custom
  rendimento par `docs/portal/src/components/TryItConsole.jsx`.
- **Fora do perímetro:** Torii lui-meme (couvert par les readiness reviews Torii) e a publicação SoraFS
  (couverte par DOCS-3/7).

## Modelo de ameaça

| Ativo | Risco | Mitigação |
| --- | --- | --- |
| Porta-jetons Torii | Vol ou reutilização fora dos documentos do sandbox | O código do dispositivo de login (`DOCS_OAUTH_*`) foi emitido, o proxy redigiu os cabeçalhos e o console expirará automaticamente os caches de credenciais. |
| Proxy Experimente | Abus como relais ouvert ou contorno dos limites Torii | `scripts/tryit-proxy*.mjs` impõe listas de permissões de origem, limitação de taxa, testes de segurança e encaminhamento explícito de `X-TryIt-Auth`; nenhuma credencial não persiste. |
| Tempo de execução do portal | Scripting entre sites ou incorporações maliciosas | `docusaurus.config.js` injeta os cabeçalhos Content-Security-Policy, Trusted Types e Permissions-Policy; Os scripts inline têm limites no tempo de execução Docusaurus. |
| Donnees d'observabilite | Telemetria manquante ou falsificação | `docs/portal/docs/devportal/observability.md` documenta as sondas/paineles; `scripts/portal-probe.mjs` é publicado em CI antes da publicação. |

Os adversários incluem usuários curiosos que consultam a visualização pública, atores mal-intencionados
testar des liens voles et des navigationurs comprometidos que tentam exfiltrar des credenciais armazenadas. Todos
Os controles devem funcionar nos padrões dos navegadores sem restrições de confiança.

## Controles necessários

1. **Login com código de dispositivo OAuth**
   - Configurador `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID`, e outros botões no ambiente de construção.
   - La carte Try it render un widget de sign-in (`OAuthDeviceLogin.jsx`) aqui
     recupere o código do dispositivo, interrogue o endpoint do token e apague automaticamente
     les jetons uma expiração. Os manuais de substituição do Bearer ainda estão disponíveis
     para um substituto de emergência.
   - Les builds ecoam maintenant si la config OAuth manque ou si les TTLs de
     fallback sortente da janela 300-900 s imposta por DOCS-1b; definir
     `DOCS_OAUTH_ALLOW_INSECURE=1` exclusivo para pré-visualizações de jatos locais.
2. **Garde-fous du proxy**
   - `scripts/tryit-proxy.mjs` aplicação de origens autorizadas, limites de taxa,
     des plafonds de taille de requete et des timeouts upstream tout en taguant le
     tráfego com `X-TryIt-Client` e uma edição dos jatos de logs.
   -`scripts/tryit-proxy-probe.mjs` mais `docs/portal/docs/devportal/observability.md`
     definindo a sonoridade da vivacidade e as regras do painel; executez-les
     lançamento anterior.
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` exporta manutenção de cabeçalhos de segurança determinados:
     `Content-Security-Policy` (default-src self, lista estritamente connect/img/script,
     exigências de tipos confiáveis), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - A lista Connect CSP whitelist aos endpoints OAuth device-code e token
     (HTTPS exclusivo salvo `DOCS_SECURITY_ALLOW_INSECURE=1`) para fazer login no dispositivo
     funciona sem relançar o sandbox para outras origens.
   - Os cabeçalhos são embarcados diretamente no gênero HTML, desde os hosts
     estatísticas não requerem configuração complementar. Guarde os scripts
     limites inline no bootstrap Docusaurus.
4. **Runbooks, observação e reversão**
   - `docs/portal/docs/devportal/observability.md` descreve os testes e painéis aqui
     monitore a verificação de login, os códigos de resposta do proxy e os orçamentos
     de requetes.
   - `docs/portal/docs/devportal/incident-runbooks.md` cobre a escalada
     se a sandbox for um abuso; combine-o com
     `scripts/tryit-proxy-rollback.mjs` para proteger os endpoints.

## Checklist de pen-test e lançamento

Complete esta lista para cada promoção de pré-visualização (junte os resultados ao ticket de lançamento):1. **Verificador da fiação OAuth**
   - Executar `npm run start` local com as exportações `DOCS_OAUTH_*` de produção.
   - De um perfil de navegador próprio, abra o console Try it e confirme que ele
     flux device-code emet un jeton, compte la duree de vie et efface le champ apres
     expiração ou saída.
2. **Sonder o proxy**
   - `npm run tryit-proxy` contra Torii teste, depois executor
     `npm run probe:tryit-proxy` com o caminho de amostra configurado.
   - Verificando os logs para `authSource=override` e confirmando a limitação de taxa
     aumenta os computadores quando a janela é desativada.
3. **Confirmar CSP/tipos confiáveis**
   - `npm run build` e depois abra `build/index.html`. Verificador da baliza `<meta
     http-equiv="Content-Security-Policy">` corresponde aos participantes das diretivas auxiliares
     e o DevTools não apresenta nenhuma violação do CSP durante o carregamento da visualização.
   - Utilize `npm run probe:portal` (ou curl) para recuperar a implantação HTML; a sonda
     echoue maintenant si les meta tags `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` manquent ou divergent des valeurs declares dans
     `docusaurus.config.js`, donc les reviewers gouvernance peuvent se fier au
     código de sortie em vez de lire la sortie curl.
4. **Revoir l'observabilité**
   - Verificador de que o painel Try it proxy est au vert (limites de taxa, taxas de erro,
     métricas de sonda de saúde).
   - Execute o exercício de incidente em `docs/portal/docs/devportal/incident-runbooks.md`
     se o host for alterado (nova implantação Netlify/SoraFS).
5. **Documente os resultados**
   - Joindre captura d'ecran/logs au ticket de release.
   - Capturador de todas as descobertas no modelo de relacionamento de remediação
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que os proprietários, SLAs e testes de reteste sejam fáceis para o auditor e mais tarde.
   - Confira esta nova lista de verificação para que o item do roteiro DOCS-1b seja auditável.

Se uma etapa ecoar, interrompa a promoção, abra um problema bloqueado e anote o plano de remediação
em `status.md`.