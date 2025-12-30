use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

fn run_prog(src: &str) {
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should run with CoreHost TLV validation");
}

#[test]
fn roundtrip_create_nft() {
    let src = r#"
        fn main() {
          create_nft(
            nft_id("rose:uuid:0123$wonderland"),
            account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland")
          );
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_set_nft_data() {
    let src = r#"
        fn main() {
          set_nft_data(nft_id("rose:uuid:ffff$wonderland"), json("{\"meta\":1}"));
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_transfer_asset() {
    let src = r#"
        fn main() {
          transfer_asset(
            account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"),
            account_id("ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"),
            asset_definition("rose#wonderland"),
            1
          );
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_burn_nft() {
    let src = r#"
        fn main() {
          burn_nft(nft_id("rose:uuid:bead$wonderland"));
        }
    "#;
    run_prog(src);
}
