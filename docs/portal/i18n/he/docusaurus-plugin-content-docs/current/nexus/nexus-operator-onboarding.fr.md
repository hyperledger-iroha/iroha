---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operator-onboarding
כותרת: Integration des operateurs de data-space Sora Nexus
תיאור: Miroir de `docs/source/sora_nexus_operator_onboarding.md`, נמצא ברשימה של שחרור ה-Bout en bout pour les Operators Nexus.
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/sora_nexus_operator_onboarding.md`. Gardez les deux copies alignees jusqu'a l'arrivee des editions localisees sur le portal.
:::

# Integration des Operators de Data-space Sora Nexus

Ce guide capture le flux de bout en bout que les operators de data-space Sora Nexus doivent suivre une fois un release annonce. ספר ההפעלה המלא (`docs/source/release_dual_track_runbook.md`) וההערה לבחירה של חפצי אמנות (`docs/source/release_artifact_selection.md`) בהערה מותרת ליישר חבילות/תמונות טלפוניות, מניפסטים ותבניות קונפיגורציה עם תבניות גלובליות ונקודות אופנתיות.

## קהל ודרישה מוקדמת
- יש אישור לתוכנית Nexus והשפעה חוזרת של מרחב נתונים (אינדקס ליין, מזהה מרחב נתונים/כינוי ודרישות של מסלול פוליטי).
- Vous pouvez acceder aux artefacts signes du release publies par Release Engineering (כדורים, תמונות, מניפסטים, חתימות, מפרסמים).
- Vous avez genere ou recu le materiel de cles de production pour votre role de validator/observer (identite de noeud Ed25519; cle de consensus BLS + PoP pour les validators; plus tout toggle de fonctionnalite confidentielle).
- Vous pouvez joindre les pairs Sora Nexus existants qui bootstrappent votre noeud.

## Etape 1 - Confirmer le profil de release
1. זהה את ה-alias de reseau ou le chain ID qui vous a ete donne.
2. Lancez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) sur un checkout dece depot. Le helper consulte `release/network_profiles.toml` and imprime le profil a deployer. יוצקים סורה Nexus la reponse doit etre `iroha3`. Pour toute autre valeur, arretez et contactez Release Engineering.
3. Notez le tag de version reference par l'annonce du release (par exemple `iroha3-v3.2.0`); vous l'utiliserez pour recuperer les artefacts and manifests.

## Etape 2 - Recuperer et valider les artefacts
1. Telechargez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et ses fichiers compagnons (`.sha256`, optionnel `.sig/.pub`, `<profile>-<version>-manifest.json`, et I100000031X, ו-I18NI deployous מתחרים).
2. Validez l'integrite avant de decompresser:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` עם אישור אימות על ידי הארגון כדי להשתמש בחומר KMS.
3. Inspectez `PROFILE.toml` dans le tarball et les manifests JSON pour confirmer:
   - `profile = "iroha3"`
   - Les champs `version`, `commit` ו-`built_at` כתבו לפרסום.
   - מערכת ההפעלה/ארכיטקטורה תואמת את הבחירה.
4. אם אתה משתמש בתמונה למשתמש, חוזר על ה-hash/חתימה לאימות `<profile>-<version>-<os>-image.tar` ואישור מזהה התמונה שנרשם ב-`<profile>-<version>-image.json`.## סרט 3 - מכין תצורה של תבניות
1. Extrayez le bundle et copiez `config/` vers l'emplacement ou le noeud lira sa configuration.
2. Traitez les fichiers sous `config/` comme des templates:
   - Remplacez `public_key`/`private_key` par vos cles Ed25519 de production. Supprimez les cles privees du disque si le noeud les source depuis un HSM; mettez a jour la configuration pour pointer vers le connecteur HSM.
   - Ajustez `trusted_peers`, `network.address` ו-`torii.address` אפינ qu'ils refletent vos interfaces accessibles et les pars bootstrap assigns.
   - Mettez a jour `client.toml` avec l'endpoint Torii cote operateur (הכוללת תצורה TLS היא רלוונטית) והוראות זיהוי המזהים להפעלת ה-outillage.
3. שמור את מזהה הרשת ארבעת ההוראה מפורשת של הממשל - המסלול הגלובלי להשתתף ב-un identifiant de chaine canonique ייחודי.
4. Planifiez le demarrage du noeud avec le flag de profil Sora: `irohad --sora --config <path>`. טעינת התצורה לא קיימת.

