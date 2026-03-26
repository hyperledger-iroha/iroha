---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito ici

Le code d'accès Kotodama est utilisé pour le bytecode Norito. Il s'agit d'un produit de qualité pour Iroha qui est en train de s'en servir. دکھاتا ہے۔

## ضروریات

1. Chaîne d'outils Rust (1,76 pouces) Version améliorée de Rust Toolchain (1,76 pouces)
2. معاون بائنریز کو بنائیں یا ڈاؤن لوڈ کریں:
   - `koto_compile` - Kotodama bytecode et IVM/Norito bytecode par défaut
   - `ivm_run` et `ivm_tool` - مقامی اجرا اور معائنہ یوٹیلٹیز
   - `iroha_cli` - Torii کے ذریعے کنٹریکٹ ڈپلائے کرنے کے لئے استعمال ہوتا ہے

   Il s'agit d'un Makefile pour `PATH` qui est en cours de création. Il s'agit d'artefacts qui sont en vente dans le monde entier. Il s'agit d'une chaîne d'outils qui contient également des assistants Makefile qui sont également des outils :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. جب آپ ڈپلائمنٹ مرحلے تک پہنچیں تو یقینی بنائیں کہ Iroha نوڈ چل رہا ہو۔ Il s'agit d'une adresse URL Torii pour votre URL `iroha_cli`. پروفائل (`~/.config/iroha/cli.toml`) میں سیٹ ہے۔

## 1. Kotodama کنٹریکٹ کمپائل کریں

Il s'agit d'un "bonjour tout le monde" `examples/hello/hello.ko` en anglais Le bytecode Norito/IVM (`.to`) contient le code suivant :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اہم فلیگز:- `--abi 1` کنٹریکٹ کو ABI ورژن 1 پر لاک کرتا ہے (تحریر کے وقت واحد سپورٹڈ ورژن).
- `--max-cycles 0` لامحدود اجرا کی درخواست کرتا ہے؛ preuves de connaissance nulle pour le remplissage du cycle et pour les preuves de connaissances nulles

## 2. Norito آرٹیفیکٹ کا معائنہ (اختیاری)

Il s'agit d'un modèle de référencement `ivm_tool`:

```sh
ivm_tool inspect target/examples/hello.to
```

Il y a des drapeaux et des points d'entrée de l'ABI et des points d'entrée. یہ ڈپلائمنٹ سے پہلے ایک فوری contrôle de santé mentale ہے۔

## 3. کنٹریکٹ کو مقامی طور پر چلائیں

`ivm_run` est un bytecode qui est en cours de lecture:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` Appel système pour `SET_ACCOUNT_DETAIL` Appel système `SET_ACCOUNT_DETAIL` Appel système Il s'agit d'une chaîne de distribution en chaîne pour les clients en ligne. ہوں۔

## 4. `iroha_cli` کے ذریعے ڈپلائے کریں

جب آپ کنٹریکٹ سے مطمئن ہوں تو CLI کے ذریعے اسے نوڈ پر ڈپلائے کریں۔ L'autorité utilise la clé de signature pour `.to` pour la charge utile Base64:

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Il s'agit du manifeste Torii et du manifeste Norito + paquet de bytecode. حیثیت پرنٹ کرتی ہے۔ Il s'agit d'un commit ou d'un code de hachage qui manifeste des instances ou des instances. استعمال کیا جا سکتا ہے:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii est en cours de réalisationbytecode رجسٹر ہونے کے بعد، آپ ایک instruction submit کر کے اسے کال کر سکتے ہیں جو محفوظ شدہ کوڈ کو حوالہ دے (مثلا `iroha_cli ledger transaction submit` یا آپ کے ایپ کلائنٹ کے ذریعے). Vous avez besoin d'autorisations pour les appels système (`set_account_detail`, `transfer_asset`, etc.) pour les appels système (`set_account_detail`, `transfer_asset`, etc.) ہیں۔

## تجاویز اور خرابیوں کا حل

- `make examples-run` استعمال کریں تاکہ فراہم کردہ مثالیں ایک ہی قدم میں کمپائل اور چل سکیں۔ Utiliser `PATH` pour remplacer les variables d'environnement `KOTO`/`IVM` et remplacer les variables d'environnement
- اگر `koto_compile` ABI ورژن مسترد کرے تو تصدیق کریں کہ کمپائلر اور نوڈ دونوں ABI v1 کو ہدف بنا رہے ہیں (`koto_compile --abi` بغیر دلائل کے چلائیں تاکہ سپورٹ دکھے).
- CLI hex et clés de signature Base64 pour les utilisateurs ٹیسٹنگ کے لئے `iroha_cli tools crypto keypair` سے نکلے ہوئے touches استعمال کیے جا سکتے ہیں۔
- Charges utiles Norito pour `ivm_tool disassemble` pour les charges utiles Kotodama سورس سے جوڑا جا سکے۔

یہ فلو CI اور انٹیگریشن ٹیسٹس میں استعمال ہونے والے مراحل کی عکاسی کرتا ہے۔ Les mappages d'appels système Kotodama et les composants internes Norito sont également compatibles avec les éléments suivants :

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`