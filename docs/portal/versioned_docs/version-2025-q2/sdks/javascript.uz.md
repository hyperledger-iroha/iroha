---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK tezkor ishga tushirish

`@iroha2/torii-client` brauzer va Torii atrofida Node.js qulay oʻramini taqdim etadi.
Ushbu tezkor ishga tushirish SDK retseptlaridagi asosiy oqimlarni aks ettiradi, shuning uchun siz quyidagilarni olishingiz mumkin
mijoz bir necha daqiqada ishlaydi. To'liqroq misollar uchun qarang
Omborda `javascript/iroha_js/recipes/`.

## 1. O'rnatish

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Agar siz mahalliy tranzaktsiyalarni imzolashni rejalashtirmoqchi bo'lsangiz, kripto yordamchilarini ham o'rnating:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii mijozini yarating

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Konfiguratsiya retseptlarda ishlatiladigan konstruktorni aks ettiradi. Agar sizning tuguningiz
asosiy avtorizatsiyadan foydalanadi, `{username, password}` ni `basicAuth` opsiyasi orqali o'tkazing.

## 3. Tugun holatini olish

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Barcha o'qish operatsiyalari Norito tomonidan qo'llab-quvvatlanadigan JSON obyektlarini qaytaradi. Yaratilgan turlarga qarang
Maydon tafsilotlari uchun `index.d.ts`.

## 4. Tranzaksiyani yuboring

Imzolovchilar yordamchi API yordamida tranzaktsiyalar tuzishlari mumkin:

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

Yordamchi tranzaktsiyani kutilgan Norito konvertiga avtomatik tarzda o'rab oladi
Torii tomonidan. Boyroq misol (shu jumladan, yakuniy kutish) uchun qarang
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Yuqori darajadagi yordamchilardan foydalaning

SDK CLI-ni aks ettiruvchi maxsus oqimlarni to'playdi:

- **Boshqaruv yordamchilari** – `recipes/governance.mjs` sahnalashtirishni namoyish etadi
  `governance` ko'rsatma quruvchilar bilan takliflar va byulletenlar.
- **ISO ko'prigi** - `recipes/iso_bridge.mjs` `pacs.008` va qanday qilib topshirishni ko'rsatadi.
  `/v1/iso20022` so'nggi nuqtalari yordamida so'rov o'tkazish holati.
- **SoraFS va triggerlar** – `src/toriiClient.js` ostida sahifalash yordamchilari ochiladi
  shartnomalar, aktivlar, triggerlar va SoraFS provayderlari uchun yozilgan iteratorlar.

Ushbu oqimlarni qayta ishlatish uchun tegishli quruvchi funksiyalarini `@iroha2/torii-client` dan import qiling.

## 6. Xatolarni qayta ishlash

Barcha SDK qo'ng'iroqlari transport metama'lumotlari bilan boy `ToriiClientError` nusxalarini chiqaradi
va Norito xato yuki. Qo'ng'iroqlarni `try/catch` ga o'rang yoki `.catch()` dan foydalaning.
foydalanuvchilar uchun sirt konteksti:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Keyingi qadamlar

- `javascript/iroha_js/recipes/` dagi retseptlarni oxirigacha o'rganish uchun o'rganing.
- Batafsil ma'lumot uchun `javascript/iroha_js/index.d.ts` da yaratilgan turlarni o'qing
  usul imzolari.
- Foydali yuklarni tekshirish va disk raskadrovka qilish uchun ushbu SDKni Norito tezkor ishga tushirish bilan bog'lang
  Torii ga yuborasiz.