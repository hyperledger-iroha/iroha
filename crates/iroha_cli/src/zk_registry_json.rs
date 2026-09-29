//! Public registry submission fields shared by the CLI and canonical key generator.

#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct VkSubmissionJson {
    pub(crate) backend: String,
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) circuit_id: String,
    pub(crate) public_inputs_schema_hash_hex: String,
    #[norito(default)]
    pub(crate) curve: Option<String>,
    #[norito(default)]
    pub(crate) gas_schedule_id: Option<String>,
    #[norito(default)]
    pub(crate) vk_len: Option<u32>,
    #[norito(default)]
    pub(crate) max_proof_bytes: Option<u32>,
    #[norito(default)]
    pub(crate) metadata_uri_cid: Option<String>,
    #[norito(default)]
    pub(crate) vk_bytes_cid: Option<String>,
    #[norito(default)]
    pub(crate) activation_height: Option<u64>,
    #[norito(default)]
    pub(crate) withdraw_height: Option<u64>,
    #[norito(default)]
    pub(crate) commitment_hex: Option<String>,
    #[norito(default)]
    pub(crate) vk_bytes: Option<String>,
    #[norito(default)]
    pub(crate) status: Option<iroha_data_model::confidential::ConfidentialStatus>,
    #[norito(default)]
    pub(crate) namespace: Option<String>,
}
