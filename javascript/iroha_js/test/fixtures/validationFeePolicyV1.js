export const VALIDATION_FEE_POLICY_HASH_HEX =
  "c0ec917f0e21aec945c982077ff591b5239559694a0de9101052672f1f7de3cf";

export const VALIDATION_FEE_POLICY_CANONICAL_BYTES_HEX =
  "4e5254300000d69d739b639ab6866991ad67e4fc1a5a00180100000000000070950f54a4bae970020201000c0b626f692d746573746e657420070707070707070707070707070707070707070707070707070707070707070708010000000000000001001d1c375a6570734a544843564c4b737246464e5a475352475a67764268760102080a000000000000006160736f726175efbe9b314eefbe8cefbe806a61efbdbcefbe924148efbe8befbda6efbe8cefbdb2efbdb6efbe89317a50546377efbe9befbe94efbe98efbe885a35efbda64b68efbe88efbdb15067efbdb3efbda668efbdb6efbdb4384a4a5357340400000000080a000000000000000a010864000000000000001d1c76616c69646174696f6e2d6665652d676f7665726e616e63652d7631190100000000000000100f54524541535552595f5041594f5554";

export const VALIDATION_FEE_POLICY_SIGNATURE_PAYLOAD_HEX =
  "a56ecc1661764e30e6946938b4e51835e2a1f838a6bd4a7d09f177047e0b988f";

export function validationFeePolicyFixture(overrides = {}) {
  const policy = {
    schema_version: 1,
    network_id: "boi-testnet",
    genesis_hash: "07".repeat(32),
    policy_version: 1,
    previous_policy_hash: null,
    ds_asset_id: "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
    ds_scale: 2,
    fee_minor_units: 10,
    treasury_account_id:
      "sorauﾛ1NﾌﾀjaｼﾒAHﾋｦﾌｲｶﾉ1zPTcwﾛﾔﾘﾈZ5ｦKhﾈｱPgｳｦhｶｴ8JJSW4",
    charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
    effective_from_height: 10,
    expires_after_height: 100,
    governance_keyset_id: "validation-fee-governance-v1",
    exemption_classes: ["TREASURY_PAYOUT"],
    ...(overrides.policy ?? {}),
  };
  const governanceKeyset = {
    keyset_id: "validation-fee-governance-v1",
    threshold: 2,
    public_keys_hex: [
      "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
      "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
    ],
    ...(overrides.governanceKeyset ?? {}),
  };
  const signatures = [
    {
      signer_public_key:
        "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
      signature:
        "cf12d296afe9f3adec7b83d29b224542af12e8af9fc67aaf03a3940c3522b354cea5d867ec3d31083c66a6b85dd04f09efd30a7fb6405dc8af6cf54c1ea8fe0e",
    },
    {
      signer_public_key:
        "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
      signature:
        "5824aa29fa708407018e0d438bcea9b94af56820326847101d3a7325bcd8689071de5b32153c6469a198587e7d8220b903f950fd460045f11d53c21022887c05",
    },
  ];
  const policyRegistry = {
    active_policy_hash: VALIDATION_FEE_POLICY_HASH_HEX,
    active_policy_version: 1,
    registered_policies: [
      {
        policy_version: 1,
        policy_hash: VALIDATION_FEE_POLICY_HASH_HEX,
        previous_policy_hash: null,
      },
    ],
    ...(overrides.policyRegistry ?? {}),
  };
  return {
    policy,
    signedPolicy: {
      policy,
      signatures: overrides.signatures ?? signatures,
    },
    governanceKeyset,
    policyRegistry,
    verificationContext: {
      networkId: "boi-testnet",
      genesisHash: "07".repeat(32),
      currentHeight: 10,
      governanceKeyset,
      policyRegistry,
      ...(overrides.verificationContext ?? {}),
    },
  };
}
