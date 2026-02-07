---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Durcissement sécurité et checklist de pen-test

## Vue d'ensemble

L'élément de feuille de route **DOCS-1b** exige un code de périphérique OAuth de connexion, des politiques de sécurité de contenu fortes
et des tests d'intrusion répétables avant que le portail aperçu puisse tourner sur des réseaux hors laboratoire.
Cette annexe explique le modèle de menace, les contrôles mis en œuvre dans le repo et la checklist de go-live
que les revues de porte doivent être exécutées.

- **Périmètre :** le proxy Try it, les panneaux Swagger/RapiDoc embarques, et la console Try it custom
  rendue par `docs/portal/src/components/TryItConsole.jsx`.
- **Hors périmètre :** Torii lui-meme (couvert par les readiness reviews Torii) et la publication SoraFS
  (couverte par DOCS-3/7).

## Modèle de menace| Actif | Risques | Atténuation |
| --- | --- | --- |
| Porteur de Jetons Torii | Vol ou réutilisation hors du sandbox docs | Le code de périphérique de connexion (`DOCS_OAUTH_*`) emet des jetons courts, le proxy rédige les en-têtes et la console expire automatiquement les caches d'informations d'identification. |
| Proxy Essayez-le | Abus comme relais ouvert ou contournement des limites Torii | `scripts/tryit-proxy*.mjs` impose des listes d'autorisation d'origine, du rate limitation, des sondes de santé et le forwarding explicite de `X-TryIt-Auth` ; Aucun identifiant n'est persisté. |
| Runtime du portail | Cross-site scripting ou intégration de malwares | `docusaurus.config.js` injecte les en-têtes Content-Security-Policy, Trusted Types et Permissions-Policy ; les scripts inline sont limités au runtime Docusaurus. |
| Données d'observabilité | Télémétrie manquante ou falsification | `docs/portal/docs/devportal/observability.md` documente les sondes/tableaux de bord ; `scripts/portal-probe.mjs` tourne en CI avant publication. |

Les adversaires utilisent des utilisateurs curieux qui consultent l'aperçu public, des acteurs malveillants
testant des liens voles et des navigateurs compromis qui tentent d'exfiltrer les identifiants stockes. Tous
les contrôles doivent fonctionner sur des navigateurs standards sans réseaux de confiance.

## Contrôles requis1. **Connexion par code de périphérique OAuth**
   - Configurateur `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID`, et les autres boutons dans l'environnement de build.
   - La carte Try it rend un widget de connexion (`OAuthDeviceLogin.jsx`) ici
     récupérer le code de l'appareil, interroger le point de terminaison du jeton et l'effacer automatiquement
     les jetons à expiration. Les manuels de remplacement de Bearer restent disponibles
     pour un repli d'urgence.
   - Les builds échouent maintenant si la config OAuth manque ou si les TTLs de
     fallback sortent de la fenêtre 300-900 s imposé par DOCS-1b; définir
     `DOCS_OAUTH_ALLOW_INSECURE=1` uniquement pour des avant-premières locales jetables.
2. **Garde-fous du proxy**
   - `scripts/tryit-proxy.mjs` applique des origines autorisées, des limites tarifaires,
     des plafonds de taille de requête et des timeouts en amont tout en taguant le
     trafic avec `X-TryIt-Client` et en rédactant les jetons des logs.
   - `scripts/tryit-proxy-probe.mjs` plus `docs/portal/docs/devportal/observability.md`
     définir la sonde de vivacité et les règles de tableau de bord ; exécutez-les
     avant chaque déploiement.
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` exporte maintenant des headers de securite déterministes :
     `Content-Security-Policy` (default-src self, listes strictes connect/img/script,
     exigences Trusted Types), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - La liste connect CSP whitelist les endpoints OAuth device-code et token(HTTPS uniquement sauf `DOCS_SECURITY_ALLOW_INSECURE=1`) afin que le périphérique se connecte
     fonctionne sans relâcher le sandbox pour d'autres origines.
   - Les headers sont embarqués directement dans le genre HTML, donc les hosts
     Les statiques n’ont pas besoin de configuration supplémentaire. Garder les scripts
     limites en ligne au bootstrap Docusaurus.
4. **Runbooks, observabilité et rollback**
   - `docs/portal/docs/devportal/observability.md` décrit les sondes et tableaux de bord qui
     surveillez les échecs de connexion, les codes de réponse du proxy et les budgets
     de requêtes.
   - `docs/portal/docs/devportal/incident-runbooks.md` couvre l'escalade
     si le bac à sable est abusé; combinez-le avec
     `scripts/tryit-proxy-rollback.mjs` pour basculer les points finaux en sécurité.

## Checklist de pen-test et release

Complétez cette liste pour chaque promotion de preview (joindre les résultats au ticket de release) :1. **Vérifier le câblage OAuth**
   - Executer `npm run start` localement avec les exports `DOCS_OAUTH_*` de production.
   - Depuis un profil de navigateur propre, ouvrir la console Try it et confirmer que le
     flux device-code emet un jeton, compte la durée de vie et efface le champ après
     expiration ou déconnexion.
2. **Sonder le proxy**
   - `npm run tryit-proxy` contre Torii staging, puis exécuteur
     `npm run probe:tryit-proxy` avec le chemin d'exemple configuré.
   - Vérifier les logs pour `authSource=override` et confirmer que le rate limiting
     incrémente les compteurs quand la fenêtre est passée.
3. **Confirmateur CSP/Types de confiance**
   - `npm run build` puis ouvrir `build/index.html`. Vérifier que la balise `` correspond aux directives attendues
     et que DevTools ne montre aucune violation CSP lors du chargement de l'aperçu.
   - Utiliser `npm run probe:portal` (ou curl) pour récupérer le déploiement HTML ; la sonde
     echoue maintenant si les balises méta `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` manquent ou divergent des valeurs déclarées dans
     `docusaurus.config.js`, donc les reviewers gouvernance peuvent se fier au
     code de sortie au lieu de lire la sortie curl.
4. **Revoir l'observabilité**
   - Vérifier que le tableau de bord Try it proxy est au vert (limites de taux, ratios d'erreur,
     métriques de sonde de santé).- Executer le drill d'incident dans `docs/portal/docs/devportal/incident-runbooks.md`
     si l'hébergeur a changé (nouveau déploiement Netlify/SoraFS).
5. **Documenter les résultats**
   - Joindre capture d'écran/logs au ticket de release.
   - Capturer chaque constatation dans le modèle de rapport de remédiation
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     afin que les propriétaires, les SLA et les preuves de retest soient faciles à un auditeur plus tard.
   - Lier a nouvelle cette checklist pour que l'élément de la feuille de route DOCS-1b reste auditable.

Si une étape fait écho, stoppez la promotion, ouvrez une issue bloquante et notez le plan de remédiation
dans `status.md`.