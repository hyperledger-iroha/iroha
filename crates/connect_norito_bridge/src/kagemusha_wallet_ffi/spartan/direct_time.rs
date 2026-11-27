//! Future wallet exchange tokens and operation-bound Send intervals.
use super::*;

impl<P: advance::KagemushaWalletPlatformV1 + 'static, S> Session<P, S> {
    pub(super) fn direct_time(
        &mut self,
        slot: advance::KagemushaWalletSlotIdV1,
        selector: u32,
        token: u64,
        first: &[u8],
        second: &[u8],
    ) -> Result<Response> {
        match (selector, token) {
            (0, 0) if first.is_empty() && second.is_empty() => {
                self.times.capacity()?;
                let exchange = self.runtime.begin_direct_time_exchange(&slot)?;
                let nonce = exchange.nonce();
                let token = self.times.put(exchange)?;
                Ok(Response {
                    kind: 13,
                    sequence: u128::from(token),
                    bytes: nonce.to_vec(),
                    ..Response::default()
                })
            }
            (1, 1..) => {
                let exchange = self.times.take(token)?;
                let anchor = setup::decode(first)?;
                let certificate = setup::decode(second)?;
                self.runtime
                    .finish_direct_time_exchange(&slot, exchange, anchor, &certificate)?;
                Ok(response(14, Vec::new()))
            }
            (2, 1..) if first.is_empty() && second.is_empty() => {
                self.times.take(token)?;
                Ok(response(84, Vec::new()))
            }
            (3, 0) if second.is_empty() => {
                let operation = first.try_into().map_err(|_| Failure::code(INVALID))?;
                let interval = self.runtime.capture_send_time(&slot, operation)?;
                let mut bytes = Vec::with_capacity(16);
                bytes.extend_from_slice(&interval.lower_ms.to_le_bytes());
                bytes.extend_from_slice(&interval.upper_ms.to_le_bytes());
                Ok(response(86, bytes))
            }
            (4, 0) if second.is_empty() => {
                let operation = first.try_into().map_err(|_| Failure::code(INVALID))?;
                if self
                    .reviews
                    .find(|r| r.slot == slot && r.capsule.operation_id() == operation)
                    .is_some()
                {
                    return Err(Failure::code(CONFLICT));
                }
                self.runtime.abandon_send_capture(&slot, operation)?;
                Ok(response(84, Vec::new()))
            }
            _ => Err(Failure::code(INVALID)),
        }
    }
}

/// Start, finish or cancel a direct exchange, or capture or abandon a Send interval.
/// # Safety
/// `slot` addresses 32 bytes, inputs address their lengths, and `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_time_v1(
    handle: u64,
    slot: *const u8,
    selector: u32,
    token: u64,
    first: *const u8,
    first_len: usize,
    second: *const u8,
    second_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::DirectTime {
                    slot: fixed32(slot)?,
                    selector,
                    token,
                    first: exports::input(first, first_len, 512)?,
                    second: exports::input(second, second_len, 512)?,
                },
            )
        })
    }
}
