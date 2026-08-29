fn invalid_privacy_parameter(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn has_exact_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    required: &Permission,
) -> bool {
    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.contains(required))
    {
        return true;
    }
    state_transaction
        .world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                state_transaction.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(|permission| permission == required))
}
pub(super) fn ensure_privacy_governance(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let required: Permission = CanEnactGovernance.into();
    if !has_exact_permission(state_transaction, authority, &required) {
        return Err(Error::InvariantViolation(
            "not permitted: CanEnactGovernance".into(),
        ));
    }
    Ok(())
}
