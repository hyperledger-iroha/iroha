---
lang: fr
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : règlement-iso-cartographie
titre : Règlement ↔ Cartographie de terrain ISO 20022
sidebar_label : Règlement ↔ ISO 20022
description : Cartographie canonique entre les flux de règlement Iroha et le pont ISO 20022.
---

:::note Source canonique
:::

## Règlement ↔ Cartographie de champ ISO 20022

Cette note capture le mappage canonique entre les instructions de règlement Iroha
(`DvpIsi`, `PvpIsi`, flux de collatéral repo) et les messages ISO 20022 exercés
par le pont. Il reflète l'échafaudage de messages mis en œuvre dans
`crates/ivm/src/iso20022.rs` et sert de référence lors de la production ou
validation des charges utiles Norito.

### Politique de Données de Référence (Identifiants et Validation)

Cette politique regroupe les préférences d'identifiant, les règles de validation et les données de référence
obligations que le pont Norito ↔ ISO 20022 doit faire respecter avant d'émettre des messages.**Points d'ancrage à l'intérieur du message ISO :**
- **Identifiants de l'instrument** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (ou le champ d'instrument équivalent).
- **Parties / agents** → `DlvrgSttlmPties/Pty` et `RcvgSttlmPties/Pty` pour `sese.*`,
  ou les structures d'agent dans `pacs.009`.
- **Comptes** → éléments `…/Acct` pour comptes de garde/espèces ; refléter le grand livre
  `AccountId` dans `SupplementaryData`.
- **Identifiants propriétaires** → `…/OthrId` avec `Tp/Prtry` et mis en miroir dans
  `SupplementaryData`. Ne remplacez jamais les identifiants réglementés par des identifiants propriétaires.

#### Préférence d'identifiant par famille de messages

##### `sese.023` / `.024` / `.025` (règlement de titres)- **Instrument (`FinInstrmId`)**
  - De préférence : **ISIN** sous `…/ISIN`. C'est l'identifiant canonique des CSD / T2S.[^anna]
  - Solutions de repli :
    - **CUSIP** ou autre NSIN sous `…/OthrId/Id` avec `Tp/Cd` défini depuis l'ISO externe
      liste de codes (par exemple, `CUSP`) ; inclure l'émetteur dans `Issr` lorsque cela est obligatoire.[^iso_mdr]
    - **Norito ID d'actif** en tant que propriétaire : `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` et
      enregistrez la même valeur dans `SupplementaryData`.
  - Descripteurs facultatifs : **CFI** (`ClssfctnTp`) et **FISN** lorsqu'ils sont pris en charge pour faciliter
    réconciliation.[^iso_cfi][^iso_fisn]
