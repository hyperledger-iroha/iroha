---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK အမြန်စတင်ပါ။

`@iroha2/torii-client` သည် ဘရောက်ဆာတစ်ခုနှင့် Torii ပတ်ပတ်လည်တွင် Node.js ဖော်ရွေသော ထုပ်ပိုးမှုကို ပံ့ပိုးပေးသည်။
ဤအမြန်စတင်ခြင်းသည် SDK ချက်ပြုတ်နည်းများမှ ပင်မစီးဆင်းမှုများကို ထင်ဟပ်စေသောကြောင့် သင်တစ်ခုရနိုင်သည်။
မိနစ်အနည်းငယ်အတွင်း client လည်ပတ်နေသည်။ ပိုမိုပြည့်စုံသော ဥပမာများအတွက် ကြည့်ပါ။
သိုလှောင်ခန်းရှိ `javascript/iroha_js/recipes/`။

## 1. ထည့်သွင်းပါ။

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

သင်သည် ပြည်တွင်း၌ ငွေပေးငွေယူများကို လက်မှတ်ထိုးရန် စီစဉ်ပါက၊ crypto helpers ကိုလည်း ထည့်သွင်းပါ-

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii client တစ်ခု ဖန်တီးပါ။

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

ဖွဲ့စည်းမှုပုံစံသည် ဟင်းချက်နည်းများတွင် အသုံးပြုသည့် constructor ကို ထင်ဟပ်စေသည်။ မင်းရဲ့ node ဆိုရင်
အခြေခံ auth ကိုအသုံးပြု၍ `basicAuth` ရွေးချယ်မှုမှတစ်ဆင့် `{username, password}` ကို ဖြတ်ပါ။

## 3. Fetch node အနေအထား

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

ဖတ်ပြီးသော လုပ်ဆောင်ချက်များအားလုံးသည် Norito-ကျောထောက်နောက်ခံ JSON အရာဝတ္ထုများကို ပြန်ပေးသည်။ ထုတ်လုပ်ထားသော အမျိုးအစားများကို ကြည့်ရှုပါ။
အကွက်အသေးစိတ်အတွက် `index.d.ts`။

## 4. ငွေပေးငွေယူတစ်ခု တင်သွင်းပါ။

လက်မှတ်ထိုးသူများသည် helper API ဖြင့် ငွေပေးငွေယူများကို တည်ဆောက်နိုင်သည်-

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'i105...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

အကူအညီပေးသူက မျှော်လင့်ထားသည့် Norito စာအိတ်တွင် ငွေပေးငွေယူကို အလိုအလျောက် ထုပ်ပေးသည်
Torii ဖြင့် ပိုမိုကြွယ်ဝသော ဥပမာတစ်ခုအတွက် (နောက်ဆုံးအား စောင့်ဆိုင်းခြင်းအပါအဝင်) ကိုကြည့်ပါ။
`javascript/iroha_js/recipes/registration.mjs`။

## 5. အဆင့်မြင့် အထောက်အကူများကို သုံးပါ။

SDK အစုအဝေးများသည် CLI ကိုထင်ဟပ်စေသော အထူးပြုစီးဆင်းမှုများ-

- ** အုပ်ချုပ်မှုအထောက်အကူများ** – `recipes/governance.mjs` သရုပ်ပြသည်
  `governance` ညွှန်ကြားချက်တည်ဆောက်သူများနှင့်အတူ အဆိုပြုချက်များနှင့် မဲများ။
- **ISO တံတား** – `recipes/iso_bridge.mjs` `pacs.008` တင်သွင်းပုံနှင့်
  `/v1/iso20022` အဆုံးမှတ်များကို အသုံးပြု၍ စစ်တမ်းလွှဲပြောင်းမှုအခြေအနေ။
- **SoraFS & triggers** – `src/toriiClient.js` အောက်တွင် Pagination helpers
  စာချုပ်များ၊ ပိုင်ဆိုင်မှုများ၊ အစပျိုးမှုများနှင့် SoraFS ပံ့ပိုးပေးသူများအတွက် iterators

ထိုစီးဆင်းမှုများကို ပြန်လည်အသုံးပြုရန်အတွက် သက်ဆိုင်ရာ တည်ဆောက်သူလုပ်ဆောင်ချက်များကို `@iroha2/torii-client` မှ တင်သွင်းပါ။

## 6. ကိုင်တွယ်မှုအမှား

SDK ခေါ်ဆိုမှုအားလုံးသည် သယ်ယူပို့ဆောင်ရေး မက်တာဒေတာဖြင့် ကြွယ်ဝသော `ToriiClientError` ဖြစ်ရပ်များကို လွှင့်ပစ်သည်။
နှင့် Norito အမှား payload ။ ခေါ်ဆိုမှုများကို `try/catch` တွင် ချုပ်ပါ သို့မဟုတ် `.catch()` ကို အသုံးပြုပါ
သုံးစွဲသူများအတွက် မျက်နှာပြင်ဆက်စပ်မှု-

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## နောက်တစ်ဆင့်

- အဆုံးမှအဆုံးအထိစီးဆင်းမှုအတွက် `javascript/iroha_js/recipes/` တွင် ချက်ပြုတ်နည်းများကို စူးစမ်းပါ။
- အသေးစိတ်အတွက် `javascript/iroha_js/index.d.ts` တွင် ထုတ်လုပ်ထားသော အမျိုးအစားများကို ဖတ်ပါ။
  နည်းလမ်းလက်မှတ်များ။
- ဤ SDK ကို Norito အမြန်စတင်ခြင်းဖြင့် ပါ၀င်မှုအား စစ်ဆေးခြင်းနှင့် အမှားရှာပြင်ခြင်း
  သင် Torii သို့ ပေးပို့ပါ။