---
slug: /norito/ledger-walkthrough
lang: am
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ይህ የእግር ጉዞ በማሳየት [Norito quickstart](./quickstart.md) ያሟላል።
በ `iroha` CLI የመመዝገቢያ ሁኔታን እንዴት መቀየር እና መመርመር እንደሚቻል። ትመዘገባለህ ሀ
አዲስ የንብረት ትርጉም ፣ አንዳንድ ክፍሎችን ወደ ነባሪ የኦፕሬተር መለያ ፣ ማስተላለፍ
የሂሳቡ አካል ወደ ሌላ መለያ, እና የተገኙትን ግብይቶች ያረጋግጡ
እና መያዣዎች. እያንዳንዱ እርምጃ በ Rust/Python/JavaScript ውስጥ የተሸፈኑትን ፍሰቶች ያንጸባርቃል
በCLI እና በኤስዲኬ ባህሪ መካከል ያለውን እኩልነት ለማረጋገጥ ኤስዲኬ በፍጥነት ይጀምራል።

## ቅድመ ሁኔታዎች

ነጠላ-አቻ አውታረመረብን ለመጀመር [ፈጣን ጅምር](./quickstart.md)ን ይከተሉ።
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) መገንባቱን ወይም መጫኑን እና እርስዎ መድረስ እንደሚችሉ ያረጋግጡ።
  እኩያ `defaults/client.toml` በመጠቀም።
- አማራጭ ረዳቶች፡- `jq` (የJSON ምላሾችን መቅረጽ) እና ለ POSIX ሼል
  ከዚህ በታች ጥቅም ላይ የዋሉ የአካባቢ-ተለዋዋጭ ቁርጥራጮች።

በመመሪያው ውስጥ፣ `$ADMIN_ACCOUNT` እና `$RECEIVER_ACCOUNT`ን በ
ለመጠቀም ያቀዱት የመለያ መታወቂያዎች። የነባሪዎች ቅርቅብ አስቀድሞ ሁለት መለያዎችን ያካትታል
ከማሳያ ቁልፎች የተወሰደ፡-

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

የመጀመሪያዎቹን ጥቂት መለያዎች በመዘርዘር እሴቶቹን ያረጋግጡ፡-

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. የጄኔሲስ ሁኔታን ይፈትሹ

CLI እያነጣጠረ ያለውን የሂሳብ መዝገብ በማሰስ ይጀምሩ፡-

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

እነዚህ ትዕዛዞች በI18NT0000001X የሚደገፉ ምላሾች ላይ ይመረኮዛሉ፣ ስለዚህ ማጣራት እና ማጣራት
የሚወስን እና ኤስዲኬዎች ከሚቀበሉት ጋር ይዛመዳሉ።

## 2. የንብረት ፍቺ ያስመዝግቡ

በ`wonderland` ውስጥ `coffee` የሚባል አዲስ ፣ ማለቂያ የሌለው የማይታወቅ ንብረት ይፍጠሩ
ጎራ፡

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI የገባውን የግብይት ሃሽ ያትማል (ለምሳሌ፡-
`0x5f…`). ሁኔታውን በኋላ ለመጠየቅ እንዲችሉ ያስቀምጡት።

## 3. ሚንት ክፍሎች ወደ ኦፕሬተር መለያ

የንብረት መጠን በ`(asset definition, account)` ጥንድ ስር ይኖራሉ። ሚንት 250
የ `coffee#wonderland` ወደ I18NI0000033X ክፍሎች:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

እንደገና፣ የግብይቱን ሃሽ (`$MINT_HASH`) ከCLI ውፅዓት ይያዙ። ለ
ሚዛኑን እንደገና ያረጋግጡ ፣ ያሂዱ

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ወይም፣ አዲሱን ንብረት ብቻ ለማነጣጠር፡-

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. ቀሪውን የተወሰነ ክፍል ወደ ሌላ መለያ ያስተላልፉ

50 ክፍሎችን ከኦፕሬተር መለያ ወደ `$RECEIVER_ACCOUNT` ይውሰዱ፡

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

የግብይቱን ሃሽ እንደ `$TRANSFER_HASH` ያስቀምጡ። በሁለቱም ላይ መያዣዎችን ይጠይቁ
አዲሱን ሂሳቦች ለማረጋገጥ መለያዎች፡-

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. የመመዝገቢያ ማስረጃዎችን ያረጋግጡ

ሁለቱም ግብይቶች መፈጸማቸውን ለማረጋገጥ የተቀመጡትን ሃሽ ይጠቀሙ፡-

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

እንዲሁም የትኛው ብሎክ ዝውውሩን እንደጨመረ ለማየት የቅርብ ጊዜ ብሎኮችን በዥረት መልቀቅ ይችላሉ፡

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

ከላይ ያለው እያንዳንዱ ትዕዛዝ ልክ እንደ ኤስዲኬዎች I18NT0000002X የክፍያ ጭነቶች ይጠቀማል። ብተደጋጋሚ
ይህ ፍሰት በኮድ በኩል (ከታች ያለውን የኤስዲኬ ፈጣን ጅምር ይመልከቱ)፣ ሃሽ እና ሚዛኖች ይሆናሉ
ተመሳሳዩን አውታረ መረብ እና ነባሪዎችን እስካላነጣጠሩ ድረስ መስመር ያድርጉ።

## የኤስዲኬ እኩልነት አገናኞች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](../sdks/rust) - የመመዝገቢያ መመሪያዎችን ያሳያል ፣
  ግብይቶችን ማስገባት, እና የምርጫ ሁኔታን ከዝገት.
- [Python SDK quickstart](../sdks/python) - ተመሳሳይ መመዝገቢያ/mint ያሳያል
  በNorito የሚደገፉ የJSON አጋዥዎች ያሉ ስራዎች።
- [JavaScript SDK quickstart](../sdks/javascript) - የTorii ጥያቄዎችን ይሸፍናል፣
  የአስተዳደር ረዳቶች እና የተተየቡ የጥያቄ መጠቅለያዎች።

መጀመሪያ የCLI መራመጃውን ያሂዱ፣ ከዚያ በመረጡት ኤስዲኬ ሁኔታውን ይድገሙት
ሁለቱም ገጽታዎች በግብይት hashes፣ ሒሳቦች እና መጠይቅ ላይ መስማማታቸውን ለማረጋገጥ
ውጤቶች.