- **Parties (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Préféré : **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback : **LEI** où la version du message expose un champ LEI dédié ; si
    absent, portez des identifiants propriétaires avec des étiquettes `Prtry` claires et incluez le BIC dans les métadonnées.[^iso_cr]
- **Lieu d'établissement / lieu** → **MIC** pour le lieu et **BIC** pour le CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` et `colr.007` (gestion du collatéral)

- Suivez les mêmes règles d'instrument que `sese.*` (ISIN préféré).
- Les parties utilisent **BIC** par défaut ; **LEI** est acceptable là où le schéma l'expose.[^swift_bic]
- Les montants en espèces doivent utiliser les codes de devise **ISO 4217** avec les unités mineures correctes.[^iso_4217]##### `pacs.009` / `camt.054` (financement et relevés PvP)

- **Agents (`InstgAgt`, `InstdAgt`, agents débiteurs/créanciers)** → **BIC** avec option
  LEI là où cela est autorisé.[^swift_bic]
- **Comptes**
  - Interbancaire : identification par **BIC** et références de compte internes.
  - Relevés destinés aux clients (`camt.054`) : inclure **IBAN** lorsqu'il est présent et le valider
    (longueur, règles du pays, somme de contrôle mod-97).[^swift_iban]
- **Devise** → **ISO 4217** Code à 3 lettres, respecter l'arrondi des unités mineures.[^iso_4217]
- **Ingestion Torii** → Soumettre les segments de financement PvP via `POST /v1/iso20022/pacs009` ; le pont
  nécessite `Purp=SECU` et applique désormais les croisements BIC lorsque les données de référence sont configurées.

#### Règles de validation (applicables avant émission)| Identifiant | Règle de validation | Remarques |
|------------|-------|-------|
| **ISIN** | Chiffre de contrôle Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` et Luhn (mod-10) conformément à l'annexe C de la norme ISO 6166 | Rejeter avant l'émission du pont ; privilégier l'enrichissement en amont.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` et module-10 avec 2 pondérations (les caractères correspondent aux chiffres) | Uniquement lorsque l'ISIN n'est pas disponible ; carte via le passage pour piétons ANNA/CUSIP une fois obtenu.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` et chiffre de contrôle mod-97 (ISO 17442) | Validez par rapport aux fichiers delta quotidiens de la GLEIF avant acceptation.[^gleif] |
| **BIC** | Expression régulière `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Code de succursale facultatif (trois derniers caractères). Confirmez l'état actif dans les fichiers RA.[^swift_bic] |
| **MIC** | Maintenir à partir du fichier ISO 10383 RA ; s'assurer que les sites sont actifs (pas d'indicateur de terminaison `!`) | Signaler les MIC déclassés avant l'émission.[^iso_mic] |
| **IBAN** | Longueur spécifique au pays, alphanumérique majuscule, mod-97 = 1 | Utiliser le registre géré par SWIFT ; rejeter les IBAN structurellement invalides.[^swift_iban] |
| **Compte propriétaire/identifiants de partie** | `Max35Text` (UTF-8, ≤35 caractères) avec espaces coupés | S'applique aux champs `GenericAccountIdentification1.Id` et `PartyIdentification135.Othr/Id`. Rejetez les entrées dépassant 35 caractères afin que les charges utiles du pont soient conformes aux schémas ISO. || **Identifiants de compte proxy** | `Max2048Text` non vide sous `…/Prxy/Id` avec codes de type facultatifs dans `…/Prxy/Tp/{Cd,Prtry}` | Stocké avec l'IBAN principal ; la validation nécessite toujours des IBAN tout en acceptant les descripteurs de proxy (avec des codes de type facultatifs) pour refléter les rails PvP. |
| **FCI** | Code à six caractères, lettres majuscules utilisant la taxonomie ISO 10962 | Enrichissement facultatif ; assurez-vous que les caractères correspondent à la classe d'instrument.[^iso_cfi] |
| **FISN** | Jusqu'à 35 caractères, alphanumériques majuscules et ponctuation limitée | Facultatif; tronquer/normaliser conformément aux directives ISO 18774.[^iso_fisn] |
| **Devise** | Code ISO 4217 à 3 lettres, échelle déterminée par des unités mineures | Les montants doivent être arrondis aux décimales autorisées ; appliquer du côté Norito.[^iso_4217] |

#### Obligations de passage pour piétons et de maintenance des données- Maintenir les passages pour piétons **ISIN ↔ Norito Asset ID** et **CUSIP ↔ ISIN**. Mise à jour tous les soirs à partir de
  Les flux ANNA/DSB et le contrôle de version des instantanés utilisés par CI.[^anna_crosswalk]
- Actualiser les mappages **BIC ↔ LEI** à partir des fichiers de relations publiques de la GLEIF afin que le pont puisse
  émettre les deux si nécessaire.[^bic_lei]
- Stockez les **définitions MIC** à côté des métadonnées du pont afin que la validation du lieu soit
  déterministe même lorsque les fichiers RA changent à midi.[^iso_mic]
- Enregistrer la provenance des données (horodatage + source) dans les métadonnées du pont pour l'audit. Persistez le
  identifiant d'instantané à côté des instructions émises.
- Configurez `iso_bridge.reference_data.cache_dir` pour conserver une copie de chaque ensemble de données chargé
  aux côtés des métadonnées de provenance (version, source, horodatage, somme de contrôle). Cela permet aux auditeurs
  et les opérateurs peuvent comparer les flux historiques même après la rotation des instantanés en amont.
