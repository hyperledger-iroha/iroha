---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
titre : Sora Nexus data-space آپریٹر آن بورڈنگ
description : `docs/source/sora_nexus_operator_onboarding.md` est un système de bout en bout et Nexus est un système de bout en bout. ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/sora_nexus_operator_onboarding.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Intégration d'un opérateur d'espace de données

Il s'agit d'un système de bout en bout pour l'espace de données de Sora Nexus. کے بعد عمل کرنا ہوتا ہے۔ Un runbook double piste (`docs/source/release_dual_track_runbook.md`) et une note de sélection d'artefact (`docs/source/release_artifact_selection.md`) Il s'agit d'un ensemble de paquets/images qui manifestent des modèles de configuration et des attentes de voie ainsi que des informations sur les images.## سامعین اور پیشگی شرائط
- Le programme Nexus fournit une affectation d'espace de données et une affectation d'espace de données (index de voie, ID/alias d'espace de données et exigences de politique de routage).
- L'ingénierie des versions contient des artefacts de version signés et des fichiers de version (archives tar, images, manifestes, signatures, clés publiques).
- آپ نے اپنے validateur/observateur رول کے لئے پروڈکشن matériel clé تیار یا حاصل کیا ہے (identité de nœud Ed25519, validateurs کے لئے clé de consensus BLS + PoP؛ اور (et les basculements des fonctionnalités confidentielles).
- Les pairs de Sora Nexus sont en train de démarrer le bootstrap.

## مرحلہ 1 - ریلیز پروفائل کی تصدیق
1. L'alias du réseau et l'ID de chaîne sont associés à votre nom de réseau.
2. Comment payer pour `scripts/select_release_profile.py --network <alias>` (`--chain-id <id>`) helper `release/network_profiles.toml` دیکھ کر déployer ہونے والا پروفائل پرنٹ کرتا ہے۔ Sora Nexus pour `iroha3` pour `iroha3` Il s'agit d'un projet de développement de Release Engineering et de Release Engineering.
3. La balise de version ریلیز اعلان میں دیا گیا est disponible (مثلاً `iroha3-v3.2.0`) ; اسی سے آپ artefacts اور manifestes حاصل کریں گے۔## مرحلہ 2 - artefacts حاصل کریں اور ویریفائی کریں
1. Ensemble `iroha3` (`<profile>-<version>-<os>.tar.zst`) et fichiers compagnon pour les fichiers associés (`.sha256`, اختیاری `.sig/.pub`, `<profile>-<version>-manifest.json`, et `<profile>-<version>-image.json` pour les conteneurs ڈپلائے کر رہے ہیں)۔
2. Pour l'intégrité et l'intégrité :
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Un KMS soutenu par du matériel est un vérificateur `openssl` pour un vérificateur de compte
3. tarball contient `PROFILE.toml` et JSON manifeste les éléments suivants :
   -`profile = "iroha3"`
   - `version`, `commit`, et `built_at` sont en cours de réalisation.
   - Le système d'exploitation/l'architecture correspond à la cible de déploiement et correspond à la réalité.
4. Ajouter l'image du conteneur en utilisant le code `<profile>-<version>-<os>-image.tar` pour vérifier le hachage/signature et `<profile>-<version>-image.json` pour l'image ID کنفرم کریں۔## مرحلہ 3 - modèles et configuration تیار کریں
1. extrait du bundle pour `config/` pour la configuration complète
2. `config/` est un modèle de modèle :
   - `public_key`/`private_key` pour les touches Ed25519 et les touches blanches Il y a de nombreuses clés HSM et des clés privées et des disques durs. config et connecteur HSM pour le connecteur
   - `trusted_peers`, `network.address` et `torii.address` pour les interfaces de démarrage et les pairs bootstrap. کریں۔
   - `client.toml` et point de terminaison Torii côté opérateur (configuration TLS pour les utilisateurs) et provisionnement des informations d'identification pour les utilisateurs کریں۔
3. bundle میں فراہم کردہ chain ID byرقرار رکھیں، الایہ کہ Governance واضح طور پر ہدایت دے - global lane ایک واحد identifiant de chaîne canonique چاہتا ہے۔
4. La société Sora est actuellement en contact avec la société : `irohad --sora --config <path>`. Il s'agit d'un chargeur de configuration SoraFS pour un système de rejet multivoies## مرحلہ 4 - métadonnées de l'espace de données et routage
1. `config/config.toml` correspond au catalogue de l'espace de données `[nexus]` du Conseil Nexus et correspond à ce qui suit :
   - `lane_count` موجودہ époque میں فعال voies کی مجموعی تعداد کے برابر ہونا چاہئے۔
   - `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` et les alias `index`/`id` sont également disponibles. چاہئیں۔ موجودہ entrées globales نہ ہٹائیں؛ اگر Council نے اضافی data-spaces دیئے ہیں تو اپنے alias délégués شامل کریں۔
   - L'espace de données est disponible `fault_tolerance (f)` pour votre compte. comités de relais de voie کا سائز `3f+1` ہوتا ہے۔
2. `[[nexus.routing_policy.rules]]` کو اپنی دی گئی پالیسی کے مطابق اپ ڈیٹ کریں۔ instructions de gouvernance du modèle par défaut pour la voie `1` pour les déploiements de contrats et la voie `2` pour l'itinéraire Il s'agit d'un espace de données dédié à la voie et d'un alias. Il s'agit d'un projet de développement de versions Release Engineering.
3. `[nexus.da]`, `[nexus.da.audit]`, et `[nexus.da.recovery]` seuils ریویو کریں۔ Le projet de loi est approuvé par le conseil et approuvé par le conseil. صرف اسی وقت بدلیں جب نئی پالیسی منظور ہو۔
4. Configuration et suivi des opérations de suivi des opérations Ticket d'intégration du runbook à double piste pour `config.toml` (secrets expurgés)## مرحلہ 5 - پری فلائٹ ویلیڈیشن
1. Voici un validateur de configuration intégré:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   La configuration résolue est terminée et les entrées de catalogue/routage sont terminées et la genèse et la configuration sont en panne.
2. Les conteneurs déploient une image de type `docker load -i <profile>-<version>-<os>-image.tar` et une image de type `--sora` (`--sora`). نہ بھولیں)۔
3. enregistre les identifiants de voie/espace de données réservés et les avertissements 4 étapes pour les déploiements de déploiements, les modèles et les identifiants d'espace réservé pour les différents déploiements
4. Procédure de fumée locale (`iroha_cli` et `FindNetworkStatus` requête pour les points finaux de télémétrie `nexus_lane_state_total` exposer les clés de streaming (rotation/importation)## مرحلہ 6 - Transition et transfert
1. Il s'agit d'un `manifest.json` pour les artefacts de signature et le ticket de sortie pour les auditeurs et les chèques pour les auditeurs.
2. Nexus Opérations liées à la configuration du système d'exploitation شامل کریں:
   - Identité du nœud (ID d'homologue, noms d'hôtes, point de terminaison Torii).
   - Catalogue de voies/espaces de données et politique de routage
   - Binaires/images vérifiés et hachages
3. Admission par les pairs (Gossip Seeds et attribution de voies) par `@nexus-core` pour le paiement par les pairs منظوری ملنے سے پہلے نیٹ ورک join نہ کریں؛ Sora Nexus occupation déterministe des voies disponible et manifeste d'admission mis à jour
4. Les versions en direct des runbooks et les remplacements de versions ultérieures de la balise de publication sont également disponibles pour l'itération. ligne de base شروع ہو۔

## ریفرنس چیک لسٹ
-[ ] Profil de version `iroha3` pour valider et valider
- [ ] Bundle/image hachages et signatures vérifiées et vérification
- [ ] Clés, adresses homologues et points de terminaison Torii pour les utilisateurs
- [ ] Nexus catalogue de voies/espaces de données et affectation du conseil de politique de routage et correspondance avec votre ordinateur
-[ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) pour les avertissements et les mises en garde
- [ ] Manifestes/signatures du ticket d'embarquement میں آرکائیو اور Ops کو اطلاع دے دی گئی ہے۔Phases de migration Nexus et attentes en matière de télémétrie کے وسیع تر سیاق کے لئے [Notes de transition Nexus](./nexus-transition-notes) دیکھیں۔