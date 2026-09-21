// Direct account permission grant/revoke visitors, included in default.
/// Permission-checked visitors for direct permission grants and revocations.
pub mod permission {
    use super::*;
    macro_rules! impl_execute {
        ($executor:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let account_id = $isi.destination().clone();
            let permission = $isi.object();
            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !$executor.context().curr_block.is_genesis() {
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        &$executor.context().authority,
                        $executor.context(),
                        $executor.host(),
                    ) {
                        deny!($executor, error);
                    }
                }
                let isi = &<$isi_type>::account_permission(any_permission, account_id);
                execute!($executor, isi);
            }
            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }
    /// Grants an account-level permission after validating the caller's authority.
    pub fn visit_grant_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_grant, Grant<Permission, Account>);
    }
    /// Revokes an account-level permission once the caller passes permission checks.
    pub fn visit_revoke_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_revoke, Revoke<Permission, Account>);
    }
}
