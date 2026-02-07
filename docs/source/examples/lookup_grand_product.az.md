---
lang: az
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Böyük Məhsul Nümunəsini axtarın

Bu nümunədə qeyd olunan FASTPQ icazə axtarış arqumentini genişləndirir
`fastpq_plan.md`.  Mərhələ 2 boru kəmərində prover seçicini qiymətləndirir
(`s_perm`) və şahid (`perm_hash`) aşağı dərəcəli genişləndirmə (LDE) sütunları
domen, işləyən böyük `Z_i` məhsulunu yeniləyir və nəhayət, bütün
Poseidon ilə ardıcıllıq.  Hashed akkumulyator transkriptə əlavə olunur
`fastpq:v1:lookup:product` domeni altında, son `Z_i` hələ də uyğun gəlir
qəbul edilmiş icazə cədvəli məhsulu `T`.

Aşağıdakı seçici dəyərləri olan kiçik bir partiyanı nəzərdən keçiririk:

| sıra | `s_perm` | `perm_hash` |
| --- | -------- | ------------------------------------------------------- |
| 0 | 1 | `0x019a...` (qrant rolu = auditor, icazə = transfer_aktivi) |
| 1 | 0 | `0xabcd...` (icazə dəyişikliyi yoxdur) |
| 2 | 1 | `0x42ff...` (rolu ləğv et = auditor, icazə = yandırma_aktivi) |

Qoy `gamma = 0xdead...` Fiat-Shamir axtarış problemi olsun.
transkript.  Prover `Z_0 = 1`-i işə salır və hər cərgəni qatlayır:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0`-in akkumulyatoru dəyişdirmədiyi sətirlər.  emal etdikdən sonra
iz, prover Poseidon transkript üçün `[Z_1, Z_2, ...]` ardıcıllığını heş edir.
hələ də cədvələ uyğun olaraq `Z_final = Z_3` (son işləyən məhsul) dərc edir
sərhəd vəziyyəti.

Cədvəl tərəfində, verilmiş icazə Merkle ağacı deterministi kodlayır
slot üçün aktiv icazələr dəsti.  Doğrulayıcı (və ya sübut edən
şahid nəsli) hesablayır

```
T = product over entries: (entry.hash + gamma)
```

Protokol `Z_final / T = 1` sərhəd məhdudiyyətini tətbiq edir.  Əgər iz
Cədvəldə olmayan icazəni təqdim etdi (və ya onu buraxdı
is), böyük məhsul nisbəti 1-dən ayrılır və yoxlayıcı rədd edir.  Çünki
Goldilocks sahəsinin daxilində hər iki tərəf `(value + gamma)` ilə çoxalır, nisbət
CPU/GPU arxa uçlarında sabit qalır.

Nümunəni armaturlar üçün Norito JSON kimi seriallaşdırmaq üçün dəstini qeyd edin
`perm_hash`, seçici və hər cərgədən sonra akkumulyator, məsələn:

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

Hexadecimal yer tutucular (`0x...`) beton qızıl kilidlərlə əvəz edilə bilər
avtomatlaşdırılmış testlər yaradan zaman sahə elementləri.  Əlavə olaraq 2-ci mərhələ armaturları
işləyən akkumulyatorun Poseidon hashını qeyd edin, lakin eyni JSON formasını saxlayın,
beləliklə, nümunə gələcək test vektorları üçün şablon kimi ikiqat ola bilər.