#[test]
fn redemption_change_c_header_declares_secret_allocator_contract() {
    let header = include_str!("../include/connect_norito_bridge.h");
    for declaration in [
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4(",
        "connect_norito_kagemusha_secret_free_buffer(uint8_t* ptr)",
        "KagemushaRecursiveSpendRedemptionChangePrepareRequestV4",
        "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
    ] {
        assert!(
            header.contains(declaration),
            "missing C header contract: {declaration}"
        );
    }
    let source = bridge_source();
    let prepare = source
        .split_once(
            "pub unsafe extern \"C\" fn connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        )
        .expect("prepare export")
        .1
        .split_once("pub unsafe extern \"C\" fn connect_norito_kagemusha_receiver_key_reference_v2")
        .expect("end of prepare export")
        .0;
    assert!(prepare.contains("write_kagemusha_secret_archive!("));
    assert!(!prepare.contains("write_kagemusha_archive_bridge"));
    assert!(!prepare.contains("connect_norito_free"));
    let secret_writer = source
        .split_once("macro_rules! write_kagemusha_secret_archive")
        .expect("secret Kagemusha archive writer")
        .1
        .split_once("unsafe fn write_kagemusha_archive_bridge")
        .expect("end of secret Kagemusha archive writer")
        .0;
    assert!(secret_writer.contains("Zeroizing::new"));
    assert!(secret_writer.contains("write_kagemusha_secret_bytes"));
    assert!(!secret_writer.contains("write_kagemusha_archive_bridge"));
    assert!(!secret_writer.contains("connect_norito_free"));
}
