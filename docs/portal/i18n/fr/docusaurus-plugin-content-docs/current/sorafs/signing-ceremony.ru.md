---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : Замена церемонии подписания
description : Le Parlement Sora met en œuvre et diffuse le chunker de luminaires SoraFS (SF-1b).
sidebar_label : Commentaires sur la cérémonie
---

> Feuille de route : **SF-1b — mise à jour des luminaires du Parlement Sora.**
> Le flux de travail parlementaire permet d'utiliser l'offre « церемонию подписания совета ».

Le rituel de montage du chunker SoraFS est disponible. Все утверждения теперь
Proche de **Парламент Sora** — DAO pour les utilisateurs actuels, mis à jour Nexus.
Les touches du panneau XOR pour la mise en service sont tournées vers le panneau et
голосуют on-chain за утверждение, отклонение или откат выпусков luminaires. C'est ça
Nous travaillons actuellement sur le processus et l'outillage pour les robots de chantier.

## Обзор парламента

- **Гражданство** — Les opérateurs bloquent le travail XOR, qui indique l'état et
  получить право на жеребьевку.
- **Panneaux** — Le mode d'affichage des panneaux est activé.
  (Infrastructure, Modération, Trésorerie, ...). Панель Infrastructure отвечает
  за утверждения luminaires SoraFS.
- **Жеребьевка и ротация** — Места в панелях перераспределяются с периодичностью,
  Selon le Parlement constitutionnel, aucun groupe n'est monopolisé
  утверждения.

## Поток утверждения luminaires

1. **Préparation à l'avance**
   - Tooling WG propose le bundle candidat `manifest_blake3.json` et le luminaire diff
     dans le registre en chaîne par `sorafs.fixtureProposal`.
   - Pré-configuration du digest BLAKE3, version sémantique et spécifications pour les modifications.
2. **Ревью и голосование**
   - Le panneau Infrastructure s'occupe de la configuration du Parlement.
   - Les panneaux détectent les artéfacts CI, effectuent des tests de parité et effectuent des tests en chaîne
     взвешенными голосами.
3. **Finalisation**
   - Après l'installation du quorum runtime, vous pouvez modifier le résumé canonique
     manifeste et engagement de Merkle concernant le dispositif de charge utile.
   - Veuillez vous connecter au registre SoraFS pour que les clients puissent les contacter ultérieurement.
     manifeste, утвержденный парламентом.
4. **Распространение**
   - Les assistants CLI (`cargo xtask sorafs-fetch-fixture`) fournissent un manifeste externe
     ici Nexus RPC. Constantes JSON/TS/Go dans les dépôts synchronisés
     J'ai trouvé `export_vectors` et j'ai vérifié le résumé en direct de la chaîne.

## Workflow de planification

- Перегенерируйте luminaires:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utilisez l'assistant de récupération parlementaire pour récupérer l'enveloppe extérieure et vérifier
  подписи и обновить local luminaires. Achetez `--signatures` sur l'enveloppe, disponible
  парламентом; helper crée un manifeste informatique, accélère le résumé de BLAKE3 et l'ajoute
  Profil canonique `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Utilisez `--manifest` si le manifeste est envoyé à l'URL de votre navigateur. Enveloppe sans подписей
отклоняются, если не задан `--allow-unsigned` для локальных fuites de fumée.

- Pour valider le manifeste de la passerelle intermédiaire, utilisez Torii au niveau local
  charges utiles :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```- La liste locale des CI n'est pas disponible sur la liste `signer.json`.
  `ci/check_sorafs_fixtures.sh` stocke un dépôt après la chaîne
  engagement et падает при расхождениях.

## Замечания по gouvernance

- La Constitution du Parlement détermine le quorum, la rotation et l'augmentation — configuration
  на уровне caisse не нужна.
- La restauration d'urgence s'effectue à partir du panneau de configuration du paramètre. Panneau
  Infrastructure подает revert proposition ссылкой на предыдущий digest manifest,
  и релиз заменяется после утверждения.
- La modification de l'historique est effectuée dans le registre SoraFS pour la relecture médico-légale.

##FAQ

- **Qu'est-ce que je18NI00000022X ?**  
  Bien sûr. Votre auteur est responsable de la chaîne ; `manifest_signatures.json`
  dans les dépôts — c'est un appareil de développeur, qui doit être ajouté à la suite
  событием утверждения.

- **Qu'avez-vous à voir avec les modules locaux Ed25519 ?**  
  Non. Le changement de paramètre concerne les articles en chaîne. Calendrier local
  нужны для воспроизводимости, но проверяются по digest парламента.

- **Quelles sont les commandes qui surveillent l'arrêt ?**  
  Téléchargez le logiciel `ParliamentFixtureApproved` ou téléchargez le registre ici
  Nexus RPC, vous devez ouvrir le manifeste de résumé du texte et les panneaux d'affichage.