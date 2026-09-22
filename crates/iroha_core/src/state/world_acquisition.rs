//! Caller-owned World acquisition and retirement from the sole field inventory.
//!
//! All inert field slots exist before the first writer is acquired. A callee
//! that unwinds leaves its original partial state in its slot; aggregate Drop
//! releases every physical writer before any slot destroys values or notifies.

#[cfg(test)]
use super::WorldBlockFields;
use super::{
    WorldBlock,
    block_field::{BlockField, OriginalPublicationBlock},
};
use mv::{BlockAcquisition, BlockMode, BlockRetirement};

pub(super) trait IntoWorldField {
    type Field;
    fn into_world_field(self) -> Self::Field;
}
impl<B: OriginalPublicationBlock> IntoWorldField for B {
    type Field = BlockField<B>;
    fn into_world_field(self) -> Self::Field {
        BlockField::new(self)
    }
}
impl IntoWorldField for super::TriggerSetBlock<'_> {
    type Field = Self;
    fn into_world_field(self) -> Self {
        self
    }
}
pub(super) fn retain_executing_field<B: IntoWorldField>(block: B) -> B::Field {
    block.into_world_field()
}

macro_rules! declare_world_acquisition {
    (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
        // Generic parameter names deliberately follow the single field census.
        #[allow(non_camel_case_types)]
        pub(super) struct WorldAcquisition<
            $($prefix: BlockAcquisition,)*
            $($privacy: BlockAcquisition,)*
            $($suffix: BlockAcquisition,)*
        > {
            $(pub(super) $prefix: Option<$prefix>,)*
            $(pub(super) $privacy: Option<$privacy>,)*
            $(pub(super) $suffix: Option<$suffix>,)*
        }

        #[allow(non_camel_case_types)]
        impl<$($prefix: BlockAcquisition,)* $($privacy: BlockAcquisition,)* $($suffix: BlockAcquisition,)*>
            WorldAcquisition<$($prefix,)* $($privacy,)* $($suffix,)*>
        {
            pub(super) fn initialize(&mut self, mode: BlockMode) {
                $(self.$prefix.as_mut().expect("original field acquisition").initialize(mode);)*
                $(self.$privacy.as_mut().expect("original field acquisition").initialize(mode);)*
                $(self.$suffix.as_mut().expect("original field acquisition").initialize(mode);)*
            }
        }

        #[allow(non_camel_case_types)]
        impl<$($prefix: BlockAcquisition,)* $($privacy: BlockAcquisition,)* $($suffix: BlockAcquisition,)*>
            Drop for WorldAcquisition<$($prefix,)* $($privacy,)* $($suffix,)*>
        {
            fn drop(&mut self) {
                $(if let Some(field) = self.$prefix.as_mut() { field.release(); })*
                $(if let Some(field) = self.$privacy.as_mut() { field.release(); })*
                $(if let Some(field) = self.$suffix.as_mut() { field.release(); })*
                // Automatic field drop now reclaims payloads/charges and wakes
                // original waiters only after all acquired writers are free.
            }
        }

        impl WorldBlock<'_> {
            pub(super) fn prepare_publication(&mut self) {
                self.publication.begin_preparation();
                let fields = self.fields.as_mut().expect("original World block fields");
                $(fields.$prefix.prepare_publication();)*
                $(fields.$privacy.prepare_publication();)*
                $(fields.$suffix.prepare_publication();)*
                self.publication.finish_preparation();
            }
            pub(super) fn publish_prepared(&mut self) {
                self.publication.begin_publication();
                let fields = self.fields.as_mut().expect("original World block fields");
                $(fields.$prefix.publish_prepared();)*
                $(fields.$privacy.publish_prepared();)*
                $(fields.$suffix.publish_prepared();)*
                self.publication.finish_publication();
            }
        }

        impl BlockRetirement for WorldBlock<'_> {
            fn release_writers(&mut self) {
                self.publication.release();
                if let Some(fields) = self.fields.as_mut() {
                    $(fields.$prefix.release_writers();)*
                    $(fields.$privacy.release_writers();)*
                    $(fields.$suffix.release_writers();)*
                }
            }
        }
    };
}

with_world_overlay_fields!(declare_world_acquisition);

// Populate inert slots through a borrowed closure before native initialization.
// Per-field constructor temporaries must leave the stack before payload work;
// the enclosing acquisition owner itself stays in its caller throughout.
#[inline(never)]
pub(super) fn fill_world_acquisition(fill: impl FnOnce()) {
    fill();
}

// Keep final field-move temporaries out of the acquisition frame while native
// constructors and payload operations execute. The closure only borrows the
// original slots; it adds no allocation or alternate cleanup owner.
#[inline(never)]
pub(super) fn finish_world_acquisition<'world>(
    finish: impl FnOnce() -> WorldBlock<'world>,
) -> WorldBlock<'world> {
    finish()
}

#[cfg(test)]
impl<'world> WorldBlock<'world> {
    pub(super) fn into_fields(mut self) -> WorldBlockFields<'world> {
        self.publication.assert_executing();
        *self.fields.take().expect("original World block fields")
    }
}

impl Drop for WorldBlock<'_> {
    fn drop(&mut self) {
        self.release_writers();
    }
}

macro_rules! build_world_block_from_fields {
    ($state:expr, $mode:expr;
        [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{
        use mv::BlockAcquisition as _;
        let mut pending = world_acquisition::WorldAcquisition {
            $($prefix: None,)*
            $($privacy: None,)*
            $($suffix: None,)*
        };
        // Fill the same caller-owned slots one at a time before acquiring any
        // writer, without retaining a full composite initializer temporary.
        world_acquisition::fill_world_acquisition(|| {
            $(pending.$prefix = Some($state.$prefix.block_acquisition());)*
            $(pending.$privacy = Some($state.$privacy.block_acquisition());)*
            $(pending.$suffix = Some($state.$suffix.block_acquisition());)*
        });
        pending.initialize($mode);
        world_acquisition::finish_world_acquisition(|| WorldBlock {
            publication: block_field::AggregatePublication::Executing,
            // TODO: include this one metadata allocation in complete retained
            // pre-execution admission before enabling that production path.
            fields: Some(Box::new(WorldBlockFields {
                dataspace_catalog: iroha_data_model::nexus::DataSpaceCatalog::default(),
                $($prefix: world_acquisition::retain_executing_field(pending.$prefix.take().expect("original field acquisition").into_block()),)*
                $($privacy: world_acquisition::retain_executing_field(pending.$privacy.take().expect("original field acquisition").into_block()),)*
                $($suffix: world_acquisition::retain_executing_field(pending.$suffix.take().expect("original field acquisition").into_block()),)*
                external_event_buf: Vec::new(),
            })),
        })
    }};
}

macro_rules! build_world_block {
    ($state:expr, $mode:expr) => {
        with_world_overlay_fields!(build_world_block_from_fields, $state, ($mode))
    };
}