- Les instantanés ISO Crosswalk sont ingérés par `iroha_core::iso_bridge::reference_data` en utilisant
  le bloc de configuration `iso_bridge.reference_data` (chemins + intervalle de rafraîchissement). Jauges
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` et
  `iso_reference_refresh_interval_secs` expose l’état d’exécution pour les alertes. Le Torii
  Le pont rejette les soumissions `pacs.008` dont les BIC d'agent sont absents du fichier configuré.
  passage pour piétons, faisant apparaître des erreurs déterministes `InvalidIdentifier` lorsqu'une contrepartie est
  inconnu.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】- Les liaisons IBAN et ISO 4217 sont appliquées au même niveau : pacs.008/pacs.009 circule désormais
  émettre des erreurs `InvalidIdentifier` lorsque les IBAN débiteur/créancier manquent d'alias configurés ou lorsque
  la devise de règlement est absente de `currency_assets`, ce qui évite un pont mal formé
  instructions d'atteindre le grand livre. La validation IBAN s'applique également à chaque pays
  longueurs et chiffres de contrôle numériques avant la réussite de la norme ISO 7064 mod‑97, donc structurellement invalides
  les valeurs sont rejetées prématurément.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- Les aides au règlement CLI héritent des mêmes garde-corps : passe
  `--iso-reference-crosswalk <path>` aux côtés de `--delivery-instrument-id` pour avoir le DvP
  prévisualisez la validation des ID d'instrument avant d'émettre l'instantané XML `sese.023`.【crates/iroha_cli/src/main.rs#L3752】
- Peluches `cargo xtask iso-bridge-lint` (et le wrapper CI `ci/check_iso_reference_data.sh`)
  instantanés et luminaires des passages pour piétons. La commande accepte `--isin`, `--bic-lei`, `--mic` et
  `--fixtures` signale et revient aux exemples d'ensembles de données dans `fixtures/iso_bridge/` lors de l'exécution
  sans arguments.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- L'assistant IVM ingère désormais de véritables enveloppes XML ISO 20022 (head.001 + `DataPDU` + `Document`)
  et valide le Business Application Header via le schéma `head.001` donc `BizMsgIdr`,Les agents `MsgDefIdr`, `CreDt` et BIC/ClrSysMmbId sont préservés de manière déterministe ; XMLDSig/XAdES
  les blocs restent intentionnellement ignorés. 

#### Considérations réglementaires et liées à la structure du marché

- **Règlement T+1** : les marchés actions US/Canada sont passés à T+1 en 2024 ; ajuster Norito
  planification et alertes SLA en conséquence.[^sec_t1][^csa_t1]
- **Pénalités CSDR** : les règles de discipline en matière de règlement imposent des pénalités en espèces ; assurer Norito
  les métadonnées capturent les références aux pénalités pour la réconciliation.[^csdr]
- **Tests pilotes de règlement le jour même** : le régulateur indien met progressivement en œuvre le règlement T0/T+0 ; garder
  calendriers de pont mis à jour à mesure que les pilotes se développent.[^india_t0]
- **Buts/conservations de garanties** : surveillez les mises à jour de l'ESMA sur les délais de rachat et les retenues facultatives
  la livraison conditionnelle (`HldInd`) est donc conforme aux dernières directives.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Livraison contre paiement → `sese.023`| Champ DvP | Chemin ISO 20022 | Remarques |
|-----------------------------------------|----------------------------------------|-------|
| `settlement_id` | `TxId` | Identifiant de cycle de vie stable |
| `delivery_leg.asset_definition_id` (sécurité) | `SctiesLeg/FinInstrmId` | Identifiant canonique (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Chaîne décimale ; rend hommage à la précision des actifs |
| `payment_leg.asset_definition_id` (devise) | `CashLeg/Ccy` | Code devise ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | Chaîne décimale ; arrondi par spécification numérique |
| `delivery_leg.from` (vendeur / livreur) | `DlvrgSttlmPties/Pty/Bic` | BIC du participant livreur *(l'ID canonique du compte est actuellement exporté dans les métadonnées)* |
| `delivery_leg.from` identifiant de compte | `DlvrgSttlmPties/Acct` | Forme libre ; Les métadonnées Norito portent l'ID de compte exact |
| `delivery_leg.to` (acheteur / destinataire) | `RcvgSttlmPties/Pty/Bic` | BIC du participant destinataire || `delivery_leg.to` identifiant de compte | `RcvgSttlmPties/Acct` | Forme libre ; correspond à l'ID du compte de réception |
| `plan.order` | `Plan/ExecutionOrder` | Énumération : `DELIVERY_THEN_PAYMENT` ou `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Énumération : `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Objectif du message** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (livrer) ou `RECE` (réception) ; reflète la jambe que la partie soumettante exécute. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (contre paiement) ou `FREE` (gratuit). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | En option Norito JSON codé en UTF‑8 |

> **Qualificateurs de règlement** – le pont reflète les pratiques du marché en copiant les codes de conditions de règlement (`SttlmTxCond`), les indicateurs de règlement partiel (`PrtlSttlmInd`) et d'autres qualificatifs facultatifs des métadonnées Norito dans `sese.023/025` lorsqu'ils sont présents. Appliquez les énumérations publiées dans les listes de codes externes ISO afin que le CSD de destination reconnaisse les valeurs.

### Financement paiement contre paiement → `pacs.009`Les branches cash-for-cash qui financent une instruction PvP sont émises sous forme de crédit FI-to-FI.
transferts. Le pont annote ces paiements afin que les systèmes en aval reconnaissent
ils financent un règlement de titres.

| Champ de financement PvP | Chemin ISO 20022 | Remarques |
|------------------------------------------------|----------------------------------------------------|-------|
| `primary_leg.quantity` / {montant, devise} ​​| `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Montant/devise débité de l'initiateur. |
| Identifiants de l'agent de contrepartie | `InstgAgt`, `InstdAgt` | BIC/LEI des agents expéditeurs et destinataires. |
| Objet du règlement | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Défini sur `SECU` pour le financement PvP lié aux titres. |
| Métadonnées Norito (identifiants de compte, données FX) | `CdtTrfTxInf/SplmtryData` | Contient l'ID de compte complet, les horodatages FX et les conseils sur le plan d'exécution. |
| Identificateur d'instruction / liaison du cycle de vie | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Correspond au Norito `settlement_id` afin que la partie trésorerie se réconcilie avec le côté titres. |Le pont ISO du SDK JavaScript s'aligne sur cette exigence en définissant par défaut le
Objectif de la catégorie `pacs.009` à `SECU` ; les appelants peuvent le remplacer par un autre
Code ISO valide lors de l'émission de virements non-titres, mais invalide
les valeurs sont rejetées d’emblée.

