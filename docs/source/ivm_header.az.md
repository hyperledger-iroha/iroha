---
lang: az
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Baytkod Başlığı


Sehrli
- 4 bayt: ASCII `IVM\0` ofset 0.

Layout (cari)
- Ofsetlər və ölçülər (cəmi 17 bayt):
  - 0..4: sehrli `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (xüsusiyyət bitləri; aşağıya baxın)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (kiçik-endian)
  - 16: `abi_version: u8`

Rejim bitləri
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (ehtiyatda saxlanılır/xüsusiyyət qapalı).

Sahələr (mənası)
- `abi_version`: sistem çağırışı cədvəli və göstərici-ABI sxem versiyası.
- `mode`: ZK tracing/VECTOR/HTM üçün xüsusiyyət bitləri.
- `vector_length`: vektor əməliyyatları üçün məntiqi vektor uzunluğu (0 → qurulmamış).
- `max_cycles`: icra doldurulması ZK rejimində və qəbulda istifadə olunur.

Qeydlər
- Endianness və layout həyata keçirilməsi ilə müəyyən edilir və `version` bağlıdır. Yuxarıdakı naqil sxemi `crates/ivm_abi/src/metadata.rs`-də cari tətbiqi əks etdirir.
- Minimal oxucu cari artefaktlar üçün bu tərtibata etibar edə bilər və gələcək dəyişiklikləri `version` qapısı vasitəsilə idarə etməlidir.
- Avadanlıq sürətləndirilməsi (SIMD/Metal/CUDA) hər bir hosta qoşulur. İş vaxtı `AccelerationConfig` dəyərlərini `iroha_config`-dən oxuyur: `enable_simd` yalan olduqda skalyar geri dönmələrə məcbur edir, `enable_metal` və `enable_cuda` isə hətta müvafiq arxa uçlara tətbiq edildikdə belə. VM yaradılmasından əvvəl `ivm::set_acceleration_config`.
- Mobil SDK-lar (Android/Swift) eyni düymələri yerləşdirir; `IrohaSwift.AccelerationSettings`
  `connect_norito_set_acceleration_config`-ə zəng edir ki, macOS/iOS qurğuları Metal /
  NEON deterministik geri dönüşləri qoruyarkən.
- Operatorlar həmçinin `IVM_DISABLE_METAL=1` və ya `IVM_DISABLE_CUDA=1` ixrac etməklə diaqnostika üçün xüsusi arxa ucları məcburi söndürə bilərlər. Bu mühitin ləğvi konfiqurasiyadan üstündür və VM-ni deterministik CPU yolunda saxlayır.

Davamlı vəziyyət köməkçiləri və ABI səthi
- Davamlı vəziyyət köməkçi sistem zəngləri (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* və JSON/SCHEMA kodlaşdırma/deşifrə) V1 ABI-nin bir hissəsidir və `abi_hash` hesablamaya daxildir.
- CoreHost telləri STATE_{GET,SET,DEL} ilə WSV tərəfindən dəstəklənən davamlı smart-kontrakt vəziyyətinə; dev/test hostları üst-üstə düşmələrdən və ya yerli davamlılıqdan istifadə edə bilər, lakin eyni müşahidə olunan davranışı qorumalıdır.

Doğrulama
- Node qəbulu yalnız `version_major = 1` və `version_minor = 0` başlıqlarını qəbul edir.
- `mode` yalnız məlum bitləri ehtiva etməlidir: `ZK`, `VECTOR`, `HTM` (naməlum bitlər rədd edilir).
- `vector_length` məsləhət xarakteri daşıyır və hətta `VECTOR` biti təyin edilməsə belə, sıfırdan fərqli ola bilər; qəbul yalnız yuxarı həddi tətbiq edir.
- Dəstəklənən `abi_version` dəyərləri: ilk buraxılış yalnız `1` (V1) qəbul edir; digər dəyərlər qəbul zamanı rədd edilir.

### Siyasət (yaradıldı)
Aşağıdakı siyasət xülasəsi icradan yaradılıb və əl ilə redaktə edilməməlidir.<!-- BEGIN GENERATED HEADER POLICY -->
| Sahə | Siyasət |
|---|---|
| versiya_major | 1 |
| versiya_kiçik | 0 |
| rejim (məlum bitlər) | 0x07 (ZK=0x01, VEKTOR=0x02, HTM=0x04) |
| abi_version | 1 |
| vektor_uzunluğu | 0 və ya 1..=64 (məsləhətçi; VEKTOR bitindən asılı olmayaraq) |
<!-- END GENERATED HEADER POLICY -->

### ABI Haşları (yaradılmış)
Aşağıdakı cədvəl icradan yaradılıb və dəstəklənən siyasətlər üçün kanonik `abi_hash` dəyərlərini sadalayır.

<!-- BEGIN GENERATED ABI HASHES -->
| Siyasət | abi_hash (hex) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Kiçik yeniləmələr `feature_bits` və ayrılmış əməliyyat kodu sahəsinin arxasına təlimatlar əlavə edə bilər; əsas yeniləmələr kodlaşdırmaları dəyişdirə və ya yalnız protokolun təkmilləşdirilməsi ilə birlikdə silə/təkrar təyin edə bilər.
- Syscall diapazonları sabitdir; aktiv `abi_version` üçün naməlum `E_SCALL_UNKNOWN` verir.
- Qaz cədvəlləri `version` ilə bağlıdır və dəyişiklik zamanı qızıl vektorlar tələb olunur.

Artefaktların yoxlanılması
- Başlıq sahələrinin sabit görünüşü üçün `ivm_tool inspect <file.to>` istifadə edin.
- İnkişaf üçün nümunələr/ tikilmiş artefaktları yoxlayan kiçik Makefile hədəfi `examples-inspect` daxildir.

Nümunə (Rust): minimal sehr + ölçü yoxlaması

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Qeyd: Sehrdən kənar dəqiq başlıq tərtibatı versiyaya uyğunlaşdırılıb və icra ilə müəyyən edilib; sabit sahə adları və dəyərlər üçün `ivm_tool inspect`-ə üstünlük verin.