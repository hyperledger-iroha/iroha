//! Process-funded no-copy Metal backing and exact command-lifetime custody.

use iroha_accel::{NativeCommandPermit, ProcessResources, UnifiedBuffer};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue, MTLComputeCommandEncoder,
    MTLDevice, MTLResourceOptions,
};
use std::{
    cell::Cell,
    mem::ManuallyDrop,
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
};

use objc2_metal::{MTLCommandEncoder, MTLComputePipelineState, MTLSize};

static PHYSICAL_UNCERTAIN: AtomicBool = AtomicBool::new(false);

pub(super) fn physical_usable() -> bool {
    !PHYSICAL_UNCERTAIN.load(Ordering::Acquire)
}

fn owner() -> &'static ProcessResources {
    ProcessResources::get_or_initialize(crate::acceleration_config().resource_limits)
}

pub(super) struct MetalBuffer {
    native: ManuallyDrop<Retained<ProtocolObject<dyn MTLBuffer>>>,
    backing: ManuallyDrop<UnifiedBuffer>,
    pending: Cell<bool>,
    claimed: Cell<bool>,
}

impl MetalBuffer {
    pub(super) fn allocate(device: &ProtocolObject<dyn MTLDevice>, len: usize) -> Option<Self> {
        if len == 0 || !physical_usable() {
            return None;
        }
        let alignment = objc2_foundation::NSPageSize();
        if UnifiedBuffer::required_capacity(len, alignment).ok()? > device.maxBufferLength() {
            return None;
        }
        let backing = owner().try_unified_buffer(len, alignment).ok()?;
        // SAFETY: the stable backing is page aligned and its complete extent is
        // a page multiple. It remains owned through native wrapper destruction,
        // or is retained permanently if command completion becomes uncertain.
        // No deallocator block is supplied: the funded backing owner frees it.
        let native = unsafe {
            device.newBufferWithBytesNoCopy_length_options_deallocator(
                backing.as_ptr().cast(),
                backing.capacity(),
                MTLResourceOptions::StorageModeShared,
                None,
            )
        }?;
        Some(Self {
            native: ManuallyDrop::new(native),
            backing: ManuallyDrop::new(backing),
            pending: Cell::new(false),
            claimed: Cell::new(false),
        })
    }

