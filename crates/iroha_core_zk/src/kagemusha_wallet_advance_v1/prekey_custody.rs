//! Provider-owned immutable pre-E1 records, under the same protected exclusive root.

use super::{
    KagemushaWalletFsV1, KagemushaWalletPlatformV1, KagemushaWalletProviderErrorV1,
    advance::KagemushaWalletAdvanceCapsuleV1,
    completion::KagemushaWalletCompletionFrameV1,
    layout::{
        KagemushaWalletCustodyDirV1, KagemushaWalletEntryNameV1,
        kagemusha_wallet_require_published_v1,
    },
    platform::{
        KagemushaWalletNotPublishedV1, KagemushaWalletPublishOutcomeV1, KagemushaWalletReadV1,
        KagemushaWalletUnavailableV1,
    },
    provider::KagemushaWalletProviderV1,
};

/// Fixed journal roles. Their bytes are recovery DATA, never reconstructed live authority.
#[derive(Clone, Copy)]
pub(crate) enum PreKeyRecordV1 {
    Client,
    Accepted,
    Slot,
}
impl PreKeyRecordV1 {
    fn name(
        self,
        id: &[u8; 32],
    ) -> Result<KagemushaWalletEntryNameV1, KagemushaWalletProviderErrorV1> {
        if *id == [0; 32] {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key request identity",
            });
        }
        let role = match self {
            Self::Client => "client",
            Self::Accepted => "accepted",
            Self::Slot => "slot",
        };
        let hex: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
        KagemushaWalletEntryNameV1::new(&format!("prekey-{hex}-{role}.norito")).ok_or(
            KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key record name",
            },
        )
    }
}
/// A create-new publication or adoption; only an actual Published result is fresh.
pub(crate) enum PreKeyPublicationV1 {
    Published,
    Existing(Vec<u8>),
}
const MAX: usize = 16_384;
impl<F, P, C, R> KagemushaWalletProviderV1<F, P, C, R>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    pub(crate) fn prekey_read(
        &self,
        id: &[u8; 32],
        role: PreKeyRecordV1,
    ) -> Result<Option<Vec<u8>>, KagemushaWalletProviderErrorV1> {
        self.require_storage()?;
        let result =
            match self
                .store
                .read(&KagemushaWalletCustodyDirV1::root(), &role.name(id)?, MAX)
            {
                KagemushaWalletReadV1::Present(bytes) => Ok(Some(bytes)),
                KagemushaWalletReadV1::Absent => Ok(None),
                KagemushaWalletReadV1::Oversized => {
                    Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                        object: "pre-key journal",
                    })
                }
                KagemushaWalletReadV1::Unavailable(reason) => {
                    Err(KagemushaWalletProviderErrorV1::Unavailable(reason))
                }
            };
        self.require_storage().and(result)
    }
    pub(crate) fn prekey_retain(
        &mut self,
        id: &[u8; 32],
        role: PreKeyRecordV1,
        bytes: &[u8],
    ) -> Result<PreKeyPublicationV1, KagemushaWalletProviderErrorV1> {
        self.require_storage()?;
        if bytes.is_empty() || bytes.len() > MAX {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key journal bound",
            });
        }
        let result = (|| {
            let dir = KagemushaWalletCustodyDirV1::root();
            let name = role.name(id)?;
            match self.store.write_new(&dir, &name, bytes) {
                KagemushaWalletPublishOutcomeV1::Published => Ok(PreKeyPublicationV1::Published),
                KagemushaWalletPublishOutcomeV1::NotPublished(
                    KagemushaWalletNotPublishedV1::DestinationExists,
                ) => {
                    let original = self.prekey_read(id, role)?.ok_or(
                        KagemushaWalletProviderErrorV1::Unavailable(
                            KagemushaWalletUnavailableV1::Busy,
                        ),
                    )?;
                    kagemusha_wallet_require_published_v1(
                        self.store.rewrite_same(&dir, &name, &original),
                    )?;
                    Ok(PreKeyPublicationV1::Existing(original))
                }
                outcome => {
                    kagemusha_wallet_require_published_v1(outcome)?;
                    unreachable!("Published handled")
                }
            }
        })();
        self.require_storage().and(result)
    }
}
