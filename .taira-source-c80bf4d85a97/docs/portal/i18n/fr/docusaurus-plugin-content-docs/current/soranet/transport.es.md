---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : Résumé du transport de SoraNet
sidebar_label : Résumé du transport
description : Poignée de main, rotation des sels et guide des capacités pour la superposition anonymisée de SoraNet.
---

:::note Fuente canonica
Cette page reflète les spécifications du transport SNNet-1 en `docs/source/soranet/spec.md`. Manten ambas copias sincronizadas.
:::

SoraNet est la superposition anonyme qui répond aux récupérations de plage de SoraFS, au streaming de Norito RPC et aux futures voies de données de Nexus. Le programme de transport (éléments de la feuille de route **SNNet-1**, **SNNet-1a** et **SNNet-1b**) définit une poignée de main déterminée, une négociation de capacités post-quantiques (PQ) et un plan de rotation des sels pour que chaque relais, client et passerelle respecte la même posture de sécurité.

## Objets et modèle de rouge

- Construire des circuits de trois sauts (entrée -> milieu -> sortie) sur QUIC v1 pour que les pairs abusent nunca lleguen directement sur Torii.
- Superposez une poignée de main Noise XX *hibrido* (Curve25519 + Kyber768) sur QUIC/TLS pour relier les touches de session à la transcription TLS.
- Exiger les TLV des capacités qui annoncent le support PQ KEM/firma, le rôle du relais et la version du protocole ; hacer GREASE de tipos desconocidos para que futures extensionses sigan desplegables.
- Rotar sels de contenu ciego a journal et fijar garde relais pendant 30 jours pour que le désabonnement du directeur ne puisse pas désanonymiser les clients.
- Mantener cell fijas de 1024 B, inyectar padding/celdas dummy et exporter des données télémétriques déterminées pour que les intentions de déclassement soient détectées rapidement.

## Pipeline de poignée de main (SNNet-1a)

1. **Enveloppe QUIC/TLS** - les clients se connectent aux relais sur QUIC v1 et effectuent une poignée de main TLS 1.3 en utilisant les certificats Ed25519 fournis par la gouvernance CA. L'exportateur TLS (`tls-exporter("soranet handshake", 64)`) alimente la capa Noise pour que les transcriptions soient inséparables.
2. **Noise XX hybrid** - chaîne du protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS. Flux de messages :

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   La sortie DH de Curve25519 et les encapsulations de Kyber se font entendre dans les touches finales similaires. Si vous ne négociez pas de matériel PQ, la poignée de main est abandonnée complètement : elle ne permet pas de repli en solo.

3. **Puzzle tickets and tokens** - les relais peuvent exiger un ticket de preuve de travail Argon2id avant `ClientHello`. Les tickets sont encadrés avec un préfixe de longitude qui amène la solution Argon2 hasheada et expire dans les limites de la politique :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Les jetons d'admission préfixés avec `SNTK` évitent les énigmes lorsqu'il y a une entreprise ML-DSA-44 de l'émetteur valide contre la politique active et la liste de révocation.

4. **Intercambio decapacité TLV** - la charge utile finale du bruit transporte les TLV de capacités décrites ci-dessous. Les clients interrompent la connexion s'ils ont une capacité obligatoire (PQ KEM/firma, version rôle) ou ne coïncident pas avec l'entrée du directeur.5. **Registro del transcript** - les relais enregistrent le hash de la transcription, le fichier TLS et le contenu TLV pour alimenter les détecteurs de déclassement et les pipelines de remplissage.

## TLV de capacité (SNNet-1c)

Les capacités réutilisent un sur TLV fijo de `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Types définis aujourd'hui :

- `snnet.pqkem` - niveau Kyber (`kyber768` pour le déploiement actuel).
- `snnet.pqsig` - suite de société PQ (`ml-dsa-44`).
- `snnet.role` - rôle du relais (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identifiant de version du protocole.
- `snnet.grease` - entrées de transport aléatoires dans la plage réservée pour garantir que les futurs TLV seront tolérés.

Les clients conservent une liste d'autorisation des TLV requis et échouent dans la poignée de main si elle est omise ou dégradée. Les relais publient le même message dans votre microdescripteur du directeur pour que la validation soit déterministe.

## Rotation des sels et CID blinding (SNNet-1b)

- Governance publica un registro `SaltRotationScheduleV1` avec valeurs `(epoch_id, salt, valid_after, valid_until)`. Les relais et les passerelles obtiennent le calendrier établi à partir de l'éditeur d'annuaire.
- Les clients appliquant le nouveau sel en `valid_after`, maintiennent le sel antérieur pendant une période de grâce de 12 heures et conservent un historique de 7 époques pour tolérer les actualisations rétroactives.
- Les identificadores ciegos canonicos usan :

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Les passerelles acceptent la clé ciega via `Sora-Req-Blinded-CID` et la font écho à `Sora-Content-CID`. L'aveuglement du circuit/requête (`CircuitBlindingKey::derive`) est présent dans `iroha_crypto::soranet::blinding`.
- Si un relais traverse une époque, detiene nouveaux circuits doivent décharger le calendrier et émettre un `SaltRecoveryEventV1`, que les tableaux de bord de garde traitent comme un capteur de radiomessagerie.

## Données du directeur et de la politique des gardes

- Les microdescripteurs indiquent l'identité du relais (Ed25519 + ML-DSA-65), les clés PQ, les TLV de capacités, les étiquettes de région, l'éligibilité de la garde et l'époque du sel annoncée.
- Les clients fixent la garde pendant 30 jours et conservent les caches `guard_set` avec l'instantané de l'entreprise du répertoire. CLI et SDK exposent le cache du cache pour que les preuves de déploiement s'ajoutent aux révisions de changement.

## Télémétrie et liste de contrôle de déploiement- Mesures à exporter avant la production :
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Les cadres d'alerte sont associés à la matrice SLO de SOP de rotation des sels (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) et doivent être réfléchis dans Alertmanager avant de promouvoir le rouge.
- Alertes : > 5 % de la tâche de chute en 5 minutes, décalage salin > 15 minutes ou inadéquation des capacités observées en production.
- Étapes de déploiement :
  1. Effectuer des tests d'interopérabilité relais/client en staging avec la poignée de main hibernée et la pile PQ autorisée.
  2. Enregistrez le SOP de rotation des sels (`docs/source/soranet_salt_plan.md`) et ajoutez les artefacts du simulacro au registre des changements.
  3. Habiliter la négociation de capacités dans le directeur, puis désengager les relais d'entrée, les relais intermédiaires, les relais de sortie et enfin les clients.
  4. Empreintes digitales du registraire du cache de garde, calendriers de sel et tableaux de bord de télémétrie pour chaque phase ; ajouter le paquet de preuves au `status.md`.

Cette liste de contrôle permet aux équipes d'opérateurs, de clients et de SDK d'adopter les transports de SoraNet selon le même rythme tout en complétant la détermination et les exigences des auditeurs capturés dans la feuille de route SNNet.