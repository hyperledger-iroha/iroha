---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : دستخطی تقریب کی جگہ نیا عمل
description : Sora est un appareil de chunker SoraFS pour un appareil photo (SF-1b).
sidebar_label : دستخطی تقریب
---

> Feuille de route : **SF-1b — Approbations des appareils du Parlement Sora.**
> پارلیمنٹ کا ورک فلو پرانی آف لائن "کونسل دستخطی تقریب" کی جگہ لیتا ہے۔

SoraFS chunker luminaires pour la maison et le bureau اب تمام منظوری
**Sora Parliament** est un système de DAO basé sur le tri et Nexus.
Les obligations XOR sont liées aux obligations XOR.
Les rencontres en chaîne et les rencontres en ligne sont organisées par des joueurs en ligne.
Il s'agit d'outils de développement pour les développeurs.

## پارلیمنٹ کا جائزہ

- **شہریت** — آپریٹرز مطلوبہ XOR bond کر کے شہری بنتے ہیں اور trition کے اہل ہوتے ہیں۔
- **پینلز** — ذمہ داریاں گردش کرنے والے پینلز میں تقسیم ہیں (Infrastructure,
  Modération, Trésorerie, ...). Approbations des luminaires du panneau d'infrastructure SoraFS pour plus de détails
- **Sortition et rotation** — پینل سیٹس پارلیمنٹ دستور میں متعین cadence پر دوبارہ
  قرعہ اندازی سے منتخب ہوتے ہیں تاکہ کوئی ایک گروہ منظوریوں پر اجارہ داری نہ رکھ سکے۔

## Flux d'approbation des luminaires

1. **Soumission de la proposition**
   - Tooling WG avec `manifest_blake3.json` bundle et diff de montage et `sorafs.fixtureProposal`
     Un registre en chaîne est disponible pour vous
   - پروپوزل BLAKE3 digest, version sémantique اور تبدیلی نوٹس ریکارڈ کرتا ہے۔
2. **Révision et vote**
   - Panneau d'infrastructure pour la file d'attente des tâches et pour la file d'attente des tâches
   - Il existe des artefacts CI et des tests de parité ainsi que des votes pondérés en chaîne et des votes pondérés en chaîne.
3. **Finalisation**
   - Un quorum est nécessaire pour l'événement d'approbation d'exécution et un résumé canonique du manifeste
     La charge utile du luminaire et l'engagement de Merkle sont également disponibles.
   - یہ événement SoraFS registre میں miroir کیا جاتا ہے تاکہ کلائنٹس تازہ ترین Manifeste approuvé par le Parlement حاصل کر سکیں۔
4. **Distribution**
   - Aides CLI (`cargo xtask sorafs-fetch-fixture`) Nexus RPC pour le manifeste manifeste
     Il s'agit des constantes JSON/TS/Go `export_vectors` pour le résumé et le résumé en chaîne.
     مقابل validate کر کے رہتے ہیں۔

## Flux de travail du développeur

- Calendrier des matchs :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Le Parlement va chercher l'aide pour vérifier la signature de l'enveloppe et vérifier les signatures.
  Actualisation des luminaires de اور مقامی `--signatures` Le Parlement a reçu une enveloppe en forme de lettre
  helper متعلقہ manifest solve کرتا ہے، BLAKE3 digest دوبارہ حساب کرتا ہے، اور canonique
  Profil `sorafs.sf1@1.0.0` نافذ کرتا ہے۔

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Il s'agit du manifeste et de l'URL du formulaire `--manifest`. غیر دستخط شدہ enveloppes رد کر دیے جاتے ہیں
جب تک مقامی fumée coule کے لیے `--allow-unsigned` سیٹ نہ ہو۔

- La passerelle de transfert de données manifeste valide la validation des charges utiles pour Torii et contient les éléments suivants :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```- Liste des membres du CI `signer.json` pour la liste des membres
  `ci/check_sorafs_fixtures.sh` repo est un engagement en chaîne pour un engagement en chaîne
  فرق ہونے پر fail کر دیتا ہے۔

## Notes de gouvernance

- Pour le quorum, la rotation et l'escalade et la gouvernance - configuration au niveau de la caisse
- Rollbacks en arrière sur le panneau de modération Le panneau d'infrastructure est rétabli
  proposition فائل کرتا ہے جو پچھلے manifeste digest کو حوالہ دیتا ہے، اور منظوری کے بعد release بدل دی جاتی ہے۔
- Approbations du registre SoraFS pour replay médico-légal

##FAQ

- **`signer.json` کہاں گیا؟**  
  اسے ہٹا دیا گیا ہے۔ Attribution du signataire en chaîne موجود ہے؛ ریپو میں `manifest_signatures.json`
  صرف développeur luminaire ہے جو آخری événement d'approbation سے میچ ہونا چاہیے۔

- **کیا اببھی مقامی Ed25519 signatures درکار ہیں؟**  
  نہیں۔ Le Parlement approuve les artefacts en chaîne مقامی prochains matchs
  reproductibilité کے لیے ہوتے ہیں مگر Recueil du Parlement کے خلاف valider کیے جاتے ہیں۔

- **ٹیمیں approbations کیسے مانیٹر کرتی ہیں؟**  
  Événement `ParliamentFixtureApproved` pour s'abonner et Nexus RPC pour le registre et la requête
  تاکہ موجودہ manifeste digest اور panel roll call حاصل کیا جا سکے۔