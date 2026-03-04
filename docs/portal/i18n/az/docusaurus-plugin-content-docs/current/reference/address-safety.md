---
lang: az
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Address Safety & Accessibility
description: UX requirements for presenting and sharing Iroha addresses safely (ADDR-6c).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu səhifə çatdırıla bilən ADDR-6c sənədlərini əks etdirir. Bunları tətbiq edin
cüzdanlara, tədqiqatçılara, SDK alətlərinə və istənilən portal səthinə məhdudiyyətlər
insan üzlü ünvanları verir və ya qəbul edir. Kanonik məlumat modeli yaşayır
`docs/account_structure.md`; aşağıdakı yoxlama siyahısı bunları necə ifşa etməyi izah edir
təhlükəsizlik və ya əlçatanlığa xələl gətirmədən formatlar.

## Təhlükəsiz paylaşma axınları

- IH58 ünvanına hər surəti/paylaşım əməliyyatını defolt edin. Həll olunanı göstərin
  domeni dəstəkləyici kontekst kimi istifadə edin, beləliklə yoxlama cəmlənmiş sətir ön və mərkəzdə qalır.
- Tam düz mətn ünvanını və QR-ni özündə birləşdirən “Paylaş” imkanı təklif edin
  eyni faydalı yükdən əldə edilən kod. Təhlükə etməzdən əvvəl istifadəçilərə hər ikisini yoxlamağa icazə verin.
- Boşluq kəsilmə tələb etdikdə (kiçik kartlar, bildirişlər), aparıcını saxlayın
  insan tərəfindən oxuna bilən prefiks, ellips göstərin və son 4-6 simvolu belə saxlayın
  the checksum anchor survives. Tam surəti çıxarmaq üçün kran/klaviatura qısayolunu təmin edin
  string without truncation.
- Öncədən baxan təsdiq tostunu yaymaqla panoya sinxronizasiyanın qarşısını alın
  kopyalanan dəqiq IH58 sətri. Telemetriyanın mövcud olduğu yerlərdə nüsxəni sayın
  UX reqressiyalarının tez üzə çıxması üçün paylaşılan hərəkətlərə qarşı cəhdlər.

## IME & input safeguards

- Reject non-ASCII input in address fields. When IME composition artefacts (full
  genişlik, Kana, ton işarələri) görünür, bunun necə olduğunu izah edən sətirli xəbərdarlıq səthi
  yenidən cəhd etməzdən əvvəl klaviaturanı Latın daxilinə keçmək üçün.
- Birləşən işarələri ayıran və əvəz edən düz mətnli yapışdırma zonası təmin edin
  doğrulamadan əvvəl ASCII boşluqları olan boşluq. This keeps users from losing
  progress when they disable their IME mid-flow.
- Sıfır genişlikli birləşdiricilərə, variasiya seçicilərinə və digərlərinə qarşı təsdiqləməni sərtləşdirin
  stealth Unicode code points. Log the rejected code point category so fuzzing
  suites can import the telemetry.

## Assistive technology expectations

- Hər bir ünvan blokunu `aria-label` və ya `aria-describedby` ilə qeyd edin
  insan tərəfindən oxuna bilən prefiksi yazır və faydalı yükü 4-8 simvolda parçalara ayırır
  groups (“ih dash b three two …”). Bu, ekran oxuyucularının bir fayl yaratmasını dayandırır
  unintelligible stream of characters.
- Nəzakətli canlı bölgə yeniləməsi vasitəsilə uğurlu surətdə/paylaşım hadisələrini elan edin. Daxil et
  təyinat (mübadilə buferi, paylaşım vərəqi, QR) istifadəçinin hərəkəti bilməsi üçün
  completed without moving focus.
- QR önizləmələri üçün təsviri `alt` mətni təqdim edin (məsələn, “IH58 ünvanı üçün
  `<account>` on chain `0x1234`”). “Ünvanı mətn kimi kopyalayın” təmin edin
  zəif görmə qabiliyyəti olan istifadəçilər üçün QR kətanına bitişik geri dönüş.

## Sora-only compressed addresses

- Gating: `sora…` sıxılmış simli açıq təsdiqin arxasında gizlədin.
  Təsdiq formanın yalnız Sora Nexus zəncirlərində işlədiyini təkrar etməlidir.
- Etiketləmə: hər bir hadisəyə görünən “Yalnız Sora” nişanı və a
  digər şəbəkələrin niyə IH58 formasını tələb etdiyini izah edən alət ipucu.
- Qoruyucular: aktiv zəncir diskriminantı Nexus bölgüsü deyilsə,
  sıxılmış ünvanı tamamilə yaratmaqdan imtina edin və istifadəçini geri yönləndirin
  IH58.
- Telemetriya: sıxılmış formanın nə qədər tez-tez tələb olunduğunu və kopyalandığını qeyd edin
  insident playbook təsadüfi paylaşma sıçrayışlarını aşkar edə bilər.

## Keyfiyyətli qapılar

- Bu ünvanı təsdiqləmək üçün avtomatlaşdırılmış UI testlərini (və ya hekayə kitabı a11y suitelərini) genişləndirin
  komponentlər tələb olunan ARIA metadatasını və həmin IME rədd mesajlarını ifşa edir
  görünür.
- IME girişi (kana, pinyin), ekran oxuyucu keçidi üçün əl ilə QA ssenarilərini daxil edin
  (VoiceOver/NVDA) və buraxılmazdan əvvəl yüksək kontrastlı mövzularda QR surəti.
- IH58 paritet testləri ilə yanaşı buraxılış yoxlama siyahılarında bu yoxlamaları göstərin
  beləliklə, reqressiyalar düzələnə qədər bloklanır.