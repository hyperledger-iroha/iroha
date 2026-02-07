---
lang: uz
direction: ltr
source: docs/source/gpuzstd_metal_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019b3aa25ae224c1595467ac809f2c53290813e91a78b78b94ca71c3dd950264
source_last_modified: "2026-01-31T19:25:45.072449+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GPU Zstd (metall) quvur liniyasi

Ushbu hujjat Metall yordamchi tomonidan ishlatiladigan deterministik GPU quvur liniyasini tavsiflaydi
zstd siqish uchun. Bu loyihalash va amalga oshirish uchun qo'llanma
Standart zstd ramkalar va deterministik baytlarni chiqaradigan `gpuzstd_metal` yordamchisi
berilgan ketma-ketlik oqimi uchun. Chiqishlar protsessor dekoderlari bilan aylana bo'lishi kerak; bayt uchun-
protsessor kompressoriga bayt pariteti kerak emas, chunki ketma-ketlikni yaratish
farq qiladi.

## Maqsadlar

- CPU zstd bilan bir xil tarzda dekodlanadigan standart zstd freymlarini chiqarish; bayt pariteti
  CPU kompressoriga ega bo'lish shart emas.
- Uskunalar, drayverlar va mavzularni rejalashtirish bo'yicha deterministik natijalar.
- Aniq chegaralarni tekshirish va prognoz qilinadigan bufer ishlash muddati.

## Joriy amalga oshirish eslatmasi

- GPUda moslikni topish va ketma-ketlikni yaratish.
- Xostda hozirda ramka yig'ish va entropiya kodlash (Huffman/FSE) ishlaydi
  kassa ichidagi enkoderdan foydalanish; GPU Huffman/FSE yadrolari paritet-sinovdan o'tgan, ammo yo'q
  hali to'liq ramka yo'liga simli.
- Decode qo'llab-quvvatlanmaydigan ramkalar uchun CPU zstd zaxirasi bilan kassa ichidagi ramka dekoderidan foydalanadi;
  GPU blokining toʻliq dekodlanishi davom etmoqda.

## Kodlash quvur liniyasi (yuqori daraja)

1. Kirish bosqichi
   - Kirishni qurilma buferiga nusxalash.
   - Ruxsat etilgan o'lchamdagi bo'laklarga (ketma-ketlikni yaratish uchun) va bloklarga (uchun
     zstd ramka yig'ilishi).
2. Moslikni topish va ketma-ketlik emissiyasi
   - GPU yadrolari har bir bo'lakni skanerlaydi va ketma-ketliklarni chiqaradi (so'zma-so'z uzunlik, moslik
     uzunlik, ofset).
   - Ketma-ket tartiblash barqaror va deterministikdir.
3. So‘zma-so‘z tayyorlanish
   - Ketma-ketliklar bo'yicha havola qilingan harflarni to'plang.
   - Harf gistogrammalarini yarating va literal blok rejimini tanlang (raw, RLE yoki
     Huffman) deterministik tarzda.
4. Huffman jadvallari (harf)
   - Gistogrammadan kod uzunligini yarating.
   - Protsessorga mos keladigan deterministik ti-break bilan kanonik jadvallarni yarating
     zstd chiqishi.
5. FSE jadvallari (LL/ML/OF)
   - Chastotalarni hisoblashni normallashtirish.
   - FSE dekodlash/kodlash jadvallarini deterministik tarzda yarating.
6. Bitstream yozuvchi
   - Paket bitlari little-endian (LSB-birinchi).
   - bayt chegaralarida flush; faqat nolga ega pad.
   - Qiymatlarni e'lon qilingan bit kengliklariga maskalash va sig'imni tekshirishni amalga oshirish.
7. Blok va ramkani yig'ish
   - Blok sarlavhalarini chiqarish (turi, o'lchami, oxirgi blok bayrog'i).
   - Literallar va ketma-ketliklarni siqilgan bloklarga seriyalash.
   - Standart zstd ramka sarlavhalari va ixtiyoriy nazorat summalarini chiqaring.

