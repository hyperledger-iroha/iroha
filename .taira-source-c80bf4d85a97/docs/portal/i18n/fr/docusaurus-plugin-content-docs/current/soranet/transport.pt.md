---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : Visa général de transport SoraNet
sidebar_label : Visa général du transport
description : Poignée de main, rotation des sels et guide des capacités pour la superposition anonymisée de SoraNet.
---

:::note Fonte canonica
Cette page contient les spécifications du transport SNNet-1 dans `docs/source/soranet/spec.md`. Mantenha ambas comme copies synchronisées.
:::

SoraNet et la superposition anonymisée qui supporte les récupérations de plage par SoraFS, le streaming de Norito RPC et les futures voies de données par Nexus. Le programme de transport (les éléments de la feuille de route **SNNet-1**, **SNNet-1a** et **SNNet-1b**) définissent une poignée de main déterminée, la négociation de capacités post-quantiques (PQ) et un plan de rotation des sels pour que chaque relais, client et passerelle observe une même posture de sécurité.

## Metas et modèle de réseau

- Construire des circuits de trois sauts (entrée -> milieu -> sortie) sur QUIC v1 pour que les pairs abusent n'importe où Torii directement.
- Récupérez une poignée de main Noise XX *hybride* (Curve25519 + Kyber768) et QUIC/TLS pour relier les touches de session à la transcription TLS.
- Exiger des TLV de capacités qui annoncent le support PQ KEM/assinatura, papel do relay et versao de protocolo ; appliquer GREASE em tipos desconhecidos para manter futures extensoes implantaveis.
- Rotation des sels de contenu cego diariamente et fixation des relais de garde pendant 30 jours pour que le désabonnement do répertoire nao puisse désanonymiser les clients.
- Fixations de cellules Manter sur le 1024 B, injection de rembourrage/cellules factices et exportation de télémétrie déterminée pour capturer rapidement les tentatives de déclassement.

## Pipeline de poignée de main (SNNet-1a)

1. **Enveloppe QUIC/TLS** - les clients se connectent aux relais via QUIC v1 et complètent une poignée de main TLS 1.3 en utilisant les certificats Ed25519 attribués à la gouvernance CA. L'exportateur TLS (`tls-exporter("soranet handshake", 64)`) semeia a camada Noise pour que les transcriptions soient inseparaveis.
2. **Noise XX hybrid** - chaîne du protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS. Flux de messages :

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Le résultat DH de Curve25519 et les encapsulations d'ambas de Kyber sao misturados nas chaves simetricas finais. Faut-il négocier le matériel PQ avorta ou poignée de main pour compléter - il n'y a pas de solution de repli classique.

3. **Tickets de puzzle et jetons** - les relais peuvent émettre un ticket de preuve de travail Argon2id avant `ClientHello`. Les tickets sao frames avec le préfixe de compromis qui demande à la solution Argon2 hasheada et expiram dans les limites de la politique :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Les jetons d'admission avec le préfixe `SNTK` posent des problèmes lorsqu'un émetteur ML-DSA-44 est valide contre la politique actuelle et la liste de révision.

4. **Troca decapacité TLV** - la charge utile finale du bruit transporte les TLV de capacités décrites ci-dessous. Les clients abandonnent une connexion avec n'importe quelle capacité obrigatoria (PQ KEM/assinatura, papel ou versao) estiver ausente ou divergente da entrada do directory.5. **Registro do transcript** - relaie l'enregistrement du hachage de la transcription, un TLS numérique impressionnant et le contenu TLV pour alimenter les détecteurs de déclassement et les pipelines de conformité.

## TLV de capacité (SNNet-1c)

Comment réutiliser les capacités de l'enveloppe TLV fixée par `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Types définis aujourd'hui :

- `snnet.pqkem` - niveau Kyber (`kyber768` pour le déploiement actuel).
- `snnet.pqsig` - suite d'assinatura PQ (`ml-dsa-44`).
- `snnet.role` - relais papier (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identifiant de vers du protocole.
- `snnet.grease` - entrées de préenchiment aléatoires dans une réserve réservée pour garantir la tolérance aux futurs TLV.

Les clients ont une liste d'autorisation des TLV requis et falham ou une poignée de main lorsqu'ils sont omis ou rétrogradés. Les relais sont publics ou définis dans votre microdescripteur dans le répertoire pour que la validation soit déterministe.

## Rotation des sels et CID aveuglant (SNNet-1b)

- Une gouvernance publique inscrite au registre `SaltRotationScheduleV1` avec les valeurs `(epoch_id, salt, valid_after, valid_until)`. Relais et passerelles buscam ou calendrier assinado aucun éditeur d'annuaire.
- Les clients appliquent le nouveau sel em `valid_after`, le manteau du sel antérieur pendant une période de grâce de 12 heures et retem un historique de 7 époques pour tolérer des actualisations atrasadas.
- Identificadores cegos canonicos usam :

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" || salt || cid)
  ```

  Les passerelles s'enchaînent via `Sora-Req-Blinded-CID` et font écho à `Sora-Content-CID`. L'aveuglement du circuit/requête (`CircuitBlindingKey::derive`) se produit en `iroha_crypto::soranet::blinding`.
- Si un relais perd une époque, il interrompt les nouveaux circuits en baixar ou calendrier et émet un `SaltRecoveryEventV1`, que les tableaux de bord de service d'astreinte comme signal de radiomessagerie.

## Dados do directory et politique de garde

- Les microdescripteurs indiquent l'identité du relais (Ed25519 + ML-DSA-65), les codes PQ, les TLV de capacité, les étiquettes de région, l'éligibilité de garde et l'époque de sel annoncée.
- Les clients fixent les ensembles de garde pendant 30 jours et conservent les caches `guard_set` avec l'instantané supprimé du répertoire. Les wrappers CLI et SDK fournissent l'empreinte digitale du cache pour que les preuves de déploiement soient examinées lors des révisions de modification.

## Télémétrie et liste de contrôle de déploiement- Mesures à exporter avant la production :
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Les limites d'alerte vives au niveau de la matrice SLO du SOP de rotation des sels (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) et doivent être reflétées dans le gestionnaire d'alertes avant la promotion du réseau.
- Alertes : taxons de faux >5 % en 5 minutes, décalage salin >15 minutes ou inadéquation des capacités observées dans la production.
- Passos de déploiement :
  1. Exécuter les tests d'interopérabilité relais/client dans la mise en scène avec la poignée de main hybride et la pile PQ autorisée.
  2. Ensaiar o SOP de rotacao de salts (`docs/source/soranet_salt_plan.md`) et anexar os artefatos do forage ao change record.
  3. Habiliter la négociation de capacités sans répertoire, puis rôle pour les relais d'entrée, les relais intermédiaires, les relais de sortie et pour les clients fim.
  4. Le registraire garde les empreintes digitales du cache, les horaires de sel et les tableaux de bord de télémétrie pour chaque phase ; annexer le lot de preuves a `status.md`.

Cette liste de contrôle permet aux opérateurs, aux clients et au SDK d'adopter les transports de SoraNet en même temps qu'ils respectent les exigences de détermination et d'auditoire capturées sans feuille de route SNNet.