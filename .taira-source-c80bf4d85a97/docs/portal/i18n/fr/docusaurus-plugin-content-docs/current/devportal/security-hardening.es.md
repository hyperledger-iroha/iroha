---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Durcissement de la sécurité et liste de contrôle du pen-test

## CV

L'élément de la feuille de route **DOCS-1b** nécessite une connexion OAuth avec le code de l'appareil, les politiques de sécurité du contenu sont fortes et
Pen tests répétitifs avant que le portail de prévisualisation puisse être exécuté en rouge à l'extérieur du laboratoire. Cette annexe
Expliquer le modèle de mesures, les contrôles mis en œuvre dans le repo et la liste de contrôle de go-live qui doivent être exécutés
las revisiones de gate.

- **Utilisation :** le proxy de Try it, les panneaux Swagger/RapiDoc intégrés et la console Try it custom renderizada por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fuera de alcance:** Torii en si (cubierto por reviews de readiness de Torii) et la publication de SoraFS
  (couvert par DOCS-3/7).

## Modèle d'amendes| Actif | Riesgo | Atténuation |
| --- | --- | --- |
| Porteur de jetons de Torii | Robo ou réutiliser hors du bac à sable de documents | Le code du périphérique de connexion (`DOCS_OAUTH_*`) émet des jetons de vie, les en-têtes de rédaction du proxy et la console expirent automatiquement les informations d'identification mises en cache. |
| Proxy de Essayez-le | Abus comme relais ouvert ou contournement des limites de Torii | `scripts/tryit-proxy*.mjs` listes autorisées d'origine, limitation de débit, sondes d'intégrité et transfert explicites de `X-TryIt-Auth` ; no se persiste credenciales. |
| Runtime du portail | Cross-site scripting o embarque des logiciels malveillants | `docusaurus.config.js` en-têtes inyecta Content-Security-Policy, Trusted Types et Permissions-Policy ; Les scripts en ligne limitent le temps d'exécution de Docusaurus. |
| Données d'observabilité | Télémétrie erronée ou manipulation | `docs/portal/docs/devportal/observability.md` documente les sondes/tableaux de bord ; `scripts/portal-probe.mjs` correspond à CI avant de publier. |

Les adversaires incluent des utilisateurs curieux vivant un aperçu public, des acteurs malveillants proposant des liens volés et
les navigateurs comprometidos qui tentent d'obtenir des informations d'identification supplémentaires. Tous les contrôles doivent fonctionner
navigateurs d'utilisation commune sans redes confiables.

## Contrôles requis1. **Connexion par code de périphérique OAuth**
   - Configurer `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` et boutons liés à l'entorno de build.
   - La tarjeta Try it rend un widget de connexion (`OAuthDeviceLogin.jsx`) que
     obtenir le code de l'appareil, interroger le point de terminaison du jeton et nettoyer automatiquement les jetons
     una vez que expiran. Les manuels de remplacement de Bearer sont disponibles pour
     repli d'urgence.
   - Les builds tombent maintenant lorsque la configuration OAuth échoue ou lorsque les TTL de
     fallback se salen de la ventana 300-900 s exigida por DOCS-1b ; ajuster
     `DOCS_OAUTH_ALLOW_INSECURE=1` seul pour les aperçus locaux descartables.
2. **Garde-corps du proxy**
   - `scripts/tryit-proxy.mjs` application autorisée origines, limites de taux, plafonds de tamanio de request
     y délais d'attente en amont pendant l'étiquette du trafic avec `X-TryIt-Client` et rédaction de jetons
     de los logs.
   - `scripts/tryit-proxy-probe.mjs` et `docs/portal/docs/devportal/observability.md`
     définir la sonde de vivacité et les paramètres du tableau de bord ; ejecutalos avant chaque jour
     déploiement.
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` maintenant exporter les en-têtes de sécurité déterministes :
     `Content-Security-Policy` (auto-src par défaut, listes restreintes de connect/img/script,
     conditions requises pour les types de confiance), `Permissions-Policy` et
     `Referrer-Policy: no-referrer`.
   - La liste de connexion du CSP autorise les points de terminaison OAuth, code de périphérique et jeton(seulement HTTPS à moins que `DOCS_SECURITY_ALLOW_INSECURE=1`) pour la connexion de l'appareil
     fonction sans relâcher le bac à sable pour d'autres origines.
   - Les en-têtes sont directement insérés dans le HTML généré, par les hôtes.
     estaticos aucune configuration nécessaire supplémentaire. Garder les scripts en ligne
     limité au bootstrap de Docusaurus.
4. **Runbooks, observabilité et restauration**
   - `docs/portal/docs/devportal/observability.md` décrit les sondes et les tableaux de bord que
     veille aux erreurs de connexion, aux codes de réponse du proxy et aux budgets de demandes.
   - `docs/portal/docs/devportal/incident-runbooks.md` pour la route d'escalade
     si le bac à sable est abusé ; combiner avec
     `scripts/tryit-proxy-rollback.mjs` pour modifier les points de terminaison de manière sécurisée.

## Checklist de pen-test et release

Cette liste complète pour chaque promotion d'aperçu (ajoutée aux résultats du ticket de sortie) :1. **Vérifier le câblage OAuth**
   - Exécuter `npm run start` localement avec les exportations `DOCS_OAUTH_*` de production.
   - Depuis un profil de navigateur propre, ouvrez la console Essayez-le et confirmez que le flux
     Le code de l'appareil émet un jeton, compte tenu de la durée de vie et nettoie le champ
     après l'expiration ou la fin de la session.
2. **Probar el proxy**
   - `npm run tryit-proxy` contre Torii staging, luego ejecuta
     `npm run probe:tryit-proxy` avec le chemin d'échantillonnage configuré.
   - Réviser les journaux pour les entrées `authSource=override` et confirmer la limitation du débit
     incrémenta compteurs cuando dépasse la ventana.
3. **Confirmer les types CSP/de confiance**
   - `npm run build` et ouvrir `build/index.html`. Assurez-vous que la balise `` coïncide avec les directives attendues
     et que DevTools ne doit pas violer CSP pour charger l'aperçu.
   - Utilisez `npm run probe:portal` (ou curl) pour obtenir le HTML téléchargé ; sonde électrique
     maintenant, lorsque les balises méta `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` faltan o difieren des valeurs déclarées en
     `docusaurus.config.js`, car les réviseurs d'État peuvent confier le
     code de sortie à la place de l'inspection de la sortie de curl.
4. **Réviser l'observabilité**
   - Vérifiez que le tableau de bord de Try it proxy est vert (limites de débit, taux d'erreur,
     métriques de sonde de santé).- Exécuter l'exercice d'incidents en `docs/portal/docs/devportal/incident-runbooks.md`
     si vous changez d'hôte (nouvelle version Netlify/SoraFS).
5. **Documenter les résultats**
   - Captures d'écran/journaux supplémentaires au ticket de sortie.
   - Capturez chaque hallazgo sur la plante du rapport de remédiation
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     pour que les propriétaires, les SLA et les preuves de retest soient faciles à auditer après.
   - Ajouter cette liste de contrôle pour que l'élément de la feuille de route DOCS-1b soit auditable.

Si quelque chose n'arrive pas, déterminez la promotion, ouvrez un problème bloquant et notez le plan de résolution en `status.md`.