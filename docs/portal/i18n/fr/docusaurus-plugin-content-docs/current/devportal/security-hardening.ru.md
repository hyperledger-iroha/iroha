---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Усиление безопасности и чеклист пен-testа

## Обзор

Les cartes **DOCS-1b** nécessitent une connexion par code de périphérique OAuth et des politiques de sécurité
Contenu gratuit et tests d'intrusion possibles avant le portail d'aperçu du logiciel
работать в сетях вне LABоратории. Ceci propose une description du modèle d'entreprise, réalisable
Dans les contrôles des dépôts et les contrôles de mise en production, vous pouvez facilement consulter les critiques des portes.

- **Dans les formats :** selon Essayez-le, dans les panneaux Swagger/RapiDoc et dans le cadre du projet
  консоль Essayez-le, рендеримая `docs/portal/src/components/TryItConsole.jsx`.
- **Вне рамок:** сам Torii (pour les évaluations de préparation Torii) et la publication SoraFS
  (покрывается DOCS-3/7).

## Modèle угроз| Actifs | Risque | Mitigation |
| --- | --- | --- |
| Torii porteur porte-monnaie | L'utilisation ou l'utilisation officielle de docs sandbox | La connexion au code de l'appareil (`DOCS_OAUTH_*`) permet d'accéder aux jetons de sécurité, de supprimer les en-têtes et de créer automatiquement des crédits. |
| Essayez-le immédiatement | Utilisation du relais ou des limites Torii | Listes autorisées d'origine appliquées par `scripts/tryit-proxy*.mjs`, limitation du débit, sondes d'intégrité et transfert `X-TryIt-Auth` ; креды не сохраняются. |
| Portail d'exécution | Cross-site scripting ou intégrations complètes | `docusaurus.config.js` inclut les en-têtes Content-Security-Policy, Trusted Types et Permissions-Policy ; scripts en ligne ограничены runtime Docusaurus. |
| Данные наблюдаемости | Télémétrie et transmissions | `docs/portal/docs/devportal/observability.md` documente les sondes/tableaux de bord ; `scripts/portal-probe.mjs` a été publié par CI avant la publication. |

Противники включают любопытных пользователей публичного preview, злоумышленников, тестирующих украденные ссылки,
et des soutiens-gorge complémentaires, vous pouvez utiliser des crédits personnels. Tous les contrôles doivent fonctionner
в обычных браузерах без доверенных сетей.

## Contrôles des tâches1. **Connexion par code de périphérique OAuth**
   - Настройте `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` et boutons de construction dans les locaux de construction.
   - Carte Essayez-le, affichez le widget de connexion (`OAuthDeviceLogin.jsx`), ici
     Utiliser le code de l'appareil, utiliser le point de terminaison du jeton et utiliser automatiquement les jetons
     après l'histoire. Les remplacements de Bearer sont prévus pour le repli d'exécution.
   - Essayez de le faire, si vous ouvrez la configuration OAuth ou si vous utilisez des TTL de secours
     выходят за окно 300-900 s, avant DOCS-1b ; установите
     `DOCS_OAUTH_ALLOW_INSECURE=1` permet un aperçu local des détails.
2. **Garde-corps proxy**
   - `scripts/tryit-proxy.mjs` indique les origines autorisées, les limites de taux et les plafonds pour déterminer les limites de taux.
     et les délais d'attente en amont, permettant d'améliorer le trafic `X-TryIt-Client` et de supprimer les jetons
     dans les journaux.
   - `scripts/tryit-proxy-probe.mjs` et `docs/portal/docs/devportal/observability.md`
     определяют vivacité sonde et правила tableau de bord ; запускайте их перед каждым déploiement.
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` permet d'exporter des en-têtes de sécurité :
     `Content-Security-Policy` (auto-src par défaut, строгие списки connect/img/script,
     требования Types de confiance), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - Liste blanche de connexion CSP avec code de périphérique OAuth et points de terminaison de jeton
     (jusqu'à HTTPS, si vous n'êtes pas connecté à `DOCS_SECURITY_ALLOW_INSECURE=1`), pour la connexion à l'appareil
     работал без ослабления sandbox для других origins.- Ces en-têtes sont créés pour le HTML générique, comme hôte statique
     не требуют дополнительной конфигурации. Ajouter des scripts en ligne gratuitement
     Amorçage Docusaurus.
4. **Runbooks, observabilité et restauration**
   - `docs/portal/docs/devportal/observability.md` analyse les sondes et les tableaux de bord
     Cela concerne les échecs de connexion, les codes de réponse proxy et les budgets de demande.
   - `docs/portal/docs/devportal/incident-runbooks.md` permet d'obtenir une escalade
     при злоупотреблении bac à sable; combiner avec
     `scripts/tryit-proxy-rollback.mjs` pour la sécurité des points de terminaison.

## Чеклист пен-теста и релиза

Sélectionnez cet article pour chaque aperçu de la production (en ajoutant les résultats du ticket de sortie) :1. **Provérifier le câblage OAuth**
   - Запустите `npm run start` localement pour la production `DOCS_OAUTH_*` exportations.
   - Si le profil du profil est ouvert, essayez-le sur la console et découvrez le flux de code de l'appareil
     Vous pouvez utiliser le jeton, le connecter à votre compte et vous connecter après l'installation ou la déconnexion.
2. **Probiter le processus**
   - `npm run tryit-proxy` pour le staging Torii, veuillez le faire
     `npm run probe:tryit-proxy` avec le chemin d'échantillon disponible.
   - Vérifiez les journaux du `authSource=override` et vérifiez la limitation du débit.
     увеличивает счетчики при превышении окна.
3. **Modifier CSP/Types de confiance**
   - `npm run build` et ouvrez `build/index.html`. Убедитесь, что тег `` répondez directement à vos questions
     Et DevTools ne détecte pas les violations CSP lors de l'aperçu.
   - Utilisez `npm run probe:portal` (ou curl), pour utiliser le code HTML ; sonde
     Veuillez utiliser les balises méta `Content-Security-Policy`, `Permissions-Policy` ou
     `Referrer-Policy` s'ouvre ou s'ouvre sur la porte
     `docusaurus.config.js`, car la gouvernance des réviseurs peut être mise à jour
     code de sortie вместо ручной проверки curl sortie.
4. **Provérifier l'observabilité**
   - Ajoutez ce tableau de bord Essayez-le proxy ici (limites de débit, taux d'erreur,
     mesures de la sonde de santé).
   - Effectuez un exercice d'incident selon `docs/portal/docs/devportal/incident-runbooks.md`,
     Si votre hébergeur est disponible (nouvelle version de Netlify/SoraFS).5. **Документировать результаты**
   - Afficher les captures d'écran/journaux et le ticket de version.
   - Inscrivez-vous dans le rapport de remédiation
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Les propriétaires, les SLA et les preuves pour retester peuvent facilement être entendus.
   - Si vous êtes à ce moment-là, vous devez vérifier que les cartes DOCS-1b sont vérifiables.

Si vous le prouvez, assurez-vous de résoudre le problème de blocage et de planifier la résolution dans `status.md`.