## Quvurni dekodlash (yuqori daraja)

1. Kadrlarni tahlil qilish
   - Sehrli baytlarni, oyna sozlamalarini va ramka sarlavhasi maydonlarini tasdiqlang.
2. Bitstream o'quvchi
   - Qattiq chegaralarni tekshirish bilan LSB-birinchi bit ketma-ketligini o'qing.
3. Literal dekodlash
   - Literal bloklarni (raw, RLE yoki Huffman) literal buferga dekodlash.
4. Ketma-ket dekodlash
   - FSE jadvallari yordamida LL/ML/OF qiymatlarini dekodlash.
   - Sürgülü oyna yordamida gugurtlarni qayta tuzing.
5. Chiqish va nazorat summasi
   - Rekonstruksiya qilingan baytlarni chiqish buferiga yozing.
   - Yoqilganda ixtiyoriy nazorat summalarini tekshiring.

## Buferning ishlash muddati va egalik qilish- Kirish buferi: xost -> qurilma, faqat o'qish uchun.
- Ketma-ketlik buferi: moslikni topish orqali ishlab chiqarilgan va entropiya tomonidan iste'mol qilinadigan qurilma
  kodlash; o'zaro faoliyat bloklarni qayta ishlatish yo'q.
- Literal bufer: qurilma, har bir blok uchun ishlab chiqariladi va blokdan keyin chiqariladi
  emissiya.
- Chiqish buferi: qurilma oxirgi kadr baytlarini xost ularni nusxalamaguncha ushlab turadi
  tashqariga.
- Scratch buferlari: yadrolar bo'ylab qayta ishlatiladi, lekin har doim deterministik tarzda yoziladi.

## Yadro mas'uliyati

- Moslikni topish yadrolari: mos keladiganlarni toping va ketma-ketlikni chiqaring (LL/ML/OF + literal).
- Huffman yadrolarini qurish: kod uzunligi va kanonik jadvallarni olish.
- FSE yadrolarini qurish: LL/ML/OF jadvallari va davlat mashinalarini yaratish.
- Blok kodlash yadrolari: bitstreamga literallar va ketma-ketliklarni ketma-ketlashtirish.
- Yadrolarni dekodlash bloki: bit oqimini tahlil qilish va literallarni/ketmalarni qayta qurish.

## Determinizm va paritet cheklovlari

- Kanonik jadval tuzilmalari protsessor bilan bir xil tartiblash va bog'lashdan foydalanishi kerak
  zstd.
- Har qanday chiqish bayti uchun mavzuni rejalashtirishga bog'liq bo'lgan atomlar yoki qisqartirishlar yo'q.
- Bitstream qadoqlash - little-endian, LSB-birinchi; nol bilan bayt hizalama yostiqchalari.
- barcha chegaralarni tekshirish aniq; noto'g'ri kiritilgan ma'lumotlar deterministik ravishda muvaffaqiyatsizlikka uchraydi.

## Tasdiqlash

- Bitstream yozuvchi/o'quvchi uchun CPU oltin vektorlari.
- GPU va protsessor chiqishini taqqoslaydigan korpus pariteti testlari.
- Noto'g'ri shakllangan ramkalar va chegara shartlari uchun noaniq qoplama.

## Benchmarking

`cargo test -p gpuzstd_metal gpu_vs_cpu_benchmark -- --ignored --nocapture` ni ishga tushiring
foydali yuk o'lchamlari bo'yicha CPU va GPU kodlash kechikishini solishtiring. Sinov hostlarda o'tkazib yuboriladi
metallga mos keladigan qurilmasiz; apparat tafsilotlari bilan birga chiqishni yozib oling
GPU tushirish chegaralarini sozlashda. Norito bir xil kesishni amalga oshiradi
`gpu_zstd::encode_all`, shuning uchun to'g'ridan-to'g'ri qo'ng'iroq qiluvchilar evristik eshikka mos keladi.