## Etape 4 - יישר את המטא נתונים של מרחב נתונים ומסלול
1. Editez `config/config.toml` pour que la section `[nexus]` corresponde au catalog de data-space fourni par le Nexus Council:
   - `lane_count` doit egaler le total des lanes activees dans l'epoque courante.
   - Chaque entree dans `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` doit contenir un `index`/`id` unique et les alias convenus. Ne supprimez pas les entrees globales existantes; ajoutez vos alias delegues si le conseil תכונה של תוספות מרחבי נתונים.
   - Assurez-vous que chaque entree de dataspace כולל `fault_tolerance (f)`; les comites נתיב ממסר בגודל `3f+1`.
2. Mettez a jour `[[nexus.routing_policy.rules]]` pour capturer la politique qui vous a ete attribute. Le template par defaut route les instruktioner de gouvernance vers lane `1` et les deploiements de contrats vers lane `2`; ajoutez ou modifiez des regles pour que le trafic destine a votre data-space soit dirige vers la lane et l'alias מתקן. Coordonnez avec Release Engineering avant de changer l'ordre des regles.
3. Revoyez les seuils `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Les operators sont censes conserver les valeurs approuvees par le conseil; הייחודיות הייחודית היא אחת הפוליטיות המאושרת.
4. רשם את סיום התצורה ב-votre tracker d'operations. Le runbook de release a double voie exige d'attacher le `config.toml` effectif (סודות מחדש) au ticket d'onboarding.## Etape 5 - אימות לפני טיסה
1. בצע את אימות התצורה אינטגרה avant de rejoindre le reseau:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela imprime la configuration resolue et echoue to si les קטלוגים/מסלולים מנות ראשונות לא קוהרנטיות או התחלה והגדרות שונות.
2. Si vous deployez des conteneurs, executez la meme commande dans l'image apres l'avoir chargee avec `docker load -i <profile>-<version>-<os>-image.tar` (pensez a inclure `--sora`).
3. בדוק את הרישומים של מציין מיקום של נתיב/מרחב נתונים. אם כן, נשמח מאוד לסרטון 4 - הייצור של פריסות זה תלוי בתבניות מזהות.
4. בצע את הליך עשן מקומי (עמוד לדוגמא soumettre une requete `FindNetworkStatus` avec `iroha_cli`, מאשר נקודות קצה טלמטריות חשיפות `nexus_lane_state_total`, ואימות que les cles se tourne de streaming exoentes).

## Etape 6 - Cutover et hand-off
1. Stockez le `manifest.json` לאמת et les artefacts de signature dans le ticket de release pour que les auditeurs puissent reproduire vos verifications.
2. Informez Nexus Operations que le noeud est pret a etre introduit; כולל:
   - Identite du noeud (מזהה עמיתים, שמות מארח, נקודת קצה Torii).
   - יעילות בקטלוג נתיב/מרחב נתונים ופוליטיקה של מסלול.
   - Hashes des binaires/images מאמת.
3. Coordonnez l'admission final des pars (זרעי רכילות et affectation de lane) avec `@nexus-core`. Ne rejoignez pas le reseau avant d'avoir recu l'approbation; Sora Nexus אפליקציית une עיסוק קובע את הנתיבים ודרוש אישור קבלה לא מזמן.
4. Apres la mise en ligne du noeud, mettez a jour vos runbooks avec les overrides introduits et notez le tag de release pour que la prochaine iteration parte de cette baseline.

## רשימת רשימת התייחסות
- [ ] Profil de release valide comme `iroha3`.
- [ ] Hashes et signatures du bundle/image verifies.
- [ ] Cles, addresses de pairs et endpoints Torii mis a jour en valeurs production.
- [ ] נתיבי קטלוג/מרחב נתונים ופוליטיקה של מסלולים Nexus, כתב ל-l'affection du conseil.
- [ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) עובר ללא נמנע.
- [ ] מניפסטים/חתימות בארכיון ב-le ticket d'onboarding et Ops להודיע.

Pour un contexte plus large sur les phases de migration Nexus et les attentes de telemetrie, consultez [Nexus הערות מעבר](./nexus-transition-notes).