    pub(super) fn copy_input<T: super::MetalBufferElement>(
        &mut self,
        values: &[T],
        len: usize,
    ) -> Option<()> {
        if len > std::mem::size_of_val(values)
            || len > self.backing.len()
            || self.claimed.get()
            || self.pending.get()
        {
            return None;
        }
        // SAFETY: only initialized padding-free primitives implement the private
        // marker; the checked source/destination ranges cannot overlap.
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr().cast::<u8>(),
                self.backing.as_ptr().as_ptr(),
                len,
            );
        }
        Some(())
    }
}
impl Deref for MetalBuffer {
    type Target = ProtocolObject<dyn MTLBuffer>;
    fn deref(&self) -> &Self::Target {
        &self.native
    }
}
impl Drop for MetalBuffer {
    fn drop(&mut self) {
        if self.pending.get() {
            return;
        }
        // SAFETY: prepared or exactly completed buffers are no longer accessed
        // by a command. Release the no-copy wrapper before freeing its backing.
        unsafe {
            ManuallyDrop::drop(&mut self.native);
            ManuallyDrop::drop(&mut self.backing);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandPhase {
    Prepared,
    Pending,
    Complete,
    Uncertain,
}

// Claims are fixed cells on the original buffers; no map or list allocation is
// needed. These buffers are !Sync, so checking and claiming all slots cannot race.
struct PreparedClaims<'a, 'b> {
    buffers: &'a [&'b MetalBuffer],
    active: bool,
}
impl<'a, 'b> PreparedClaims<'a, 'b> {
    fn acquire(buffers: &'a [&'b MetalBuffer]) -> Option<Self> {
        if buffers
            .iter()
            .any(|buffer| buffer.claimed.get() || buffer.pending.get())
        {
            return None;
        }
        for buffer in buffers {
            buffer.claimed.set(true);
        }
        Some(Self {
            buffers,
            active: true,
        })
    }
}
impl Drop for PreparedClaims<'_, '_> {
    fn drop(&mut self) {
        if self.active {
            for buffer in self.buffers {
                buffer.claimed.set(false);
            }
        }
    }
}

/// Fixed command-owner state; no allocation is needed to retain uncertain work.
pub(super) struct Command<'a, 'b> {
    native: ManuallyDrop<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    _permit: ManuallyDrop<NativeCommandPermit>,
    buffers: &'a [&'b MetalBuffer],
    phase: CommandPhase,
    encoder_issued: bool,
    encoding_succeeded: bool,
}
impl<'a, 'b> Command<'a, 'b> {
    pub(super) fn prepare(
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        buffers: &'a [&'b MetalBuffer],
    ) -> Option<Self> {
        if !physical_usable() {
            return None;
        }
        let mut claims = PreparedClaims::acquire(buffers)?;
        let permit = owner().try_native_command()?;
        let native = queue.commandBuffer()?;
        claims.active = false;
        Some(Self {
            native: ManuallyDrop::new(native),
            _permit: ManuallyDrop::new(permit),
            buffers,
            phase: CommandPhase::Prepared,
            encoder_issued: false,
            encoding_succeeded: false,
        })
    }
    pub(super) fn encode(
        &mut self,
        pipeline: &ProtocolObject<dyn MTLComputePipelineState>,
        grid_width: usize,
        threadgroup_width: usize,
    ) -> bool {
        if self.phase != CommandPhase::Prepared || self.encoder_issued {
            return false;
        }
        self.encoder_issued = true;
        if grid_width == 0
            || threadgroup_width == 0
            || threadgroup_width > pipeline.maxTotalThreadsPerThreadgroup()
        {
            return false;
        }
        let Some(encoder) = self.native.computeCommandEncoder() else {
            return false;
        };
        encoder.setComputePipelineState(pipeline);
        for (index, buffer) in self.buffers.iter().copied().enumerate() {
            // SAFETY: this command exclusively claims every complete no-copy
            // backing before binding. Each owner stays live through completion.
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(buffer), 0, index);
            }
        }
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width: grid_width,
                height: 1,
                depth: 1,
            },
            MTLSize {
                width: threadgroup_width,
                height: 1,
                depth: 1,
            },
        );
        encoder.endEncoding();
        drop(encoder);
        self.encoding_succeeded = true;
        true
    }
    pub(super) fn commit(&mut self) -> bool {
        if self.phase != CommandPhase::Prepared
            || !physical_usable()
            || (self.encoder_issued && !self.encoding_succeeded)
        {
            return false;
        }
        // Mark pending before the native call, including a panic after enqueue.
        self.phase = CommandPhase::Pending;
        for buffer in self.buffers {
            buffer.pending.set(true);
        }
        self.native.commit();
        true
    }
    /// Return terminal success/failure only after this owner reads actual status.
    /// Prepared, nonterminal and irreversibly uncertain work cannot release claims.
    pub(super) fn observe_completion(&mut self) -> Option<bool> {
        if self.phase != CommandPhase::Pending {
            return None;
        }
        let success = match self.native.status() {
            MTLCommandBufferStatus::Completed => true,
            MTLCommandBufferStatus::Error => false,
            _ => return None,
        };
        self.phase = CommandPhase::Complete;
        for buffer in self.buffers {
            buffer.pending.set(false);
            buffer.claimed.set(false);
        }
        Some(success)
    }
    pub(super) fn mark_uncertain(&mut self) {
        if self.phase == CommandPhase::Pending {
            self.phase = CommandPhase::Uncertain;
            PHYSICAL_UNCERTAIN.store(true, Ordering::Release);
            super::METAL_DISABLED.store(true, Ordering::SeqCst);
        }
    }
}
impl Drop for Command<'_, '_> {
    fn drop(&mut self) {
        self.mark_uncertain();
        if self.phase == CommandPhase::Uncertain {
            // Native command, original count permit, and every pending buffer's
            // no-copy wrapper/backing/ceilings remain owned without allocation.
            // A late command completion never refunds uncertain custody.
            return;
        }
        if self.phase == CommandPhase::Prepared {
            for buffer in self.buffers {
                buffer.claimed.set(false);
            }
        }
        // Complete claims were already released by this owner's observed status;
        // another command may now own those cells. Never clear them a second time.
        // SAFETY: no enqueue occurred, or a terminal driver status established
        // completion. Reclaim native command before releasing its original permit.
        unsafe {
            ManuallyDrop::drop(&mut self.native);
            ManuallyDrop::drop(&mut self._permit);
        }
    }
}

