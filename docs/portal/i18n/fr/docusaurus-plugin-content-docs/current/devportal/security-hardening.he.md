---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 90e66415f50946b40f80099175b840428ed636c493e2e33ecba7636197012f6a
source_last_modified: "2026-01-03T18:07:58+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Durcissement securite et checklist de pen-test

## Vue d'ensemble

L'item de roadmap **DOCS-1b** exige un login OAuth device-code, des politiques de securite de contenu fortes
et des tests d'intrusion repetables avant que le portail preview puisse tourner sur des reseaux hors labo.
Cet appendice explique le modele de menace, les controles implementes dans le repo et la checklist de go-live
que les gate reviews doivent executer.

- **Perimetre:** le proxy Try it, les panneaux Swagger/RapiDoc embarques, et la console Try it custom
  rendue par `docs/portal/src/components/TryItConsole.jsx`.
- **Hors perimetre:** Torii lui-meme (couvert par les readiness reviews Torii) et la publication SoraFS
  (couverte par DOCS-3/7).

## Modele de menace

| Actif | Risque | Mitigation |
| --- | --- | --- |
| Jetons bearer Torii | Vol ou reutilisation hors du sandbox docs | Le login device-code (`DOCS_OAUTH_*`) emet des jetons courts, le proxy redacte les headers et la console expire automatiquement les credentials caches. |
| Proxy Try it | Abus comme relais ouvert ou contournement des limites Torii | `scripts/tryit-proxy*.mjs` impose des allowlists d'origine, du rate limiting, des probes de sante et le forwarding explicite de `X-TryIt-Auth`; aucune credential n'est persistee. |
| Runtime du portail | Cross-site scripting ou embeds malveillants | `docusaurus.config.js` injecte les headers Content-Security-Policy, Trusted Types et Permissions-Policy; les scripts inline sont limites au runtime Docusaurus. |
| Donnees d'observabilite | Telemetrie manquante ou falsification | `docs/portal/docs/devportal/observability.md` documente les probes/dashboards; `scripts/portal-probe.mjs` tourne en CI avant publication. |

Les adversaires incluent des utilisateurs curieux qui consultent le preview public, des acteurs malveillants
testant des liens voles et des navigateurs compromis qui tentent d'exfiltrer des credentials stockes. Tous
les controles doivent fonctionner sur des navigateurs standards sans reseaux de confiance.

## Controles requis

1. **OAuth device-code login**
   - Configurer `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID`, et les autres knobs dans l'environnement de build.
   - La carte Try it rend un widget de sign-in (`OAuthDeviceLogin.jsx`) qui
     recupere le device code, interroge le token endpoint et efface automatiquement
     les jetons a expiration. Les overrides manuels de Bearer restent disponibles
     pour un fallback d'urgence.
   - Les builds echouent maintenant si la config OAuth manque ou si les TTLs de
     fallback sortent de la fenetre 300-900 s imposee par DOCS-1b; definir
     `DOCS_OAUTH_ALLOW_INSECURE=1` uniquement pour des previews locaux jetables.
2. **Garde-fous du proxy**
   - `scripts/tryit-proxy.mjs` applique des origins autorisees, des rate limits,
     des plafonds de taille de requete et des timeouts upstream tout en taguant le
     trafic avec `X-TryIt-Client` et en redactant les jetons des logs.
   - `scripts/tryit-proxy-probe.mjs` plus `docs/portal/docs/devportal/observability.md`
     definissent la sonde de liveness et les regles de dashboard; executez-les
     avant chaque rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` exporte maintenant des headers de securite deterministes:
     `Content-Security-Policy` (default-src self, listes strictes connect/img/script,
     exigences Trusted Types), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - La liste connect CSP whitelist les endpoints OAuth device-code et token
     (HTTPS uniquement sauf `DOCS_SECURITY_ALLOW_INSECURE=1`) afin que le device login
     fonctionne sans relacher le sandbox pour d'autres origins.
   - Les headers sont embarques directement dans le HTML genere, donc les hosts
     statiques n'ont pas besoin de configuration supplementaire. Garder les scripts
     inline limites au bootstrap Docusaurus.
4. **Runbooks, observabilite et rollback**
   - `docs/portal/docs/devportal/observability.md` decrit les probes et dashboards qui
     surveillent les echec de login, les codes de reponse du proxy et les budgets
     de requetes.
   - `docs/portal/docs/devportal/incident-runbooks.md` couvre l'escalation
     si le sandbox est abuse; combinez-le avec
     `scripts/tryit-proxy-rollback.mjs` pour basculer les endpoints en securite.

## Checklist de pen-test et release

Completer cette liste pour chaque promotion de preview (joindre les resultats au ticket de release):

1. **Verifier le wiring OAuth**
   - Executer `npm run start` localement avec les exports `DOCS_OAUTH_*` de production.
   - Depuis un profil de navigateur propre, ouvrir la console Try it et confirmer que le
     flux device-code emet un jeton, compte la duree de vie et efface le champ apres
     expiration ou sign-out.
2. **Sonder le proxy**
   - `npm run tryit-proxy` contre Torii staging, puis executer
     `npm run probe:tryit-proxy` avec le sample path configure.
   - Verifier les logs pour `authSource=override` et confirmer que le rate limiting
     incremente les compteurs quand la fenetre est depassee.
3. **Confirmer CSP/Trusted Types**
   - `npm run build` puis ouvrir `build/index.html`. Verifier que la balise `<meta
     http-equiv="Content-Security-Policy">` correspond aux directives attendues
     et que DevTools ne montre aucune violation CSP lors du chargement du preview.
   - Utiliser `npm run probe:portal` (ou curl) pour recuperer le HTML deploie; la sonde
     echoue maintenant si les meta tags `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` manquent ou divergent des valeurs declarees dans
     `docusaurus.config.js`, donc les reviewers gouvernance peuvent se fier au
     code de sortie au lieu de lire la sortie curl.
4. **Revoir l'observabilite**
   - Verifier que le dashboard Try it proxy est au vert (rate limits, ratios d'erreur,
     metriques de health probe).
   - Executer le drill d'incident dans `docs/portal/docs/devportal/incident-runbooks.md`
     si l'host a change (nouveau deploiement Netlify/SoraFS).
5. **Documenter les resultats**
   - Joindre captures d'ecran/logs au ticket de release.
   - Capturer chaque finding dans le template de rapport de remediation
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     afin que owners, SLAs et preuves de retest soient faciles a auditer plus tard.
   - Lier a nouveau cette checklist pour que l'item de roadmap DOCS-1b reste auditable.

Si une etape echoue, stopper la promotion, ouvrir une issue bloquante et noter le plan de remediation
dans `status.md`.
