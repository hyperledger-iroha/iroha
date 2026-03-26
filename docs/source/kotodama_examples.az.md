---
lang: az
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Nümunələrə Baxış

Bu səhifə qısa Kotodama nümunələrini və onların IVM sistem çağırışlarına və göstərici-ABI arqumentlərinə necə uyğunlaşdığını göstərir. Həmçinin bax:
- İşlənə bilən mənbələr üçün `examples/`
- ABI kanonik sistem zəngi üçün `docs/source/ivm_syscalls.md`
- Tam dil spesifikasiyası üçün `kotodama_grammar.md`

## Salam + Hesab Təfərrüatları

Mənbə: `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

Xəritəçəkmə (göstərici-ABI):
- `authority()` → `SCALL 0xA4` (host `&AccountId`-i `r10`-ə yazır)
- `set_account_detail(a, k, v)` → hərəkət `r10=&AccountId`, `r11=&Name`, `r12=&Json`, sonra `SCALL 0x1A`

## Aktivin Transferi

Mənbə: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Xəritəçəkmə (göstərici-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, sonra `SCALL 0x24`

## NFT Yarat + Transfer

Mənbə: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let recipient = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Xəritəçəkmə (göstərici-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Göstərici Norito Köməkçiləri

Göstərici ilə qiymətləndirilən davamlı vəziyyət tipli TLV-lərin -ə və ya ondan çevrilməsini tələb edir
Hostlar olan `NoritoBytes` zərf qalır. Kotodama indi bu köməkçiləri bağlayır
inşaatçılar göstərici standartlarından və xəritədən istifadə edə bilməsi üçün birbaşa tərtibçi vasitəsilə
əl ilə FFI yapışqansız axtarışlar:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Aşağı salma:

- Göstərici defoltları yazılmış TLV-ni dərc etdikdən sonra `POINTER_TO_NORITO` yayır, beləliklə
  ev sahibi saxlama üçün kanonik `NoritoBytes` faydalı yük alır.
- Oxumalar `POINTER_FROM_NORITO` ilə əks əməliyyatı yerinə yetirir, təmin edir
  `r11`-də gözlənilən göstərici növü id.
- Hər iki yol avtomatik olaraq INPUT bölgəsinə hərfi TLV-ləri dərc etməyə imkan verir
  string literals və runtime göstəricilərini şəffaf şəkildə qarışdırmaq üçün müqavilələr bağlayır.

İş vaxtı reqressiyası üçün `crates/ivm/tests/kotodama_pointer_args.rs`-ə baxın
`MockWorldStateView`-ə qarşı gediş-gəliş həyata keçirir.

## Deterministik Xəritə İterasiyası (dizayn)

Hər biri üçün deterministik xəritə sərhəd tələb edir. Çox girişli iterasiya dövlət xəritəsi tələb edir; kompilyator `.take(n)` və ya elan edilmiş maksimum uzunluğu qəbul edir.

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

Semantika:
- İterasiya dəsti dövrə girişində anlıq görüntüdür; sıra açarın Norito baytına görə leksikoqrafikdir.
- `E_ITER_MUTATION` ilə döngə tələsində `M` üçün struktur mutasiyalar.
- Məhdudiyyət olmadan kompilyator `E_UNBOUNDED_ITERATION` yayır.

## Kompilyator/host daxili (Rust, Kotodama mənbəyi deyil)

Aşağıdakı fraqmentlər alətlər zəncirinin Rust tərəfində yaşayır. Onlar kompilyator köməkçilərini və VM endirmə mexanikasını təsvir edir və ** etibarlı deyil** Kotodama `.ko` mənbəyidir.

## Geniş Əməliyyat Kodu Parçalı Çərçivə Yeniləmələri

Kotodama-in geniş əməliyyat kodu köməkçiləri IVM tərəfindən istifadə edilən 8 bitlik operand düzümünü hədəfləyir.
geniş kodlaşdırma. 128-bit dəyərləri daşıyan yüklər və mağazalar üçüncü operanddan yenidən istifadə edir
yüksək registr üçün yuva, buna görə də əsas registr artıq finalı saxlamalıdır
ünvanı. Yükü / anbarı verməzdən əvvəl bazanı `ADDI` ilə tənzimləyin:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Parçalanmış çərçivə yeniləmələri reyestri təmin edərək bazanı 16 baytlıq addımlarla irəliləyir
`STORE128` tərəfindən törədilən cüt, tələb olunan uyğunlaşma sərhədinə düşür. Eyni
nümunə `LOAD128`-ə aiddir; əvvəl istədiyiniz addım ilə `ADDI` verilməsi
hər bir yük üçüncü operand yuvasına bağlı yüksək təyinat registrini saxlayır.
`VMError::MisalignedAccess` ilə yanlış hizalanmış ünvanlar VM ilə uyğunlaşır
davranış `crates/ivm/tests/wide_memory128.rs`-də həyata keçirilir.

Bu 128-bit köməkçiləri yayan proqramlar vektor qabiliyyətini reklam etməlidir.
Kotodama kompilyatoru istənilən vaxt avtomatik olaraq `VECTOR` rejim bitini işə salır.
`LOAD128`/`STORE128` görünür; ilə VM tələlər
Proqram onları icra etməyə cəhd edərsə, `VMError::VectorExtensionDisabled`
o bit dəsti olmadan.

## Geniş Şərti Şöbənin Azaldılması

Kotodama, `if`/`else` və ya üçlü filialı geniş bayt koduna endirdikdə, o,
sabit `BNE cond, zero, +2` ardıcıllığı və ardından bir cüt `JAL` təlimatı:

1. Qısa `BNE` şərti budaqı 8 bitlik dərhal zolaqda saxlayır
   `JAL`-dən tullanaraq.
2. Birinci `JAL` `else` blokunu hədəfləyir (şərt belə olduqda yerinə yetirilir.
   yalan).
3. İkinci `JAL` `then` blokuna tullanır (şərt uyğunlaşdıqda götürülür
   doğrudur).

Bu nümunə şərt yoxlamasının heç vaxt daha böyük ofsetləri kodlaşdırmamasına zəmanət verir
hələ də `then` üçün özbaşına böyük bədənləri dəstəkləyərkən ±127 sözdən çox
və geniş `JAL` köməkçisi vasitəsilə `else` blokları. Bax
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` üçün
ardıcıllıqla kilidləyən reqressiya testi.

### Aşağı salma nümunəsi

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Aşağıdakı geniş təlimat skeletinə (registr nömrələri və
mütləq ofsetlər əlavə funksiyadan asılıdır):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Sonrakı təlimatlar sabitləri maddiləşdirir və qaytarılan dəyəri yazır.
`BNE` ilk `JAL` üzərindən keçdiyi üçün şərti ofset həmişə
`+2` sözləri, blok gövdələri genişləndikdə belə filialı əhatə dairəsində saxlayır.