Si une infrastructure nécessite une confirmation explicite des titres, le pont
continue d'émettre `sese.025`, mais cette confirmation reflète la jambe titres
statut (par exemple, `ConfSts = ACCP`) plutôt que le « but » PvP.

### Confirmation de paiement contre paiement → `sese.025`| Champ PvP | Chemin ISO 20022 | Remarques |
|-----------------------------------------------|-------------------------------|-------|
| `settlement_id` | `TxId` | Identifiant de cycle de vie stable |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Code de devise pour l'étape principale |
| `primary_leg.quantity` | `SttlmAmt` | Montant délivré par l'initiateur |
| `counter_leg.asset_definition_id` | `AddtlInf` (charge utile JSON) | Code de devise du compteur intégré dans les informations supplémentaires |
| `counter_leg.quantity` | `SttlmQty` | Montant du compteur |
| `plan.order` | `Plan/ExecutionOrder` | Même énumération définie que DvP |
| `plan.atomicity` | `Plan/Atomicity` | Même énumération définie que DvP |
| Statut `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` en cas de correspondance ; pont émet des codes de défaillance en cas de rejet |
| Identifiants des contreparties | `AddtlInf` JSON | Le pont actuel sérialise les tuples AccountId/BIC complets dans les métadonnées |

### Substitution de garanties de pension → `colr.007`| Champ / contexte du dépôt | Chemin ISO 20022 | Remarques |
|-------------------------------------------------|-----------------------------------|--------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Identifiant du contrat de pension |
| Identifiant Tx de substitution de garantie | `TxId` | Généré par substitution |
| Quantité de garantie originale | `Substitution/OriginalAmt` | Matches promis en garantie avant substitution |
| Devise de garantie d'origine | `Substitution/OriginalCcy` | Code devise |
| Quantité de garantie de remplacement | `Substitution/SubstituteAmt` | Montant de remplacement |
| Devise de garantie de remplacement | `Substitution/SubstituteCcy` | Code devise |
| Date d'entrée en vigueur (échéancier de marge de gouvernance) | `Substitution/EffectiveDt` | Date ISO (AAAA-MM-JJ) |
| Classement des coupes de cheveux | `Substitution/Type` | Actuellement `FULL` ou `PARTIAL` basé sur la politique de gouvernance |
| Raison de gouvernance / note de coupe de cheveux | `Substitution/ReasonCd` | Facultatif, comporte une justification de gouvernance |

