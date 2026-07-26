    fn build_key_description_der(
        challenge: &[u8],
        metadata: &AndroidFixtureMetadata,
    ) -> Vec<u8> {
        let unique = vec![0x44; 16];
        yasna::construct_der(|writer| {
            writer.write_sequence(|writer| {
                writer.next().write_u64(4);
                writer
                    .next()
                    .write_enum(i64::from(KM_SECURITY_LEVEL_STRONG_BOX));
                writer.next().write_u64(4);
                writer
                    .next()
                    .write_enum(i64::from(KM_SECURITY_LEVEL_STRONG_BOX));
                writer.next().write_bytes(challenge);
                writer.next().write_bytes(&unique);
                writer.next().write_sequence(|writer| {
                    writer.next().write_tagged(
                        Tag::context(u64::from(KM_TAG_ATTESTATION_APPLICATION_ID)),
                        |writer| {
                            writer.write_sequence(|writer| {
                                writer.next().write_set(|writer| {
                                    writer.next().write_sequence(|writer| {
                                        writer
                                            .next()
                                            .write_bytes(metadata.package_name.as_bytes());
                                        writer.next().write_u64(1);
                                    });
                                });
                                writer.next().write_set(|writer| {
                                    writer
                                        .next()
                                        .write_bytes(&metadata.signing_digest);
                                });
                            });
                        },
                    );
                });
                for _ in 0..2 {
                    writer.next().write_sequence(|writer| {
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_PURPOSE)),
                            |writer| {
                                writer.write_set(|writer| {
                                    writer.next().write_i64(KM_PURPOSE_SIGN as i64);
                                });
                            },
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_ALGORITHM)),
                            |writer| writer.write_i64(KM_ALGORITHM_EC as i64),
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_KEY_SIZE)),
                            |writer| writer.write_i64(256),
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_EC_CURVE)),
                            |writer| writer.write_i64(KM_EC_CURVE_P256 as i64),
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_ORIGIN)),
                            |writer| writer.write_i64(KM_ORIGIN_GENERATED as i64),
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_ROLLBACK_RESISTANCE)),
                            |writer| writer.write_bool(true),
                        );
                        writer.next().write_tagged(
                            Tag::context(u64::from(KM_TAG_ROOT_OF_TRUST)),
                            |writer| {
                                writer.write_sequence(|writer| {
                                    writer.next().write_bytes(&[0xAA; 32]);
                                    writer.next().write_bool(true);
                                    writer
                                        .next()
                                        .write_enum(
                                            i64::from(KM_VERIFIED_BOOT_STATE_VERIFIED),
                                        );
                                    writer.next().write_bytes(&[0xBB; 32]);
                                });
                            },
                        );
                    });
                }
            });
        })
    }