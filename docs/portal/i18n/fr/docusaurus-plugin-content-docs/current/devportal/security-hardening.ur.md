---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سیکیورٹی ہارڈننگ اور pen-test چیک لسٹ

## جائزہ

روڈمیپ آئٹم **DOCS-1b** Connexion par code de périphérique OAuth, avec politiques de sécurité du contenu,
اور tests d'intrusion répétables پہلے کہ aperçu پورٹل لیب سے باہر
نیٹ ورکس پر چل سکے۔ Un modèle de menace, un dépôt et la mise en œuvre de contrôles de gestion et de mise en ligne
چیک لسٹ بیان کرتا ہے جسے gate reviews کو exécuter کرنا ہوتا ہے۔

- **اسکوپ :** Essayez-le proxy, panneaux Swagger/RapiDoc intégrés et console d'essai personnalisée.
  `docs/portal/src/components/TryItConsole.jsx` pour le rendu ہوتی ہے۔
- **آؤٹ اسکوپ:** Torii خود (Torii review review) et SoraFS publication
  (DOCS-3/7 ici).

## Modèle de menace| Actif | Risque | Atténuation |
| --- | --- | --- |
| Jetons au porteur Torii | docs bac à sable pour le vol et la réutilisation | connexion par code de périphérique (`DOCS_OAUTH_*`) jetons de courte durée menthe et en-têtes de proxy et rédaction des informations d'identification mises en cache par la console et expiration automatique |
| Essayez-le proxy | relais ouvert pour abus et limites de débit Torii pour contournement | Listes autorisées d'origine `scripts/tryit-proxy*.mjs`, limitation du débit, sondes d'intégrité, et `X-TryIt-Auth`, application du transfert explicite. Les informations d'identification persistent |
| Exécution du portail | cross-site scripting et intégrations malveillantes | `docusaurus.config.js` Content-Security-Policy, Trusted Types, et les en-têtes Permissions-Policy injectent des informations supplémentaires scripts en ligne Docusaurus runtime disponible en ligne |
| Données d'observabilité | télémétrie manquante ou falsification | Document des sondes/tableaux de bord `docs/portal/docs/devportal/observability.md` `scripts/portal-probe.mjs` publier سے پہلے CI میں چلتا ہے۔ |

Les adversaires sont en avant-première publique et les utilisateurs curieux, ainsi que les liens et les tests des acteurs malveillants.
Les navigateurs compromis ont été supprimés et les informations d'identification stockées ont été récupérées. تمام contrôles کو
réseaux de confiance pour les navigateurs de produits de base et pour les navigateurs de base

## Contrôles requis1. **Connexion par code de périphérique OAuth**
   - `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` pour les boutons et l'environnement de construction pour configurer les paramètres
   - Essayez-le avec le widget de connexion de la carte (`OAuthDeviceLogin.jsx`), rendu et récupération du code de l'appareil.
     point de terminaison du jeton pour le sondage et l'expiration des jetons pour l'effacement automatique Remplacements manuels du support
     secours d'urgence
   - Configuration OAuth manquante et TTL de secours DOCS-1b avec fenêtre de 300 à 900 s et échec des builds
     `DOCS_OAUTH_ALLOW_INSECURE=1` Ensemble d'aperçus locaux jetables
2. **Garde-corps proxy**
   - Origines autorisées `scripts/tryit-proxy.mjs`, limites de débit, limites de taille de demande et délais d'attente en amont appliqués.
     `X-TryIt-Client` contient une balise de trafic et des journaux et des jetons rédigés.
   - Sonde de vivacité `scripts/tryit-proxy-probe.mjs` et `docs/portal/docs/devportal/observability.md`
     Les règles du tableau de bord définissent les paramètres ہر déploiement سے پہلے انہیں چلائیں۔
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` pour l'exportation d'en-têtes de sécurité déterministes:
     `Content-Security-Policy` (auto-src par défaut, listes de connexion/img/script, exigences de types de confiance)
     `Permissions-Policy`, et `Referrer-Policy: no-referrer`۔
   - Liste de connexion CSP Code de périphérique OAuth et points de terminaison de jeton et liste blanche
     (Il s'agit du protocole HTTPS `DOCS_SECURITY_ALLOW_INSECURE=1`) Pour ouvrir la connexion à l'appareil
     اور دوسرے origines کے لئے sandbox relax نہ ہو۔- en-têtes générés HTML et intégration pour les hôtes statiques et la configuration et les paramètres de configuration
     scripts en ligne et bootstrap Docusaurus pour le démarrage
4. **Runbooks, observabilité, retour en arrière**
   - Sondes `docs/portal/docs/devportal/observability.md` et tableaux de bord pour les échecs de connexion, les codes de réponse proxy,
     اور demander des budgets پر نظر رکھتے ہیں۔
   - Chemin d'escalade `docs/portal/docs/devportal/incident-runbooks.md` pour abus de sandbox
     `scripts/tryit-proxy-rollback.mjs` combine plusieurs points de terminaison avec un commutateur et un commutateur

## Pen-test et liste de contrôle de publication

La promotion en avant-première et le ticket de sortie sont joints à la pièce jointe :1. **Vérification du câblage OAuth ici**
   - `npm run start` production `DOCS_OAUTH_*` exportations locales
   - Voir le profil du navigateur et essayer la console et confirmer le jeton de flux de code de l'appareil menthe.
     compte à rebours à vie et expiration et déconnexion et champ clair
2. ** Sonde proxy ici **
   - `npm run tryit-proxy` et mise en scène Torii pour la mise en scène
     Chemin d'échantillon configuré `npm run probe:tryit-proxy`
   - les journaux d'entrées `authSource=override` et confirment que la fenêtre de limitation de débit dépasse 90 %
     les compteurs augmentent ہوتے ہیں۔
3. **CSP/Trusted Types confirment ici**
   - `npm run build` pour `build/index.html` pour `build/index.html` یقینی بنائیں کہ `` les directives attendues par la balise correspondent à celles-ci.
     Ajouter un aperçu du chargement à DevTools en cas de violations CSP
   - `npm run probe:portal` (curl) pour la récupération HTML déployée la sonde échoue ہو جاتا ہے اگر
     `Content-Security-Policy`, `Permissions-Policy`, et `Referrer-Policy` balises méta ici et là
     `docusaurus.config.js` Valeurs déclarées et code de sortie des réviseurs de gouvernance
     La sortie curl est terminée.
4. **Revue d'observabilité ici**
   - Essayez-le tableau de bord proxy سبز ہو (limites de débit, taux d'erreur, mesures de la sonde de santé)۔
   - L'hôte est en place (déploiement Netlify/SoraFS) et `docs/portal/docs/devportal/incident-runbooks.md`
     Exercice d'incident5. **Document نتائج کریں**
   - captures d'écran/journaux et ticket de sortie et pièces jointes
   - Trouver un modèle de rapport de remédiation et capturer des fichiers
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Les propriétaires, les SLA, et les preuves de nouveau test pour l'audit et l'audit
   - Liste de contrôle et lien vers l'élément de feuille de route DOCS-1b vérifiable ici

L'échec de la promotion est lié au problème de blocage et au plan de remédiation `status.md`.