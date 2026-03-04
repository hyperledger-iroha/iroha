---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adapté à [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Fournisseurs de données politiques et d'identification SoraFS (noir SF-2b)

Cette étude de faisabilité a pour résultat des résultats pratiques pour **SF-2b** : améliorer et
принудительно применять процесс допуска, требования к идентичности и
Charge utile d'attestation pour la phase SoraFS du fournisseur. Она расширяет
Procédez à votre recherche, en consultant l'architecture RFC SoraFS, et lancez-vous
оставшуюся работу на отслеживаемые инженерные задачи.

## Ce sont les politiciens

- Assurez-vous que seuls les opérateurs agréés peuvent publier la fiche `ProviderAdvertV1`, selon le principe.
- Donnez la priorité à l'information sur les documents d'identification, la gouvernance internationale, l'attestation d'investissement et la participation minime.
- Préparer les vérifications des instruments de détermination, telles que Torii, et `sorafs-node`, les vérifications spécifiques.
- Prend en charge la maintenance et l'annulation d'événements autres que les instruments de détection ou d'utilisation.

## Требования к идентичности и pieu

| Trebovanie | Description | Résultat |
|------------|----------|---------------|
| Procédure d'entretien des clés | Le fournisseur doit s'enregistrer pour la clé Ed25519, qui est en mesure de recevoir une annonce. Le groupe a également pour mission de promouvoir la gouvernance. | Enregistrez le schéma `ProviderAdmissionProposalV1` sur le pôle `advert_key` (32 octets) et n'utilisez aucun fichier (`sorafs_manifest::provider_admission`). |
| Pieu Указатель | Pour la deuxième fois, le nouveau `StakePointer` est activé sur le pool de jalonnement actif. | Effectuez la validation dans `sorafs_manifest::provider_advert::StakePointer::validate()` et sélectionnez les paramètres CLI/test. |
| Теги юрисдикции | Le fournisseur obtient le droit + le contact légal. | Расширить схему предложения полем `jurisdiction_code` (ISO 3166-1 alpha-2) et опциональным `contact_uri`. |
| Demande d'attestation | Vous devez obtenir le certificat mTLS ou QUIC pour obtenir le certificat mTLS ou QUIC. | Déployez la charge utile Norito `EndpointAttestationV1` et envoyez-la à votre ordinateur dans la bande passante. |

## Processus de copie

1. **Prévisions**
   - CLI : ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`,
     formulaire `ProviderAdmissionProposalV1` + bande d'attestation.
   - Validation : sélectionnez le poteau de l'appareil, mise > 0, poignée de chunker canonique dans `profile_id`.
2. **Gouvernance améliorée**
   - Le module soviétique `blake3("sorafs-provider-admission-v1" || canonical_bytes)` utilise la fonctionnalité
     enveloppe d'instruments (module `sorafs_manifest::governance`).
   - Enveloppe сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Внесение в реестр**
   - Réalisez le validateur (`sorafs_manifest::provider_admission::validate_envelope`), ici
     переиспользуют Torii/шлюзы/CLI.
   - Ouvrez la copie Torii pour ouvrir des publicités, votre résumé ou ouvrir votre enveloppe.
4. **Обновление и отзыв**
   - Ajoutez `ProviderAdmissionRenewalV1` avec les détails opérationnels/pieux.
   - Ouvrez la CLI `--revoke`, ce qui permet d'ouvrir et d'ouvrir la gouvernance.

## Задачи реализации| Область | Задача | Propriétaire(s) | Statut |
|---------|--------|----------|--------|
| Schéma | Transférer `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) à `crates/sorafs_manifest/src/provider_admission.rs`. Réalisé dans `sorafs_manifest::provider_admission` avec les validations.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Stockage / Gouvernance | ✅ Завершено |
| Instruments CLI | Utiliser `sorafs_manifest_stub` pour les modules : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | ✅ Завершено |

CLI поток теперь принимает промежуточные бандлы сертификатов (`--endpoint-attestation-intermediate`),
Utilisez l'enveloppe/l'enveloppe canonique et envoyez le message correspondant à `sign`/`verify`. Les opérateurs peuvent
regarder une annonce publicitaire sur le site ou publier une annonce publicitaire, et les photos peuvent être téléchargées
Avant de connecter `--council-signature-public-key` à `--council-signature-file` pour l'automatisation.

### CLI principal

Utilisez la commande `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Drapeaux d'affichage : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et comme minime `--endpoint=<kind:host>`.
  - Pour l'attestation du client, la réponse est `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, certificat ici
    `--endpoint-attestation-leaf=<path>` (plus optionnel `--endpoint-attestation-intermediate=<path>`
    pour les éléments de sélection) et les jeux ALPN ID
    (`--endpoint-attestation-alpn=<token>`). Pour les services RAPIDES, vous pouvez précéder les transports en commun depuis
    `--endpoint-attestation-report[-hex]=...`.
  - Recherche : versions canoniques de Norito (`--proposal-out`) et code JSON
    (sortie standard à utiliser ou `--json-out`).
-`sign`
  - Входные данные: предложение (`--proposal`), подписанный advert (`--advert`), опциональное тело advert
    (`--advert-body`), époque de rétention et quelle durée de vie minimale. Il est possible de s'en rendre compte
    inline (`--council-signature=<signer_hex:signature_hex>`) ou ici, vous pouvez trouver
    `--council-signature-public-key` à `--council-signature-file=<path>`.
  - Former une enveloppe de validation (`--envelope-out`) et un JSON ajouté au résumé privé,
    числом подписантов и входными путями.
-`verify`
  - Vérifier l'enveloppe appropriée (`--envelope`) et éventuellement fournir une enveloppe appropriée,
    annonce ou annonce téléphonique. JSON-отчет значения digest, statut проверки подписей
    и какие опциональные артефакты совпали.
