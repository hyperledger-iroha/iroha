---
lang: az
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3502fc6de75095282d44ce778b00d1b0d554773de1861d1b92f7dc573dfafa2
source_last_modified: "2025-12-29T18:16:35.969398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ISI Genişləndirilməsi Planı (v1)

Bu qeyd yeni Iroha Xüsusi Təlimatlar və çəkilişlər üçün prioritet qaydada imzalanır.
icradan əvvəl hər bir təlimat üçün müzakirə olunmayan invariantlar. Sifariş uyğun gəlir
təhlükəsizlik və əməliyyat riski birinci, UX ötürmə qabiliyyəti ikinci.

## Prioritet yığını

1. **RotateAccountSignatory** – Dağıdıcı miqrasiya olmadan açarların gigiyenik fırlanması üçün tələb olunur.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – Deterministik müqavilə təmin edin
   açarları öldürün və təhlükəyə məruz qalmış yerləşdirmələr üçün yaddaşın bərpası.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – Metadata paritetini konkret aktivə genişləndirin
   balanslaşdırır, belə ki, müşahidə edilə bilən alətlər holdinqləri etiketləyə bilər.
4. **BatchMintAsset** / **BatchTransferAsset** – Yük həcmini saxlamaq üçün deterministik fan-out köməkçiləri
   və VM geri qaytarılma təzyiqi idarə edilə bilər.

## Təlimat dəyişikliyi

### SetAssetKeyValue / RemoveAssetKeyValue
- `AssetMetadataKey` ad sahəsini (`state.rs`) təkrar istifadə edin ki, kanonik WSV açarları sabit qalsın.
- Hesab metadata köməkçiləri ilə eyni şəkildə JSON ölçüsü və sxem məhdudiyyətlərini tətbiq edin.
- Təsirə məruz qalan `AssetId` ilə `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` buraxın.
- Mövcud aktiv metadata redaktələri ilə eyni icazə nişanlarını tələb edin (tərif sahibi OR
  `CanModifyAssetMetadata` üslublu qrantlar).
- Aktiv qeydi yoxdursa dayandırın (dolaylı yaradılması yoxdur).

### Hesabı İmzalayanı Döndürün
- `AccountId`-də hesabın metadatasını qoruyarkən imzalayanın atom mübadiləsi və əlaqəli
  resurslar (aktivlər, tetikler, rollar, icazələr, gözlənilən hadisələr).
- Cari imza sahibinin zəng edənə (yaxud açıq-saçıq işarə ilə verilmiş səlahiyyətə) uyğun olduğunu yoxlayın.
- Yeni açıq açar artıq eyni domendə başqa hesabı dəstəkləyirsə, rədd edin.
- Hesab identifikatorunu daxil edən bütün kanonik açarları yeniləyin və əməliyyatdan əvvəl keşləri etibarsız edin.
- Audit yolları üçün köhnə/yeni açarlarla xüsusi `AccountEvent::SignatoryRotated` buraxın.
- Miqrasiya iskelesi: `AccountLabel` + `AccountRekeyRecord` (bax `account::rekey`) təqdim edin
  mövcud hesablar hash fasilələri olmadan yuvarlanan təkmilləşdirmə zamanı sabit etiketlərə uyğunlaşdırıla bilər.

### Müqavilə Nümunəsini Deaktiv edin
- Mənbə məlumatlarını davam etdirərkən `(namespace, contract_id)` bağlamasını silin və ya məzar daşı
  problemlərin aradan qaldırılması üçün (kim, nə vaxt, səbəb kodu).
- Aktivləşdirmə ilə eyni idarəetmə icazəsini tələb edin, icazə verilməməsi üçün siyasət qarmaqları
  yüksək təsdiq olmadan əsas sistem ad boşluqlarının deaktiv edilməsi.
- Hadisə qeydlərini deterministik saxlamaq üçün instansiya artıq qeyri-aktiv olduqda rədd edin.
- Aşağı axını müşahidə edənlərin istehlak edə biləcəyi `ContractInstanceEvent::Deactivated` buraxın.### SmartContractBytes Sil
- Yalnız heç bir manifest və ya aktiv nümunə olmadıqda `code_hash` tərəfindən saxlanılan bayt kodunun kəsilməsinə icazə verin
  artefakt istinad; əks halda təsviri xəta ilə uğursuz olur.
- İcazə qapısı güzgülərinin qeydiyyatı (`CanRegisterSmartContractCode`) və operator səviyyəsində
  qoruyucu (məsələn, `CanManageSmartContractStorage`).
- Təqdim olunan `code_hash`-in silinməmişdən əvvəl saxlanılan bədən həzminə uyğun olduğunu yoxlayın.
  köhnə tutacaqlar.
- Hash və zəng edən metadata ilə `ContractCodeEvent::Removed` emit.

### BatchMintAsset / BatchTransferAsset
- Hamısı və ya heç nə semantikası: ya hər bir dəst uğurlu olur, ya da göstəriş yan olmadan dayandırılır
  effektləri.
- Giriş vektorları deterministik qaydada sıralanmalı (dolaylı çeşidləmə yoxdur) və konfiqurasiya ilə məhdudlaşdırılmalıdır.
  (`max_batch_isi_items`).
- Aşağı axın mühasibat uçotunun ardıcıl qalması üçün hər bir maddə üzrə aktiv hadisələri buraxın; toplu kontekst əlavədir,
  əvəzedici deyil.
- İcazə yoxlamaları hər bir hədəf üçün mövcud tək elementli məntiqi təkrar istifadə edir (aktiv sahibi, tərif sahibi,
  və ya verilmiş qabiliyyət) dövlət mutasiyasından əvvəl.
- Məsləhətçi giriş dəstləri optimist paralelliyi düzgün saxlamaq üçün bütün oxu/yazma düymələrini birləşdirməlidir.

## İcra İskeleleri

- Data modeli indi balans metadatası üçün `SetAssetKeyValue` / `RemoveAssetKeyValue` skafoldlarını daşıyır
  redaktələr (`transparent.rs`).
- İcraçı ziyarətçilər məftil torpaqlarına ev sahibliyi etdikdən sonra icazələri bağlayacaq yer tutucuları ifşa edirlər
  (`default/mod.rs`).
- Rekey prototip növləri (`account::rekey`) yuvarlanan miqrasiya üçün eniş zonasını təmin edir.
- Dünya dövlətinə `AccountLabel` ilə işarələnmiş `account_rekey_records` daxildir ki, biz etiket hazırlaya bilək →
  tarixi `AccountId` kodlaşdırmasına toxunmadan imzalayan miqrasiya.

## IVM Syscall Layihəsi

- `DeactivateContractInstance` / `RemoveSmartContractBytes` üçün host şimləri kimi göndərilir
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) və
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), hər ikisini əks etdirən Norito TLV-ləri istehlak edir.
  kanonik ISI strukturları.
- `abi_syscall_list()`-i yalnız host işləyiciləri saxlamaq üçün `iroha_core` icra yollarını əks etdirdikdən sonra genişləndirin
  ABI hashləri inkişaf zamanı sabitdir.
- Sistem zəngi nömrələri sabitləşdikdən sonra aşağı salınan Kotodama-i yeniləyin; genişləndirilmiş üçün qızıl örtük əlavə edin
  eyni zamanda səth.

## Vəziyyət

Yuxarıdakı sifariş və invariantlar icraya hazırdır. Təqib filialları istinad etməlidir
icra yolları və sistemə məruz qalma zamanı bu sənəd.