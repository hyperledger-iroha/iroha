---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK Quickstart

`@iroha2/torii-client` አሳሽ እና Node.js ተስማሚ መጠቅለያ በ Torii ዙሪያ ያቀርባል።
ይህ ፈጣን ጅምር ከኤስዲኬ የምግብ አዘገጃጀቶች ዋና ፍሰቶችን ያንፀባርቃል ስለዚህ እርስዎ ማግኘት ይችላሉ።
ደንበኛ በጥቂት ደቂቃዎች ውስጥ እየሄደ ነው። ለተጨማሪ ምሳሌዎች፣ ይመልከቱ
`javascript/iroha_js/recipes/` በማከማቻው ውስጥ።

## 1. ጫን

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

ግብይቶችን በአገር ውስጥ ለመፈረም ካቀዱ፣ እንዲሁም የ crypto አጋዥዎችን ይጫኑ፡-

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. የTorii ደንበኛ ይፍጠሩ

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

አወቃቀሩ በአዘገጃጀቱ ውስጥ ጥቅም ላይ የዋለውን ገንቢ ያንጸባርቃል. የእርስዎ አንጓ ከሆነ
መሰረታዊ auth ይጠቀማል፣ በ`basicAuth` አማራጭ በኩል `{username, password}` ማለፍ።

## 3. የመስቀለኛ መንገድ ሁኔታን አምጣ

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

ሁሉም የተነበቡ ስራዎች በNorito የሚደገፉ የJSON ነገሮችን ይመለሳሉ። የተፈጠሩትን ዓይነቶች ይመልከቱ
ለመስክ ዝርዝሮች `index.d.ts`።

## 4. ግብይት አስገባ

ፈራሚዎች በረዳት ኤፒአይ ግብይቶችን መገንባት ይችላሉ፡-

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

ረዳቱ በቀጥታ በሚጠበቀው Norito ኤንቨሎፕ ውስጥ ግብይቱን ያጠቃልለዋል።
በ Torii. ለበለጸገ ምሳሌ (የመጨረሻውን መጠበቅን ጨምሮ) ይመልከቱ
`javascript/iroha_js/recipes/registration.mjs`.

## 5. ከፍተኛ ደረጃ ረዳቶችን ይጠቀሙ

ኤስዲኬ CLI ን የሚያንፀባርቁ ልዩ ፍሰቶችን ይሰበስባል፡-

- ** የመንግስት ረዳቶች *** - `recipes/governance.mjs` ዝግጅት ያሳያል
  ከ`governance` የመመሪያ ገንቢዎች ጋር ፕሮፖዛል እና ምርጫዎች።
- ** ISO ድልድይ *** - `recipes/iso_bridge.mjs` እንዴት `pacs.008` እና ማስገባት እንደሚቻል ያሳያል
  የ`/v1/iso20022` የመጨረሻ ነጥቦችን በመጠቀም የምርጫ ማስተላለፍ ሁኔታ።
- **SoraFS እና ቀስቅሴዎች** - በ `src/toriiClient.js` ስር የገጽታ ረዳቶች ያጋልጣሉ
  ለኮንትራቶች፣ ንብረቶች፣ ቀስቅሴዎች እና SoraFS አቅራቢዎች የተተየቡ ተደጋጋሚ።

እነዚያን ፍሰቶች እንደገና ለመጠቀም ከ`@iroha2/torii-client` ተዛማጅ ገንቢ ተግባራትን ያስመጡ።

## 6. አያያዝ ላይ ስህተት

ሁሉም የኤስዲኬ ጥሪዎች የበለጸጉ `ToriiClientError` ምሳሌዎችን ከትራንስፖርት ዲበዳታ ጋር ይጥላሉ
እና Norito የስህተት ክፍያ። ጥሪዎችን በ`try/catch` ጠቅለል ያድርጉ ወይም ለ `.catch()` ይጠቀሙ
የወለል አውድ ለተጠቃሚዎች፡-

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## ቀጣይ እርምጃዎች

- ከጫፍ እስከ ጫፍ ፍሰቶችን በ`javascript/iroha_js/recipes/` ውስጥ ያሉትን የምግብ አዘገጃጀቶች ያስሱ።
- በ `javascript/iroha_js/index.d.ts` ውስጥ የተፈጠሩትን ዓይነቶች ለዝርዝር ያንብቡ
  ዘዴ ፊርማዎች.
- ሸክሞችን ለመመርመር እና ለማረም ይህንን ኤስዲኬ ከNorito ፈጣን ጅምር ጋር ያጣምሩ
  ወደ Torii ይልካሉ።