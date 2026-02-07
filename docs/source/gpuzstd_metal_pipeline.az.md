---
lang: az
direction: ltr
source: docs/source/gpuzstd_metal_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019b3aa25ae224c1595467ac809f2c53290813e91a78b78b94ca71c3dd950264
source_last_modified: "2026-01-31T19:25:45.072449+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GPU Zstd (Metal) Boru Kəməri

Bu sənəd Metal köməkçisi tərəfindən istifadə edilən deterministik GPU boru xəttini təsvir edir
zstd sıxılma üçün. üçün dizayn və icra təlimatıdır
Standart zstd çərçivələri və deterministik baytları yayan `gpuzstd_metal` köməkçisi
verilmiş ardıcıllıq axını üçün. Çıxışlar CPU dekoderləri ilə gediş-gəliş etməlidir; bayt üçün-
CPU kompressoru ilə bayt pariteti tələb olunmur, çünki ardıcıllığın yaradılması
fərqlənir.

## Məqsədlər

- CPU zstd ilə eyni şəkildə deşifrə olunan standart zstd çərçivələri buraxın; bayt pariteti
  CPU kompressoru ilə tələb olunmur.
- Aparat, drayverlər və iş qrafiki üzrə deterministik nəticələr.
- Açıq sərhəd yoxlamaları və proqnozlaşdırıla bilən bufer ömürləri.

## Cari icra qeydi

- GPU-da uyğunluğun tapılması və ardıcıllığın yaradılması.
- Çərçivə montajı və entropiya kodlaşdırması (Huffman/FSE) hazırda hostda işləyir
  qutuda olan kodlayıcıdan istifadə etməklə; GPU Huffman/FSE ləpələri paritet testindən keçir, lakin yox
  hələ tam çərçivə yoluna simli.
- Decode dəstəklənməyən çərçivələr üçün CPU zstd ehtiyatı ilə qutudaxili çərçivə dekoderindən istifadə edir;
  tam GPU blokunun deşifr edilməsi davam edir.

## Kodlaşdırma boru kəməri (yüksək səviyyə)

1. Daxiletmə quruluşu
   - Girişi cihazın buferinə kopyalayın.
   - Sabit ölçülü parçalara (ardıcıllıq yaratmaq üçün) və bloklara (
     zstd çərçivə montajı).
2. Uyğunluğun tapılması və ardıcıllıq emissiyası
   - GPU nüvələri hər bir parçanı skan edir və ardıcıllıqlar (hərfi uzunluq, uyğunluq
     uzunluq, ofset).
   - Ardıcıl sıralama sabit və deterministikdir.
3. Hərfi hazırlıq
   - Ardıcıllıqla istinad edilən hərfləri toplayın.
   - Hərfi histoqramlar yaradın və hərfi blok rejimini seçin (raw, RLE və ya
     Huffman) deterministik olaraq.
4. Huffman cədvəlləri (hərfi)
   - Histoqramdan kod uzunluqlarını yaradın.
   - CPU-ya uyğun gələn deterministik tie-break ilə kanonik cədvəllər yaradın
     zstd çıxışı.
5. FSE cədvəlləri (LL/ML/OF)
   - Tezlik saymalarını normallaşdırın.
   - FSE kodlaşdırma/kodlaşdırma cədvəllərini qəti şəkildə qurun.
6. Bitstream yazıçısı
   - Paket bitləri kiçik endian (LSB-ilk).
   - Bayt sərhədləri üzrə flush; yalnız sıfırları olan pad.
   - Dəyərləri elan edilmiş bit genişliklərinə maskalayın və tutumun yoxlanılmasını həyata keçirin.
7. Blok və çərçivənin yığılması
   - Blok başlıqlarını buraxın (növ, ölçü, sonuncu blok bayrağı).
   - Sıxılmış bloklara hərfləri və ardıcıllığı sıralayın.
   - Standart zstd çərçivə başlıqlarını və əlavə yoxlama məbləğlərini buraxın.

## Deşifrə boru kəməri (yüksək səviyyə)

1. Çərçivə təhlili
   - Sehrli baytları, pəncərə parametrlərini və çərçivə başlıq sahələrini təsdiqləyin.
2. Bit axını oxuyucusu
   - LSB-ilk bit ardıcıllıqlarını ciddi sərhəd yoxlamaları ilə oxuyun.
3. Hərfi deşifrə
   - Hərfi blokları (xam, RLE və ya Huffman) hərfi buferə deşifrə edin.
4. Ardıcıl deşifrə
   - FSE cədvəllərindən istifadə edərək LL/ML/OF dəyərlərini deşifrə edin.
   - Sürüşən pəncərədən istifadə edərək matçları yenidən qurun.
5. Çıxış və yoxlama məbləği
   - Yenidən qurulmuş baytları çıxış buferinə yazın.
   - Aktiv olduqda əlavə yoxlama məbləğlərini yoxlayın.

## Bufer ömrü və sahiblik- Giriş buferi: host -> cihaz, yalnız oxunur.
- Ardıcıllıq buferi: uyğunluq tapmaqla istehsal olunan və entropiya ilə istehlak edilən cihaz
  kodlaşdırma; çarpaz blokların təkrar istifadəsi yoxdur.
- Hərfi bufer: hər blok üçün istehsal olunan və blokdan sonra buraxılan cihaz
  emissiya.
- Çıxış buferi: cihaz, son kadr baytlarını host onları kopyalayana qədər saxlayır
  həyata.
- Scratch buferləri: nüvələr arasında təkrar istifadə olunur, lakin həmişə determinist şəkildə üzərinə yazılır.

## Kernel məsuliyyətləri

- Uyğunluq tapma ləpələri: uyğunluqları tapın və ardıcıllıqları buraxın (LL/ML/OF + hərfi).
- Huffman ləpələri qurur: kod uzunluqlarını və kanonik cədvəlləri əldə edin.
- FSE qurmaq ləpələri: LL/ML/OF cədvəlləri və dövlət maşınları qurun.
- Blok kodlayan ləpələr: bit axınına literalları və ardıcıllığı seriallaşdırın.
- Blok deşifrə ləpələri: bit axını təhlil edin və literalları/ardıcıllıqları yenidən qurun.

## Determinizm və paritet məhdudiyyətləri

- Kanonik cədvəl quruluşları CPU ilə eyni sıralama və bağlamadan istifadə etməlidir
  zstd.
- Hər hansı bir çıxış baytı üçün mövzu planlamasından asılı olan atom və ya azalma yoxdur.
- Bitstream qablaşdırma az-endian, LSB-birincidir; bayt hizalama yastıqları sıfırlarla.
- Bütün sərhəd yoxlamaları açıqdır; etibarsız girişlər deterministik şəkildə uğursuz olur.

## Doğrulama

- Bit axını yazıçısı/oxucusu üçün CPU qızıl vektorları.
- GPU və CPU çıxışlarını müqayisə edən korpus pariteti testləri.
- Düzgün olmayan çərçivələr və sərhəd şərtləri üçün qeyri-səlis əhatə.

## Müqayisə

`cargo test -p gpuzstd_metal gpu_vs_cpu_benchmark -- --ignored --nocapture`-i işə salın
CPU və GPU kodlaşdırma gecikməsini faydalı yük ölçüləri ilə müqayisə edin. Test hostlarda keçir
metal qabiliyyətli cihaz olmadan; aparat detalları ilə yanaşı çıxışı ələ keçirin
GPU yükləmə hədlərini tənzimləyərkən. Norito eyni kəsməni tətbiq edir
`gpu_zstd::encode_all`, beləliklə, birbaşa zəng edənlər evristik qapıya uyğun gəlir.