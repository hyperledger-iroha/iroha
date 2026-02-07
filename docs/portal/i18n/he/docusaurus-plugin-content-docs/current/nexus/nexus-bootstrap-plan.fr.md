---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-bootstrap-plan
כותרת: Bootstrap et observabilite Sora Nexus
תיאור: תכננו את התפעול של ה-Cluster Central de Validateurs Nexus Avant d'ajouter les Services SoraFS et SoraNet.
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/soranexus_bootstrap_plan.md`. Gardez les deux copies alignees jusqu'a ce que les versions localisees arrivent sur le portal.
:::

# Plan de bootstrap et d'observabilite Sora Nexus

## אובייקטים
- Mettre en place le reseau de base validateurs/observateurs Sora Nexus avec cles de governance, APIs Torii ומעקב קונצנזוס.
- Valider les services coeur (Torii, קונצנזוס, התמדה) avant d'activer les deploiements piggyback SoraFS/SoraNet.
- Etablir des workflows CI/CD et des dashboards/alertes d'observabilite pour assurer la sante du reseau.

## תנאי מוקדם
- Materiel de cles de governance (multisig du conseil, cles de comite) זמין ב-HSM ou Vault.
- תשתית דה בסיס (אשכולות Kubernetes ou noeuds bare-metal) dans les regions primaire/secondaire.
- רצועת אתחול תצורה mise a jour (`configs/nexus/bootstrap/*.toml`) משפרת את הפרמטרים של קונצנזוס.

## מרכז סביבה
- Operator deux environnements Nexus עם קידומות רזולוציה מבדילים:
- **Sora Nexus (mainnet)** - קידומת reseau de production `nexus`, hebergeant la governance canonique et les services piggyback SoraFS/SoraNet (מזהה שרשרת Grafana Grafana /Grafana).
- **Sora Testus (testnet)** - קידומת reseau de staging `testus`, miroir de la configuration mainnet pour les tests d'integration et la validation pre-release (שרשרת UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Maintenir des fichiers genesis, des cles de governance et des empreintes d'infrastructure מפרידים את סביבת צ'אקה. Testus sert de terrain de preuve pour tous les rollouts SoraFS/SoraNet avant promotion vers Nexus.
- Les pipelines CI/CD doivent deployer d'abord sur Testus, executer des tests עשן אוטומציה, ודרשו קידום מכירות מנואלי לעומת Nexus une fois les checks עובר.
- Les bundles de configuration de reference se trouvent sous `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet), chacun contenant un exemple `config.toml`, `genesis.json` et desmission000X I1000x repertoirs d'1000X.

## Etape 1 - Revue de configuration
1. Auditer la documentation existante:
   - `docs/source/nexus/architecture.md` (קונצנזוס, פריסה Torii).
   - `docs/source/nexus/deployment_checklist.md` (דרישות אינפרא).
   - `docs/source/nexus/governance_keys.md` (נהלים דה גארד דה קלאס).
2. Valider que les fichiers genesis (`configs/nexus/genesis/*.json`) s'alignent avec le roster actuel des validateurs et les poids de staking.
3. Confirmer les parameters reseau:
   - Taille du comite de consensus et quorum.
   - Intervalle de bloc / seuils de finalite.
   - יציאות שירות Torii ואישורי TLS.## Etape 2 - Deploiement du cluster bootstrap
1. Provisionner les noeuds validateurs:
   - Deployer des instances `irohad` (validateurs) avec volumes persistants.
   - מוודא que les regles de firewall autorised to trafic consensus & Torii entre nouds.
2. Demarrer les services Torii (REST/WebSocket) sur chaque validateur avec TLS.
3. Deployer des noeuds observateurs (lecture seule) pour une resilience additionnelle.
4. מנהל סקריפטים bootstrap (`scripts/nexus_bootstrap.sh`) יורד את תחילתו של מפיץ, מגדיר את הקונצנזוס ואת רשום הקודמים.
5. מבצע בדיקות עשן:
   - Soumettre des transactions de test דרך Torii (`iroha_cli tx submit`).
   - Verifier la production/finalite des blocs באמצעות טלמטריה.
   - Verifier la replication du ledger entre validateurs/observateurs.

## Etape 3 - Gouvernance et gestion des cles
1. מטען la configuration multisig du conseil; מאשר que les propositions de governance peuvent etre soumises et ratifiees.
2. Stocker de maniere securisee les cles de consensus/comite; מתכנת des sauvegardes automatiques avec journalisation d'acces.
3. יש להקפיד על פרוצדורות של רוטציה של דחיפות (`docs/source/nexus/key_rotation.md`) וספר הפעלה.

## Etape 4 - אינטגרציה CI/CD
1. מתקין צינורות:
   - בנייה ופרסום של אימות התמונות/Torii (GitHub Actions או GitLab CI).
   - אימות אוטומטי של תצורה (יצירת מוך, אימות חתימות).
   - פריסת צינורות (Helm/Customize) יוצקים אשכולות והפקה.
2. מיישם את בדיקות העשן ב-CI (demarrer un cluster ephemere, executer la suite canonique de transactions).
3. הוסיפו תסריטים להחזרה על תעריפי פריסה ותיעוד של ספרי ריצה.

## Etape 5 - Observabilite and alertes
1. Deployer la stack de monitoring (Prometheus + Grafana + Alertmanager) לפי אזור.
2. Coller les metriques coeur:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - יומנים דרך Loki/ELK pour les services Torii ובקונצנזוס.
3. לוחות מחוונים:
   - Sante du consensus (hauteur de bloc, finalite, statut des peers).
   - Latence/taux d'erreur de l'API Torii.
   - Transactions de gouvernance et statut des propositions.
4. התראות:
   - Arret de production de blocs (> 2 intervalles de bloc).
   - Baisse du nombre de peers sous le quorum.
   - Pics de taux d'erreur Torii.
   - צבר של הצעות ממשלתיות.## Etape 6 - אימות ומסירת
1. Executer la אימות מקצה לקצה:
   - Soumettre une proposition de governance (עמ' דוגמה Change de Parameter).
   - La faire passer par l'approbation du conseil pour s'assurer que le pipeline de governance fonctionne.
   - Executer un diff d'etat du ledger pour assurer la coherence.
2. Documenter le runbook pour on-call (אירוע תגובה, failover, קנה מידה).
3. Communiquer la disponibilite aux equipes SoraFS/SoraNet; confirmer que les deploiements piggyback peuvent pointer vers des noeuds Nexus.

## רשימת רשימות ליישום
- [ ] סיום תחילת הביקורת/תצורה.
- [ ] Noeuds validateurs et observateurs deployes avec un consensus sain.
- [ ] Cles de gouvernance chargees, נבחן בהצעה.
- [ ] Pipelines CI/CD en marche (בנייה + פריסה + בדיקות עשן).
- [ ] לוחות מחוונים פעילים עם התראה.
- [ ] תיעוד de handoff livree aux equipes במורד הזרם.