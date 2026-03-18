---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : Обзор транспорта SoraNet
sidebar_label : Обзор транспорта
description : Poignée de main, sels de rotation et activation pour la gestion anonyme de SoraNet.
---

:::note Канонический источник
Cette page est dédiée au transport spécifique SNNet-1 dans `docs/source/soranet/spec.md`. Vous pouvez obtenir des copies de synchronisation si vous souhaitez créer des documents sans avoir à vous soucier de l'exportation.
:::

SoraNet - C'est un aperçu anonyme qui permet de récupérer la plage SoraFS, Norito RPC streaming et de créer des voies de données Nexus. Le programme de transport (éléments de la feuille de route **SNNet-1**, **SNNet-1a** et **SNNet-1b**) a permis de déterminer la poignée de main, en prévoyant la valorisation post-quantique (PQ) et le plan de rotation des sels, Ce relais, client et passerelle déterminent la posture de sécurité.

## Celes et modeles

- Définissez des circuits triples (entrée -> milieu -> sortie) pour QUIC v1, qui permettent aux pairs de ne pas utiliser Torii.
- Permet la poignée de main Noise XX *hybride* (Curve25519 + Kyber768) pour QUIC/TLS, qui permet de partager les clés de session et la transcription TLS.
- Améliorer la capacité TLV, ce qui correspond au PQ KEM/poste, au rôle de relais et à la version du protocole ; GRAISSE неизвестные типы, чтобы будущие расширения оставались развертываемыми.
- Il est possible de faire pivoter les sels à contenu aveugle et de configurer les relais de garde pendant 30 jours, ce qui ne permet pas de désabonnement dans le répertoire pour les clients.
- Ajoutez des cellules de fixation sur 1024 B, activez des cellules de rembourrage/factices et exportez des données de télémétrie, pour que vous puissiez déclasser les pop-ups.

## Prise de contact avec le pipeline (SNNet-1a)

1. **Enveloppe QUIC/TLS** - les clients incluent des relais pour QUIC v1 et prennent en charge la poignée de main TLS 1.3, en utilisant le certificat Ed25519, en ajoutant la gouvernance CA. L'exportateur TLS (`tls-exporter("soranet handshake", 64)`) засевает слой Noise, чтобы transcriptions были неразделимы.
2. **Noise XX hybrid** - protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS. Поток сообщений:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Le Curve25519 DH et les encapsulations Kyber sont disponibles dans les clés finales symétriques. Les prédécesseurs éprouvés du PQ peuvent utiliser la poignée de main - solution de secours uniquement dans la classe sélectionnée.

3. **Tickets de puzzle et jetons** - les relais peuvent traiter le ticket de preuve de travail Argon2id pour `ClientHello`. Billets - ce sont des images à préfixe de longueur, qui ne nécessitent pas de résolution Argon2 et sont installées dans les politiques précédentes :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Les jetons d'admission avec le préfixe `SNTK` permettent d'obtenir des puzzles, ainsi que ML-DSA-44 pour les validations politiques et politiques les plus actives. отзыва.

4. **Échange de capacité TLV** - La charge utile de bruit finale permet d'exploiter les TLV de capacité, plus précisément. Les clients choisissent la connexion s'ils ont une capacité d'exploitation (PQ KEM/подпись, rôle ou version) ou ne sont pas connectés au répertoire de stockage.

5. **Journalisation des transcriptions** - relaie la transcription de hachage, l'empreinte digitale TLS et le contenu TLV, qui permet de gérer les détecteurs de déclassement et les pipelines de conformité.## TLV de capacité (SNNet-1c)

Capacités utilisées par le fixateur TLV-оболочку `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Types de période d'utilisation :

- `snnet.pqkem` - utilisez Kyber (`kyber768` pour le déploiement technique).
- `snnet.pqsig` - suite PQ подписей (`ml-dsa-44`).
- `snnet.role` - relais à rôle (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - version du protocole d'identification.
- `snnet.grease` - les éléments de remplissage sont insérés dans la diapason de remplissage, afin de respecter les TLV.

Les clients peuvent autoriser la liste d'autorisation des TLV et évaluer la poignée de main en cas de projet ou de rétrogradation. Les relais sont publiés dans le microdescripteur d'annuaire, ce qui permet de les valider.

## Sels de rotation et aveuglement CID (SNNet-1b)

- La gouvernance publie `SaltRotationScheduleV1` avec `(epoch_id, salt, valid_after, valid_until)`. Les relais et les passerelles peuvent être utilisés comme graphiques par l'éditeur d'annuaire.
- Les clients introduisent un nouveau sel dans `valid_after`, fournissant du sel avant 12 heures de grâce et une histoire de 7 époques pour la période de grâce. задержанных обновлений.
- Les identifiants aveugles canoniques utilisent :

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Les passerelles utilisent une clé aveugle pour `Sora-Req-Blinded-CID` et celle-ci est installée dans `Sora-Content-CID`. Blindage de circuit/demande (`CircuitBlindingKey::derive`) correspondant à `iroha_crypto::soranet::blinding`.
- Si le relais produit une époque, sur la mise en place de nouveaux circuits pour afficher la graphique et la commande `SaltRecoveryEventV1`, les tableaux de bord d'astreinte traitent le signal de radiomessagerie.

## Données d'annuaire et politique de garde

- Les microdescripteurs incluent le relais d'identité (Ed25519 + ML-DSA-65), les clés PQ, les TLV de capacité, les balises de région, l'éligibilité de la garde et l'époque de sel annoncée.
- Les clients fixent les ensembles de garde pendant 30 jours et mettent en cache `guard_set` dans l'instantané du répertoire correspondant. Les wrappers CLI et SDK permettent d'utiliser l'empreinte digitale du cache, le déploiement des preuves pouvant être effectué lors de l'examen des modifications.

## Télémétrie et liste de contrôle de déploiement

- Paramètres pour l'exportation avant la production :
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Les alertes de détection des sels rotatifs SOP (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) s'affichent et vous pouvez ouvrir une session dans Alertmanager. продвижения сети.
- Alertes : taux de défaillance > 5 % en 5 minutes, décalage salin > 15 minutes ou inadéquation des capacités en production.
- Déploiement de Shagi :
  1. Programmer les tests d'interopérabilité relais/client lors de la mise en scène avec une prise de contact hybride et une pile PQ.
  2. Supprimer les sels de rotation SOP (`docs/source/soranet_salt_plan.md`) et utiliser les artefacts de forage pour modifier l'enregistrement.
  3. Cliquez sur la négociation des capacités dans le répertoire, puis sur les relais d'entrée, les relais intermédiaires, les relais de sortie et les clients de connexion.
  4. Supprimez les empreintes digitales du cache de garde, les horaires de sel et les tableaux de bord de télémétrie pour chaque fonction ; приложить l'ensemble des preuves к `status.md`.Cette liste de contrôle permet aux opérateurs de commande, aux clients et au SDK de gérer les transports SoraNet de manière synchronisée et de procéder ainsi à la détection et à l'audio, отраженные в SNNet feuille de route.