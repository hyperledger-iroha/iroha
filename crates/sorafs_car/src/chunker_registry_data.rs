use sorafs_chunker::ChunkProfile;

#[derive(Debug, Clone, Copy)]
pub struct RawChunkerDescriptor {
    pub id: u32,
    pub namespace: &'static str,
    pub name: &'static str,
    pub semver: &'static str,
    pub profile: ChunkProfile,
    pub multihash_code: u64,
    pub aliases: &'static [&'static str],
}

pub const RAW_REGISTRY: &[RawChunkerDescriptor] = &[
    RawChunkerDescriptor {
        id: 1,
        namespace: "sorafs",
        name: "sf1",
        semver: "1.0.0",
        profile: ChunkProfile::DEFAULT,
        multihash_code: 0x1f,
        aliases: &["sorafs.sf1@1.0.0", "sorafs-sf1"],
    },
    RawChunkerDescriptor {
        id: 2,
        namespace: "sorafs",
        name: "sf2",
        semver: "1.0.0",
        profile: ChunkProfile::SF2,
        multihash_code: 0x1f,
        aliases: &["sorafs.sf2@1.0.0", "sorafs-sf2"],
    },
];
