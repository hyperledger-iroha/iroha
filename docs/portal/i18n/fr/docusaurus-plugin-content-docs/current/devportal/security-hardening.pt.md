---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Durcissement de la sécurité et liste de contrôle du pen-test

## Visa général

L'élément de la feuille de route **DOCS-1b** nécessite une connexion au code de l'appareil OAuth, politiques fortes de sécurité du contenu
Les testicules de pénétration sont répétitifs avant le portail d'aperçu sur les réseaux du laboratoire. Este
annexe expliquant le modèle d'amis, les contrôles mis en œuvre dans le dépôt et une liste de contrôle de mise en production que
os gate examine devem executar.

- **Escopo :** ou proxy Essayez-le, paineis Swagger/RapiDoc est intégré à une console Essayez-le rendu personnalisé par
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fora do escopo:** Torii em si (coberto por reviews of readiness do Torii) et publicacao SoraFS
  (coût pour DOCS-3/7).

## Modèle d'amis| Ativo | Risco | Mitigacao |
| --- | --- | --- |
| Porteur de jetons do Torii | Roubo ou utiliser les forums sandbox de docs | Le code du périphérique de connexion (`DOCS_OAUTH_*`) émet des jetons de courte durée, les en-têtes de proxy modifiés et les informations d'identification de la console expirent automatiquement dans le cache. |
| Proxy Essayez-le | Abuso comme relais ouvert ou contournement des limites de Torii | `scripts/tryit-proxy*.mjs` listes autorisées d'origine, limitation de débit, sondes d'intégrité et transfert explicites de `X-TryIt-Auth` ; nenhuma credencial e persistida. |
| Runtime do portail | Cross-site scripting ou embarque des logiciels malveillants | `docusaurus.config.js` en-têtes injeta Politique de sécurité du contenu, Types de confiance et Politique d'autorisations ; les scripts en ligne sont limités au runtime par Docusaurus. |
| Données d'observation | Télémétrie ausente ou adultérée | Sondes/tableaux de bord documenta `docs/portal/docs/devportal/observability.md` ; `scripts/portal-probe.mjs` est placé dans CI avant de publier. |

Les adversaires comprennent des utilisateurs curieux vendant ou aperçu public, des auteurs malveillants testant des liens roubados et
les navigateurs comprometidos tentando extrair credenciais armazenadas. Tous les contrôles doivent les fonctionner
navigateurs communs sem redes confiaveis.

## Contrôles requis1. **Connexion par code de périphérique OAuth**
   - Configurer `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` et boutons liés à l'ambiance de construction.
   - O card Essayez-le pour rendre un widget de connexion (`OAuthDeviceLogin.jsx`) que
     rechercher le code de l'appareil, interroger automatiquement le point de terminaison du jeton et le nettoyer
     jetons quand ils expirent. Remplace les manuels de Bearer disponibles de façon permanente
     pour le repli d'urgence.
   - Os builds agora Falham quando a configuracao OAuth esta ausente ou quando os
     TTL de repli pour janvier 300-900 requis par DOCS-1b ; ajuster
     `DOCS_OAUTH_ALLOW_INSECURE=1` apena para previsualisations locales descartaveis.
2. **Les garde-corps font du proxy**
   - `scripts/tryit-proxy.mjs` application origines autorisées, limites de taux, plafonds de tamanho de request
     e timeouts en amont enquanto marca o trafego com `X-TryIt-Client` e
     supprimer les jetons dos journaux.
   - `scripts/tryit-proxy-probe.mjs` mais `docs/portal/docs/devportal/observability.md`
     définir une sonde de vivacité et des paramètres du tableau de bord ; exécuter-os antes de cada
     déploiement.
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` depuis l'exportation des en-têtes de sécurité déterminants :
     `Content-Security-Policy` (auto-src par défaut, liste des valeurs de connect/img/script,
     requis de Trusted Types), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - Une liste de connexion à la liste blanche CSP des points de terminaison OAuth du code de l'appareil et du jeton
     (quelque peu HTTPS pour moins que `DOCS_SECURITY_ALLOW_INSECURE=1`) pour la connexion de l'appareilFonctionne sans relâcher le bac à sable pour d'autres origines.
   - Les en-têtes sont intégrés directement dans le code HTML, puis les hôtes sont statiques avec précision
     de configuration supplémentaire. Les scripts Mantenha en ligne sont limités au bootstrap du Docusaurus.
4. **Runbooks, observabilité et restauration**
   - `docs/portal/docs/devportal/observability.md` décrit les sondes du système d'exploitation et les tableaux de bord que
     surveiller les erreurs de connexion, les codes de réponse du proxy et les budgets de demande.
   - `docs/portal/docs/devportal/incident-runbooks.md` connecteur du chemin d'escalier
     voir le bac à sable pour abusado; combiner com
     `scripts/tryit-proxy-rollback.mjs` pour sécuriser les points de terminaison.

## Checklist du pen-test et de la version

Complétez cette liste pour chaque promotion d'aperçu (voir les résultats du ticket de sortie) :1. **Vérifier le câblage OAuth**
   - Exécuter `npm run start` localement avec les exportations `DOCS_OAUTH_*` de production.
   - À partir d'un profil de navigateur vide, ouvrez une console Essayez-le et confirmez que
     Le code de l'appareil flux émet un jeton, contient la durée et nettoie le champ après l'expiration
     ou vous devez vous déconnecter.
2. **Provar ou proxy**
   - `npm run tryit-proxy` contre Torii staging, après exécution
     `npm run probe:tryit-proxy` avec l'exemple de chemin configuré.
   - Vérifier les journaux des entrées `authSource=override` et confirmer la limitation du débit
     incrémenta compteurs quando voce dépasser une janela.
3. **Confirmer les types CSP/de confiance**
   - `npm run build` et ouvre `build/index.html`. Garanta que une balise `` correspond aux diretivas esperadas
     et que les DevTools ne montrent pas les violations CSP pour télécharger l'aperçu.
   - Utilisez `npm run probe:portal` (ou curl) pour rechercher le déploiement HTML ; une sonde
     agora falha quando comme balises méta `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` estao ausentes ou diferem dos valeurs déclarées em
     `docusaurus.config.js`, les réviseurs de gouvernement peuvent confier non
     code de sortie avant d'inspecter la sortie de curl.
4. **Révision de l'observabilité**
   - Vérifiez le tableau de bord du proxy Essayez-le esta verde (limites de débit, taux d'erreur,
     métriques de sonde de santé).
   - Exécuter l'exercice de incidents em `docs/portal/docs/devportal/incident-runbooks.md`voir l'hôte mudou (nouveau déploiement Netlify/SoraFS).
5. **Résultats documentés**
   - Captures d'écran/journaux anexe et ticket de sortie.
   - Capturez chaque personne ne trouvant aucun modèle de relation de correction
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     pour que les propriétaires, les SLA et les preuves de retest soient faceis de auditar depois.
   - Lien de cette liste de contrôle pour que l'élément de la feuille de route DOCS-1b continue d'être auditable.

Si vous avez échoué, regardez la promotion, ouvrez un problème bloqué et notez le plan de résolution sur `status.md`.