#[cfg(all(test, feature = "metal-hardware-tests"))]
mod tests {
    use super::*;
    use objc2_metal::MTLCreateSystemDefaultDevice;

    fn isolated(name: &str, control: impl FnOnce()) {
        const CHILD: &str = "IVM_METAL_CUSTODY_CONTROL";
        if std::env::var(CHILD).as_deref() == Ok(name) {
            control();
            return;
        }
        let path = module_path!()
            .split_once("::")
            .expect("test module has crate prefix")
            .1;
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", &format!("{path}::{name}"), "--nocapture"])
            .env(CHILD, name)
            .output()
            .expect("isolated Metal custody control");
        assert!(
            output.status.success(),
            "isolated Metal control failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn required_metal_buffer_lifetime_and_observed_completion() {
        isolated(
            "required_metal_buffer_lifetime_and_observed_completion",
            || {
                let device = MTLCreateSystemDefaultDevice()
                    .expect("Metal buffer qualification requires a physical device");
                let pool = owner();
                let before = pool.usage();
                let mut buffer = MetalBuffer::allocate(&device, 33).expect("funded no-copy buffer");
                let bytes =
                    UnifiedBuffer::required_capacity(33, objc2_foundation::NSPageSize()).unwrap();
                assert_eq!(
                    pool.usage().unified_bytes[0],
                    before.unified_bytes[0] + bytes
                );
                assert_eq!(pool.usage().host_bytes[0], before.host_bytes[0] + bytes);
                assert_eq!(pool.usage().device_bytes[0], before.device_bytes[0] + bytes);
                assert_eq!(
                    buffer.contents().as_ptr().cast::<u8>(),
                    buffer.backing.as_ptr().as_ptr()
                );
                assert_eq!(
                    buffer.contents().as_ptr() as usize % objc2_foundation::NSPageSize(),
                    0
                );
                assert!(buffer.copy_input(&[1u8; 34], 34).is_none());
                buffer.copy_input(&[73u8; 33], 33).unwrap();
                // SAFETY: no command has begun; the uniquely owned backing is initialized.
                assert_eq!(
                    unsafe {
                        std::slice::from_raw_parts(buffer.contents().as_ptr().cast::<u8>(), 33)
                    },
                    [73; 33]
                );
                let queue = device.newCommandQueue().expect("Metal command queue");
                let buffers = [&buffer];
                {
                    let command =
                        Command::prepare(&queue, &buffers).expect("funded exclusive command");
                    assert_eq!(pool.usage().in_flight[0], before.in_flight[0] + 1);
                    drop(command);
                    assert_eq!(pool.usage().in_flight[0], before.in_flight[0]);
                    assert_eq!(pool.usage().streams[0], before.streams[0]);
                }
                {
                    let mut command =
                        Command::prepare(&queue, &buffers).expect("funded exclusive command");
                    assert!(command.commit());
                    assert!(buffer.pending.get());
                    assert!(matches!(
                        super::super::finalize_command_buffer(
                            &mut command,
                            "required Metal custody control"
                        ),
                        super::super::MetalCommandOutcome::Complete
                    ));
                    assert_eq!(
                        command.observe_completion(),
                        None,
                        "completion cannot release twice"
                    );
                    assert!(!buffer.pending.get());
                }
                assert_eq!(pool.usage().in_flight[0], before.in_flight[0]);
                drop(buffer);
                assert_eq!(pool.usage().unified_bytes[0], before.unified_bytes[0]);
                assert_eq!(pool.usage().host_bytes[0], before.host_bytes[0]);
                assert_eq!(pool.usage().device_bytes[0], before.device_bytes[0]);
            },
        );
    }

    #[test]
    fn required_metal_uncertain_drop_retains_backing_and_original_permits() {
        isolated(
            "required_metal_uncertain_drop_retains_backing_and_original_permits",
            || {
                // Run this control in its own process: intentional uncertainty is sticky
                // and its reservations must remain charged until that process exits.
                let device = MTLCreateSystemDefaultDevice()
                    .expect("Metal uncertainty qualification requires a physical device");
                let pool = owner();
                let before = pool.usage();
                let buffer = MetalBuffer::allocate(&device, 17).expect("funded no-copy buffer");
                let bytes = buffer.backing.capacity();
                let queue = device.newCommandQueue().expect("Metal command queue");
                let buffers = [&buffer];
                let mut command =
                    Command::prepare(&queue, &buffers).expect("funded exclusive command");
                assert!(command.commit());
                // Even if this empty command has already completed physically, the owner
                // has not observed terminal status. Drop cannot invent that evidence.
                drop(command);
                drop(buffer);
                assert!(!physical_usable());
                assert!(Command::prepare(&queue, &[]).is_none());
                assert_eq!(
                    pool.usage().unified_bytes[0],
                    before.unified_bytes[0] + bytes
                );
                assert_eq!(pool.usage().host_bytes[0], before.host_bytes[0] + bytes);
                assert_eq!(pool.usage().device_bytes[0], before.device_bytes[0] + bytes);
                assert_eq!(pool.usage().in_flight[0], before.in_flight[0] + 1);
                assert_eq!(pool.usage().streams[0], before.streams[0] + 1);
            },
        );
    }
    #[test]
    fn required_metal_exclusive_preparation_reentry_and_completion_evidence() {
        isolated(
            "required_metal_exclusive_preparation_reentry_and_completion_evidence",
            || {
                let device = MTLCreateSystemDefaultDevice()
                    .expect("Metal custody qualification requires hardware");
                let pool = owner();
                let before = pool.usage();
                let buffer = MetalBuffer::allocate(&device, 19).expect("funded buffer");
                let queue = device.newCommandQueue().expect("Metal queue");
                let buffers = [&buffer];
                let mut first = Command::prepare(&queue, &buffers).expect("first exclusive owner");
                assert!(buffer.claimed.get());
                assert_eq!(
                    first.observe_completion(),
                    None,
                    "unsubmitted command is no completion evidence"
                );
                assert!(buffer.claimed.get());
                assert!(
                    Command::prepare(&queue, &buffers).is_none(),
                    "prepared overlap must refuse before construction"
                );
                assert_eq!(pool.usage().in_flight[0], before.in_flight[0] + 1);
                drop(first);
                assert!(
                    !buffer.claimed.get(),
                    "no-enqueue drop returns the original claim"
                );
                assert_eq!(pool.usage().in_flight[0], before.in_flight[0]);
                let mut first =
                    Command::prepare(&queue, &buffers).expect("claim reused after prepared drop");
                assert!(first.commit());
                assert!(!first.commit(), "repeat commit must not reach Metal");
                assert!(
                    Command::prepare(&queue, &buffers).is_none(),
                    "pending overlap must refuse"
                );
                assert!(matches!(
                    super::super::finalize_command_buffer(&mut first, "exclusive custody control"),
                    super::super::MetalCommandOutcome::Complete
                ));
                assert!(!first.commit(), "complete command cannot be enqueued again");
                let second = Command::prepare(&queue, &buffers)
                    .expect("observed completion releases buffer claim");
                drop(first);
                assert!(
                    buffer.claimed.get(),
                    "old completed drop must not release the new owner's claim"
                );
                assert!(Command::prepare(&queue, &buffers).is_none());
                drop(second);
                assert!(!buffer.claimed.get());
                assert_eq!(pool.usage().in_flight[0], before.in_flight[0]);
                drop(buffer);
                assert_eq!(pool.usage().unified_bytes[0], before.unified_bytes[0]);
            },
        );
    }
}