### Financement et relevés| Contexte Iroha | Message ISO 20022 | Localisation cartographique |
|----------------------------------|-------------------|------------------|
| Repo cash leg allumage/déroulement | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` remplis à partir des segments DvP/PvP |
| Déclarations post-règlement | `camt.054` | Mouvements de jambe de paiement enregistrés sous `Ntfctn/Ntry[*]` ; Bridge injecte des métadonnées de grand livre/compte dans `SplmtryData` |

### Notes d'utilisation* Tous les montants sont sérialisés à l'aide des assistants numériques Norito (`NumericSpec`)
  pour garantir la conformité à l’échelle entre les définitions d’actifs.
* Les valeurs `TxId` sont `Max35Text` — appliquez une longueur UTF‑8 ≤ 35 caractères avant
  exportation vers des messages ISO 20022.
* Les BIC doivent comporter 8 ou 11 caractères alphanumériques majuscules (ISO9362) ; rejeter
  Métadonnées Norito qui échouent à cette vérification avant d'émettre des paiements ou des règlements
  confirmations.
* Les identifiants de compte (AccountId / ChainId) sont exportés dans des fichiers supplémentaires
  métadonnées afin que les participants récepteurs puissent effectuer un rapprochement avec leurs grands livres locaux.
* `SupplementaryData` doit être un JSON canonique (UTF‑8, clés triées, JSON natif
  s'échapper). Les assistants du SDK appliquent cela afin que les signatures, les hachages de télémétrie et l'ISO
  les archives de charge utile restent déterministes lors des reconstructions.
* Les montants en devises suivent les chiffres des fractions ISO4217 (par exemple, JPY a 0
  décimales, USD en a 2); le pont serre la précision numérique Norito en conséquence.
* Les assistants de règlement CLI (`iroha app settlement ... --atomicity ...`) émettent désormais
  Instructions Norito dont les plans d'exécution mappent 1:1 à `Plan/ExecutionOrder` et
  `Plan/Atomicity` ci-dessus.
* L'assistant ISO (`ivm::iso20022`) valide les champs listés ci-dessus et rejette
  messages dans lesquels les étapes DvP/PvP violent les spécifications numériques ou la réciprocité de la contrepartie.

### Aides au générateur de SDK- Le SDK JavaScript expose désormais `buildPacs008Message` /
  `buildPacs009Message` (voir `javascript/iroha_js/src/isoBridge.js`) donc client
  l'automatisation peut convertir les métadonnées de règlement structuré (BIC/LEI, IBAN,
  codes objet, champs supplémentaires Norito) en pacs déterministes XML
  sans réimplémenter les règles de mappage de ce guide.
- Les deux assistants nécessitent un `creationDateTime` explicite (ISO‑8601 avec fuseau horaire)
  les opérateurs doivent donc plutôt intégrer un horodatage déterministe à partir de leur flux de travail
  de laisser le SDK par défaut à l'heure de l'horloge murale.
- `recipes/iso_bridge_builder.mjs` montre comment connecter ces assistants à
  une CLI qui fusionne les variables d'environnement ou les fichiers de configuration JSON, imprime le
  XML généré et le soumet éventuellement à Torii (`ISO_SUBMIT=1`), en réutilisant
  la même cadence d'attente que la recette du pont ISO.


### Références- Exemples de règlement LuxCSD / Clearstream ISO 20022 montrant `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) et `Pmt` (`APMT`/`FREE`).[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Spécifications Clearstream DCP couvrant les qualificatifs de règlement (`SttlmTxCond`, `PrtlSttlmInd`).[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- Directives SWIFT PMPG recommandant `pacs.009` avec `CtgyPurp/Cd = SECU` pour le financement PvP lié aux titres.[3](https://www.swift.com/swift-resource/251897/download)
- Rapports sur la définition des messages ISO 20022 pour les contraintes de longueur d'identifiant (BIC, Max35Text).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- Orientations de l'ANNA DSB sur le format ISIN et les règles de somme de contrôle.[5](https://www.anna-dsb.com/isin/)

### Conseils d'utilisation

- Collez toujours l'extrait de code Norito ou la commande CLI pertinent afin que LLM puisse inspecter
  noms de champs exacts et échelles numériques.
- Demander des citations (`provide clause references`) pour conserver une trace écrite de
  conformité et examen par l’auditeur.
- Capturez le résumé de la réponse dans `docs/source/finance/settlement_iso_mapping.md`
  (ou annexes liées) afin que les futurs ingénieurs n'aient pas besoin de répéter la requête.

## Manuels de commande d'événements (ISO 20022 ↔ Norito Bridge)

### Scénario A — Substitution de garanties (Repo / Nage)

**Participants :** donneur/preneur de garantie (et/ou agents), dépositaire(s), CSD/T2S  
**Calendrier :** selon les coupures de marché et les cycles jour/nuit de T2S ; orchestrez les deux étapes afin qu'elles se terminent dans la même fenêtre de règlement.#### Chorégraphie des messages
1. `colr.010` Demande de substitution de garantie → donneur/preneur de garantie ou agent.  
2. Réponse de substitution de garantie `colr.011` → accepter/rejeter (motif de rejet facultatif).  
3. `colr.012` Confirmation de substitution de garantie → confirme l'accord de substitution.  
4. Instructions `sese.023` (deux pattes) :  
   - Renvoyez les garanties originales (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Fournir des garanties de remplacement (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Liez la paire (voir ci-dessous).  
5. Avis d'état `sese.024` (accepté, correspondant, en attente, en échec, rejeté).  
6. Confirmations `sese.025` une fois réservée.  
7. Delta en espèces facultatif (frais/décote) → `pacs.009` Virement de crédit FI à FI avec `CtgyPurp/Cd = SECU` ; état via `pacs.002`, revient via `pacs.004`.

#### Accusés de réception/statuts requis
- Niveau transport : les passerelles peuvent émettre des `admi.007` ou des rejets avant le traitement métier.  
- Cycle de vie du règlement : `sese.024` (états de traitement + codes de motif), `sese.025` (final).  
- Côté caisse : `pacs.002` (`PDNG`, `ACSC`, `RJCT` etc.), `pacs.004` pour les retours.#### Conditionnalité / champs de déroulement
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) pour enchaîner les deux instructions.  
- `SttlmParams/HldInd` à conserver jusqu'à ce que les critères soient remplis ; version via `sese.030` (état `sese.031`).  
- `SttlmParams/PrtlSttlmInd` pour contrôler le règlement partiel (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` pour les conditions spécifiques au marché (`NOMC`, etc.).  
- Règles facultatives de livraison conditionnelle de titres T2S (CoSD) lorsqu'elles sont prises en charge.

#### Références
- MDR de gestion des garanties SWIFT (`colr.010/011/012`).  
- Guides d'utilisation CSD/T2S (par exemple, DNB, ECB Insights) pour les liens et les statuts.  
- Pratique de règlement SMPG, manuels Clearstream DCP, ateliers ASX ISO.

### Scénario B – Violation de la fenêtre FX (échec du financement PvP)

**Participants :** contreparties et agents de trésorerie, dépositaire titres, CSD/T2S  
**Calendrier :** Fenêtres FX PvP (CLS/bilatéral) et seuils CSD ; garder les jambes de titres en attente en attendant la confirmation du paiement en espèces.#### Chorégraphie des messages
1. Virement de crédit FI à FI `pacs.009` par devise avec `CtgyPurp/Cd = SECU` ; statut via `pacs.002` ; rappel/annulation via `camt.056`/`camt.029` ; si déjà réglé, retour `pacs.004`.  
2. Instruction(s) DvP `sese.023` avec `HldInd=true` afin que la branche titres attende la confirmation en espèces.  
3. Avis du cycle de vie `sese.024` (acceptés/correspondants/en attente).  
4. Si les deux jambes `pacs.009` atteignent `ACSC` avant l'expiration de la fenêtre → relâchez avec `sese.030` → `sese.031` (état du mod) → `sese.025` (confirmation).  
5. Si la fenêtre de change est violée → annuler/rappeler des espèces (`camt.056/029` ou `pacs.004`) et annuler des titres (`sese.020` + `sese.027`, ou inversion `sese.026` si déjà confirmée par règle de marché).

#### Accusés de réception/statuts requis
- Espèces : `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` pour les retours.  
- Titres : `sese.024` (raisons en attente/échec telles que `NORE`, `ADEA`), `sese.025`.  
- Transport : `admi.007` / rejets passerelle avant traitement métier.#### Conditionnalité / champs de déroulement
- `SttlmParams/HldInd` + `sese.030` libération/annulation en cas de succès/échec.  
- `Lnkgs` pour lier les instructions sur titres à la jambe cash.  
- Règle T2S CoSD en cas d'utilisation de la livraison conditionnelle.  
- `PrtlSttlmInd` pour éviter les partiels involontaires.  
- Sur `pacs.009`, `CtgyPurp/Cd = SECU` signale les financements liés aux titres.

#### Références
- Guidage PMPG / CBPR+ pour les paiements dans les processus titres.  
- Pratiques de règlement SMPG, informations T2S sur les liaisons/holds.  
- Manuels Clearstream DCP, documentation ECMS pour les messages de maintenance.

### pacs.004 renvoie des notes de mappage

- Les appareils de retour normalisent désormais `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) et les motifs de retour propriétaires exposés comme `TxInf[*]/RtrdRsn/Prtry`, afin que les consommateurs de pont puissent rejouer l'attribution des frais et les codes d'opérateur sans réanalyser le XML. enveloppe.
- Les blocs de signature AppHdr à l'intérieur des enveloppes `DataPDU` restent ignorés lors de l'ingestion ; les audits doivent s'appuyer sur la provenance du canal plutôt que sur les champs XMLDSIG intégrés.### Liste de contrôle opérationnel pour le pont
- Faire respecter la chorégraphie ci-dessus (collatéral : `colr.010/011/012 → sese.023/024/025` ; violation FX : `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- Traiter les statuts `sese.024`/`sese.025` et les résultats `pacs.002` comme des signaux de déclenchement ; `ACSC` déclenche le déclenchement, `RJCT` force le déroulement.  
- Encodez la livraison conditionnelle via `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` et les règles CoSD facultatives.  
- Utilisez `SupplementaryData` pour corréler les identifiants externes (par exemple, UETR pour le `pacs.009`) si nécessaire.  
- Paramétrer les timings de maintien/dénouement par calendrier/cut-off du marché ; émettre `sese.030`/`camt.056` avant les délais d'annulation, recours aux retours si nécessaire.

### Exemples de charges utiles ISO 20022 (annotées)

#### Paire de substitution de garantie (`sese.023`) avec liaison d'instructions

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Soumettez l'instruction liée `SUBST-2025-04-001-B` (réception FoP d'une garantie de remplacement) avec `SctiesMvmntTp=RECE`, `Pmt=FREE` et le lien `WITH` pointant vers `SUBST-2025-04-001-A`. Relâchez les deux jambes avec un `sese.030` correspondant une fois la substitution approuvée.

#### Jambe de titres en attente de confirmation de change (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Relâchez une fois que les deux jambes `pacs.009` atteignent `ACSC` :

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` confirme la mainlevée de retenue, suivi de `sese.025` une fois la jambe titres réservée.#### Branche de financement PvP (`pacs.009` à finalité titres)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` suit l'état du paiement (`ACSC` = confirmé, `RJCT` = rejeté). Si la fenêtre est violée, rappelez via `camt.056`/`camt.029` ou envoyez `pacs.004` pour restituer les fonds réglés.