-`renewal`
  - Связывает новый утвержденный enveloppe с ранее утвержденным digest. Требуются
    `--previous-envelope=<path>` et ensuite `--envelope=<path>` (pour la charge utile Norito).
    La CLI fournit des informations sur les alias de profil, les capacités et les clés publicitaires qui ne sont pas disponibles, pour cette raison.
    обновления enjeu, эндпоинтов и métadonnées. Batterie canonique `ProviderAdmissionRenewalV1`
    (`--renewal-out`) et JSON-сводку.
-`revoke`
  - Si vous avez acheté une bande `ProviderAdmissionRevocationV1` pour le fournisseur, cette enveloppe ne doit pas être retirée.
    Testez `--envelope=<path>`, `--reason=<text>`, avec un minimum de `--council-signature` et des options supplémentaires
    `--revoked-at`/`--notes`. La CLI permet de vérifier et de vérifier le résumé, en affichant la charge utile Norito ici
    `--revocation-out` et le fichier JSON est ajouté au résumé et à la description.
| Proverbe | Réalisez le validateur en utilisant Torii, les modèles et `sorafs-node`. Préparer l'unité + les tests d'intégration CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Mise en réseau TL / Stockage | ✅ Завершено |
| Intégration Torii | En téléchargeant le validateur des premières annonces dans Torii, ouvrez les annonces en politique et publiez des informations télévisées. | Réseautage TL | ✅ Завершено | Torii vous permet d'ajouter des enveloppes de gouvernance (`torii.sorafs.admission_envelopes_dir`), de fournir un résumé/une publication par le biais d'un programme télévisé допуска.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || Nouveautés | Ajoutez des schémas de mise à jour/de mise à jour + CLI, ouvrez le cycle dans la documentation (avec le runbook et les commandes CLI dans `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Stockage / Gouvernance | ✅ Завершено |
| Télémétrie | Определить tableaux de bord/alertes `provider_admission` (пропущенное обновление, срок действия enveloppe). | Observabilité | 🟠Processus | La fiche `torii_sorafs_admission_total{result,reason}` correspond ; tableaux de bord/alertes en cours.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Mise à jour et mise à jour du Runbook

#### Плановое обновление (обновления pieu/топологии)
1. Vérifiez la pré-annonce/annonce suivante pour `provider-admission proposal` et `provider-admission sign`,
   увеличив `--retention-epoch` и обновив pieu/эндпоинты по необходимости.
2. Выполните
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   La commande prouve qu'elle n'a pas de capacité/profil ici
   `AdmissionRecord::apply_renewal`, выпускает `ProviderAdmissionRenewalV1` и печатает digests pour
   журнала gouvernance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Sélectionnez l'enveloppe précédente dans `torii.sorafs.admission_envelopes_dir`, puis remplacez la mise à jour Norito/JSON.
   Dans la gouvernance du référentiel et l'évolution du hachage + l'époque de rétention dans `docs/source/sorafs/migration_ledger.md`.
4. Veuillez contacter l'opérateur pour savoir quelle nouvelle enveloppe est activée et la résoudre.
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` pour la prise en charge.
5. Пересоздайте и зафиксируйте канонические luminaires через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ;
   CI (`ci/check_sorafs_fixtures.sh`) garantit la stabilité Norito.

#### Divers résultats
1. Ouvrir l'enveloppe commerciale et choisir les éléments suivants :
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   La CLI permet de télécharger `ProviderAdmissionRevocationV1`, afin de vérifier la situation ici.
   `verify_revocation_signatures` et je propose un résumé complet.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Sélectionnez l'enveloppe `torii.sorafs.admission_envelopes_dir` et transférez Norito/JSON dans le fichier d'admission.
   et précisez les principes de hachage dans le protocole de gouvernance.
3. Ouvrez le `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour le modifier.
   что кеши отбросили отозванный annonce; Ораните артефакты отзыва в ретроспективах инцидента.

## Tests et télémétrie- Ajouter des luminaires dorés pour la présentation et l'envoi d'enveloppes
  `fixtures/sorafs_manifest/provider_admission/`.
- Déverrouillez CI (`ci/check_sorafs_fixtures.sh`) pour la préparation et la vérification de l'enveloppe.
- Сгенерированные luminaires включают `metadata.json` с каноническими digests ; tests en aval
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Préparer les tests d'intégration :
  - Torii ouvre des publicités sur les enveloppes d'admission ou les enveloppes d'admission.
  - CLI проходит aller-retour предложения → enveloppe → vérification.
  - La gouvernance de l'attestation du point de terminaison sans identification du fournisseur.
- Travaux de télémétrie :
  - Émettre des compteurs `provider_admission_envelope_{accepted,rejected}` dans Torii. ✅ `torii_sorafs_admission_total{result,reason}` теперь показывает accepté/rejeté.
  - Effectuer la préparation de l'installation dans les 7 jours suivants.

## Следующие шаги

1. ✅ Vérifiez le schéma de configuration Norito et ajoutez les informations validées dans `sorafs_manifest::provider_admission`. Les drapeaux de fonctionnalités ne sont pas disponibles.
2. ✅ Paramètres CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) fournis et vérifiés tests d'intégration; Ajoutez la gouvernance des scripts à la synchronisation avec le runbook.
3. ✅ Torii admission/découverte contient des enveloppes et publie des fiches télémétriques d'entrée/sortie.
4. Focus sur l'installation : afficher les tableaux de bord/alertes lors de l'admission, les mises à jour, les travaux techniques semestriels, les étapes préalables à la mise en œuvre. (`torii_sorafs_admission_total`, jauges de péremption).