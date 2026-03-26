---
lang: he
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Parcours du registre
תיאור: Reproduisez un flux deterministe register -> מנטה -> העברה עם CLI `iroha` ואימות תוצאות החשבונות.
slug: /norito/ledger-walkthrough
---

Ce parcours complete le [Quickstart Norito](./quickstart.md) en montrant comment modifier et inspecter l'etat du Ledger avec le CLI `iroha`. Vous enregistrerez une nouvelle definition d'actif, minterez des unites sur le compte operationur par defaut, transfererez une partie du solde vers un autre compte et verifierez les transactions and avoirs resultants. Chaque etape reflete les flux couverts par les quickstarts SDK Rust/Python/JavaScript efin de confirmer la parite entre le CLI et le comportement des SDK.

## תנאי מוקדם

- Suivez le [Quickstart](./quickstart.md) pour demarrer le reseau mono-pair via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (le CLI) est construit ou telecharge et que vous pouvez
  joindre le peer avec `defaults/client.toml`.
- כולל אפשרויות אופציות: `jq` (פורמט של תגובות JSON) et un shell POSIX pour les
  קטעי משתנים של הסביבה ci-dessous.

Tout au long du guide, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` par les
תעודות זהות שמשמשות למשתמשים. Le bundle par defaut inclut deja deux comptes
issus des cles demo :

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

אשרת את הבחירה ב-listant les premiers מתחרה:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecter l'etat genesis

התחלת לחקור את ה- Ledger Cible עבור CLI:

```sh
# Domains enregistres en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dans wonderland (remplacez --limit par un nombre plus eleve si besoin)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions qui existent deja
iroha --config defaults/client.toml asset definition list all --table
```

Ces commandes reposent sur des reponses Norito, donc le filtrage et la pagetion sont
 deterministes et correspondent a ce que recoivent les SDK.

## 2. Enregistrer une definition d'actif

Creez un nouvel actif infiniment mintable appele `coffee` dans le domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Le CLI affiche le hash de transaction soumis (לדוגמה, `0x5f...`). קונסרבז-לה
pour consulter le statut plus tard.

## 3. Minter des unites sur le compte operateur

Les quantites d'actifs vivent sous la paire `(asset definition, account)`. Mintez 250
מאחד את `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` ב-`$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

הדרן une fois, recuperez le hash de transaction (`$MINT_HASH`) depuis la sortie du CLI. יוצקים
מאמת le solde, executez:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler seulement le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. מעביר une partie du solde vers un autre compte

Deplacez 50 מאחד את מפעילי החברה לעומת `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Sauvegardez le hash de transaction comme `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
pour Verifier Les Nouveaux Soldes:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifier les preuves du ledger

Utilisez les hashes sauvegardes pour confirmer que les deux עסקאות על ועדות:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Vous pouvez aussi streamer les blocks האחרונים pour voir quel bloc a inclusive the transfert :

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Toutes les commandes ci-dessus utilisent les memes payloads Norito que les SDK. אני בטוח
reproduisez ce flux via du code (voir les quickstarts SDK ci-dessous), les hashes et
Soldes Seront מיישר את ברירת המחדל של tant que vous ciblez le meme reseau et les memes.

## Liens de parite SDK

- [התחלה מהירה של SDK של חלודה](../sdks/rust) - הוראות הרשמה לרישום,
  la soumission de transaktions et le polling de statut depuis Rust.
- [התחלה מהירה של Python SDK](../sdks/python) - montre les memes operation register/mint
  avec des helpers JSON משתמש ב-Norito.
- [התחלה מהירה של JavaScript SDK](../sdks/javascript) - couvre les requetes Torii,
  les helpers de gouvernance et les wrappers de requetes typeses.

בצע את ההליכה של CLI, תרחיש חוזר עם ה-SDK
prefere pour vous assurer que les deux surfaces s'accordent sur les hashes de
עסקאות, les soldes et les resultats de requetes.