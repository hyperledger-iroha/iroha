---
lang: am
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e39dc94f52395bd9323177df1a7feeb7bbd4f9a3cdea07b02f9d60e7826e199e
source_last_modified: "2026-01-22T16:26:46.506936+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
slug: /norito/quickstart
translator: machine-google-reviewed
---

ይህ የእግር ጉዞ ገንቢዎች ሲማሩ እንዲከተሏቸው የምንጠብቀውን የስራ ሂደት ያንጸባርቃል
Norito እና I18NT0000000X ለመጀመሪያ ጊዜ፡ የሚወስን ነጠላ-አቻ ኔትወርክን አስነሳ፣
ውል ማጠናቀር፣ በአገር ውስጥ ማድረቅ፣ ከዚያም በTorii በ
ማጣቀሻ CLI.

የምሳሌ ኮንትራቱ እርስዎ እንዲችሉ የቁልፍ/ዋጋ ጥንድ ወደ የደዋይ መለያ ይጽፋል
የጎንዮሽ ጉዳቱን ወዲያውኑ በ I18NI0000039X ያረጋግጡ።

## ቅድመ ሁኔታዎች

- [Docker](https://docs.docker.com/engine/install/) ከጽሑፍ V2 ጋር የነቃ (ጥቅም ላይ የዋለ)
  በ `defaults/docker-compose.single.yml` ውስጥ የተገለጸውን የናሙና አቻ ለመጀመር)።
- ካላወረዱ የረዳት ሁለትዮሽዎችን ለመገንባት ዝገት የመሳሪያ ሰንሰለት (1.76+)
  የታተሙት.
- `koto_compile`፣ `ivm_run`፣ እና `iroha_cli` ሁለትዮሽ። ከ ሊገነቡዋቸው ይችላሉ
  የስራ ቦታ ፍተሻ ከዚህ በታች እንደሚታየው ወይም ተዛማጅ የሆኑትን የመልቀቂያ ቅርሶች ያውርዱ፡

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ከላይ ያሉት ሁለትዮሽዎች ከተቀረው የስራ ቦታ ጋር ለመጫን ደህና ናቸው።
> ከ `serde`/I18NI0000045X ጋር ፈጽሞ አይገናኙም; Norito ኮዴኮች ከጫፍ እስከ ጫፍ ተፈጻሚ ናቸው።

## 1. ነጠላ-አቻ ዴቭ አውታረ መረብ ይጀምሩ

ማከማቻው በ`kagami swarm` የተፈጠረ Docker ጻፍ ጥቅል ያካትታል
(`defaults/docker-compose.single.yml`)። ነባሪውን ዘፍጥረት፣ ደንበኛን ያገናኛል።
ውቅረት ፣ እና የጤና ምርመራዎች ስለዚህ Torii በ `http://127.0.0.1:8080` ሊደረስ ይችላል።

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

ኮንቴይነሩ እየሮጠ ይተውት (በፊት ለፊት ወይም በተናጥል)። ሁሉም
ቀጣይ የCLI ጥሪዎች ይህንን እኩያ በ`defaults/client.toml` ኢላማ ያደርጋሉ።

## 2. ውሉን አዘጋጅ

የስራ ማውጫ ይፍጠሩ እና አነስተኛውን Kotodama ምሳሌ ያስቀምጡ፡

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> የI18NT0000003X ምንጮችን በስሪት ቁጥጥር ውስጥ ማስቀመጥን እመርጣለሁ። በፖርታል የሚስተናገዱ ምሳሌዎች ናቸው።
> እንዲሁም በ[Norito ምሳሌዎች ማዕከለ-ስዕላት](./examples/) ስር ይገኛል።
> የበለፀገ መነሻ ይፈልጋሉ።

## 3. በ IVM ማጠናቀር እና ማድረቅ

ውሉን ወደ IVM/Norito ባይትኮድ (`.to`) በማጠናቀር በአገር ውስጥ ለመፈጸም
አውታረ መረቡን ከመንካትዎ በፊት አስተናጋጅ ሲሳይሎች ስኬታማ መሆናቸውን ያረጋግጡ፡-

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ሯጩ የ I18NI0000051X ሎግ ያትማል እና ያከናውናል
`SET_ACCOUNT_DETAIL` syscall በተሳለቀበት አስተናጋጅ ላይ። አማራጭ I18NI0000053X ከሆነ
ሁለትዮሽ አለ፣ I18NI0000054X ያሳያል
ABI ራስጌ፣ የባህሪ ቢትስ እና ወደ ውጭ የተላኩ የመግቢያ ነጥቦች።

## 4. በ Torii በኩል ባይትኮድ ያስገቡ

መስቀለኛ መንገዱ አሁንም እየሄደ እያለ፣ CLI ን በመጠቀም የተቀናበረውን ባይትኮድ ወደ Torii ይላኩ።
ነባሪው የልማት ማንነት ከህዝብ ቁልፍ የተወሰደ ነው።
`defaults/client.toml`፣ ስለዚህ የመለያ መታወቂያው ነው።
```
<katakana-i105-account-id>
```

Torii URL፣ ሰንሰለት መታወቂያ እና የመፈረሚያ ቁልፍ ለማቅረብ የውቅር ፋይሉን ይጠቀሙ፡-

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI ግብይቱን በNorito ያስቀምጣል፣ በዴቭ ቁልፍ ይፈርማል እና
ለሩጫ እኩያ ያቀርባል። ለI18NI0000056X የDocker ምዝግብ ማስታወሻዎችን ይመልከቱ
ለተፈጸመው የግብይት ሃሽ የ CLI ውፅዓት ማመሳሰል ወይም መከታተል።

## 5. የግዛቱን ለውጥ ያረጋግጡ

ውሉ የጻፈውን የመለያ ዝርዝር ለማግኘት ተመሳሳዩን የCLI መገለጫ ይጠቀሙ፡-

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

በNorito የሚደገፍ የJSON ጭነትን ማየት አለቦት፡

```json
{
  "hello": "world"
}
```

እሴቱ ከጠፋ፣ የDocker ድርሰት አገልግሎት አሁንም መሆኑን ያረጋግጡ
እየሄደ እና በ`iroha` የተዘገበው የግብይት ሃሽ `Committed` ደርሷል።
ሁኔታ.

## ቀጣይ እርምጃዎች

- ለማየት በራስ የመነጨውን [ለምሳሌ ጋለሪ](./examples/) ያስሱ
  ምን ያህል የላቀ Kotodama ቅንጥቦች ካርታ ወደ Norito syscals.
- ለበለጠ ጥልቀት [Norito የመነሻ መመሪያ](./getting-started) ያንብቡ
  የአቀናባሪ/ሯጭ መገልገያ፣ የሰነድ ዝርዝር መግለጫ እና የI18NT0000025X ማብራሪያ
  ሜታዳታ
- በራስዎ ኮንትራቶች ላይ ሲደጋገሙ, በ ውስጥ `npm run sync-norito-snippets` ይጠቀሙ
  ፖርታል ሰነዶች እና ቅርሶች እንዲቆዩ ሊወርዱ የሚችሉ ቅንጣቢዎችን ለማደስ የስራ ቦታ
  በ `crates/ivm/docs/examples/` ስር ካሉት ምንጮች ጋር በማመሳሰል።