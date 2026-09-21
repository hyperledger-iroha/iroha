// Canonical native/user state namespace predicates, included in the host module.
impl<QS: Default + QueryStateAccess> CoreHostImpl<QS> {
    fn contract_state_key_matches_namespace(key: &str, prefix: &str) -> bool {
        if let Some(root) = prefix
            .strip_suffix('/')
            .or_else(|| prefix.strip_suffix('_'))
        {
            return key == root || key.starts_with(prefix);
        }
        key == prefix
            || key
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('_') || suffix.starts_with('/'))
    }
    fn contract_state_namespace_access(key: &str) -> ContractStateNamespaceAccess {
        if OPAQUE_SYSTEM_CONTRACT_STATE_PREFIXES
            .iter()
            .any(|prefix| Self::contract_state_key_matches_namespace(key, prefix))
        {
            ContractStateNamespaceAccess::OpaqueSystem
        } else if READ_ONLY_SYSTEM_CONTRACT_STATE_PREFIXES
            .iter()
            .any(|prefix| Self::contract_state_key_matches_namespace(key, prefix))
        {
            ContractStateNamespaceAccess::ReadOnlySystem
        } else {
            ContractStateNamespaceAccess::User
        }
    }
    fn ensure_contract_state_read_allowed(path: &StatePath) -> Result<(), ivm::VMError> {
        if Self::contract_state_namespace_access(path.as_ref())
            == ContractStateNamespaceAccess::OpaqueSystem
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        Ok(())
    }
    fn ensure_contract_state_write_allowed(path: &StatePath) -> Result<(), ivm::VMError> {
        if Self::contract_state_namespace_access(path.as_ref())
            != ContractStateNamespaceAccess::User
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        Ok(())
    }
}
