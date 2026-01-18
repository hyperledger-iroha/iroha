//! API which simplifies writing of smartcontracts
#![allow(unsafe_code)]

use std::{boxed::Box, fmt::Debug, ptr};

use data_model::{
    isi::BuiltInInstruction,
    prelude::*,
    query::{Query, parameters::ForwardCursor},
};
pub use iroha_data_model as data_model;
use iroha_data_model::query::{
    QueryOutputBatchBoxTuple, QueryRequest, QueryResponse, QueryWithParams, SingularQuery,
    SingularQueryBox, SingularQueryOutputBox,
    builder::{QueryBuilder, QueryExecutor},
};
use iroha_smart_contract_codec::decode_with_length_prefix_from_raw;
pub use iroha_smart_contract_derive::main;
use iroha_smart_contract_utils::encode_and_execute;
pub use iroha_smart_contract_utils::{DebugExpectExt, DebugUnwrapExt, dbg, dbg_panic};
use norito::NoritoSerialize;

#[doc(hidden)]
pub mod utils {
    //! Crate with utilities

    pub use iroha_smart_contract_utils::register_getrandom_err_callback;

    /// Get context for smart contract `main()` entrypoint.
    ///
    /// # Safety
    ///
    /// It's safe to call this function as long as it's safe to construct, from the given
    /// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
    #[doc(hidden)]
    #[cfg(not(test))]
    pub unsafe fn __decode_smart_contract_context(
        context: *const u8,
    ) -> crate::data_model::smart_contract::payloads::SmartContractContext {
        unsafe { iroha_smart_contract_codec::decode_with_length_prefix_from_raw(context) }
    }
}

pub mod log {
    //! IVM runtime logging utilities
    pub use iroha_smart_contract_utils::{debug, error, event, info, trace, warn};
}

/// An iterable query cursor for use in smart contracts.
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct QueryCursor {
    cursor: ForwardCursor,
}

/// Client for the host environment
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[allow(missing_copy_implementations)]
pub struct Iroha;

impl Iroha {
    /// Submits one Iroha Special Instruction
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit<I: BuiltInInstruction + NoritoSerialize>(
        &self,
        isi: &I,
    ) -> Result<(), ValidationFail> {
        self.submit_all([isi])
    }

    /// Submits several Iroha Special Instructions
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    #[expect(clippy::unused_self)]
    pub fn submit_all<'isi, I: BuiltInInstruction + NoritoSerialize + 'isi>(
        &self,
        instructions: impl IntoIterator<Item = &'isi I>,
    ) -> Result<(), ValidationFail> {
        instructions.into_iter().try_for_each(|isi| {
            #[cfg(not(test))]
            use host::execute_instruction as host_execute_instruction;
            #[cfg(test)]
            use tests::_iroha_smart_contract_execute_instruction_mock as host_execute_instruction;

            let bytes = isi.encode_as_instruction_box();
            // Safety: `host_execute_instruction` doesn't take ownership of it's pointer parameter
            unsafe {
                decode_with_length_prefix_from_raw::<Result<_, ValidationFail>>(
                    host_execute_instruction(bytes.as_ptr(), bytes.len()),
                )
            }
        })?;

        Ok(())
    }

    /// Build an iterable query for execution in a smart contract.
    pub fn query<Q>(&self, query: Q) -> QueryBuilder<'_, Self, Q, Q::Item>
    where
        Q: Query,
    {
        QueryBuilder::new(self, query)
    }

    /// Run a singular query in a smart contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub fn query_single<Q>(&self, query: Q) -> Result<Q::Output, ValidationFail>
    where
        Q: SingularQuery,
        SingularQueryBox: From<Q>,
        Q::Output: TryFrom<SingularQueryOutputBox>,
        <Q::Output as TryFrom<SingularQueryOutputBox>>::Error: Debug,
    {
        let query = SingularQueryBox::from(query);

        let result = self.execute_singular_query(query)?;

        Ok(result
            .try_into()
            .expect("BUG: iroha returned unexpected type in singular query"))
    }

    fn execute_query(query: &QueryRequest) -> Result<QueryResponse, ValidationFail> {
        #[cfg(not(test))]
        use host::execute_query as host_execute_query;
        #[cfg(test)]
        use tests::_iroha_smart_contract_execute_query_mock as host_execute_query;

        // Safety: - `host_execute_query` doesn't take ownership of it's pointer parameter
        //         - ownership of the returned result is transferred into the decode below
        unsafe {
            // Receive Norito-framed bytes with a usize length prefix added by host.
            let ptr = encode_and_execute(query, host_execute_query);
            let len_size_bytes = core::mem::size_of::<usize>();
            let len = usize::from_le_bytes(
                core::slice::from_raw_parts(ptr, len_size_bytes)
                    .try_into()
                    .expect("Prefix length size(bytes) incorrect. This is a bug."),
            );
            let bytes = Box::from_raw(core::ptr::slice_from_raw_parts_mut(ptr.cast_mut(), len));
            let payload = &bytes[len_size_bytes..];

            // 1) Try bare Norito decode (codec::Decode) first for robust interop.
            if let Ok(val) =
                <Result<QueryResponse, ValidationFail> as norito::codec::DecodeAll>::decode_all(
                    &mut &*payload,
                )
            {
                return val;
            }

            // 2) Fallback to header-framed Norito decode guarded against panics from
            //    hybrid layout invariants. If this still fails, surface a decode error.
            let framed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                norito::from_bytes::<Result<QueryResponse, ValidationFail>>(payload).map(|archived| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        <Result<QueryResponse, ValidationFail> as norito::core::NoritoDeserialize>::deserialize(
                            archived,
                        )
                    }))
                })
            }));

            match framed {
                Ok(Ok(Ok(val))) => val,
                _ => {
                    // Final attempt: strict bare decode with error context
                    <Result<QueryResponse, ValidationFail> as norito::codec::DecodeAll>::decode_all(
                        &mut &*payload,
                    )
                    .expect("Decoding of Result<QueryResponse, ValidationFail> failed")
                }
            }
        }
    }
}

impl QueryExecutor for Iroha {
    type Cursor = QueryCursor;
    type Error = ValidationFail;

    fn execute_singular_query(
        &self,
        query: SingularQueryBox,
    ) -> Result<SingularQueryOutputBox, Self::Error> {
        let QueryResponse::Singular(output) = Self::execute_query(&QueryRequest::Singular(query))?
        else {
            dbg_panic!("BUG: iroha returned unexpected type in singular query");
        };

        Ok(output)
    }

    fn start_query(
        &self,
        query: QueryWithParams,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
        let QueryResponse::Iterable(output) = Self::execute_query(&QueryRequest::Start(query))?
        else {
            dbg_panic!("BUG: iroha returned unexpected type in iterable query");
        };

        let (batch, remaining_items, cursor) = output.into_parts();
        // Return the forwarded cursor unchanged for in-contract iteration.
        // Contracts are expected to consume cursors within the same execution.
        let cursor = cursor.map(|c| QueryCursor { cursor: c });
        Ok((batch, remaining_items, cursor))
    }

    fn continue_query(
        cursor: Self::Cursor,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
        let QueryResponse::Iterable(output) =
            Self::execute_query(&QueryRequest::Continue(cursor.cursor))?
        else {
            dbg_panic!("BUG: iroha returned unexpected type in iterable query");
        };

        let (batch, remaining_items, cursor) = output.into_parts();

        Ok((
            batch,
            remaining_items,
            cursor.map(|cursor| QueryCursor { cursor }),
        ))
    }
}

#[unsafe(no_mangle)]
extern "C" fn _iroha_smart_contract_alloc(len: usize) -> *const u8 {
    if len == 0 {
        dbg_panic!("Cannot allocate 0 bytes");
    }
    let layout = std::alloc::Layout::array::<u8>(len).dbg_expect("Cannot allocate layout");
    // Safety: safe because `layout` is guaranteed to have non-zero size
    unsafe { std::alloc::alloc_zeroed(layout) }
}

/// # Safety
/// - `offset` is a pointer to a `[u8; len]` which is allocated in the IVM memory.
/// - This function can't call destructor of the encoded object.
#[unsafe(no_mangle)]
unsafe extern "C" fn _iroha_smart_contract_dealloc(offset: *mut u8, len: usize) {
    let _ = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(offset, len)) };
}

#[cfg(not(test))]
mod host {
    unsafe extern "C" {
        /// Execute encoded query by providing offset and length
        /// into the IVM's linear memory where query is stored
        ///
        /// # Warning
        ///
        /// This function doesn't take ownership of the provided allocation
        /// but it does transfer ownership of the result to the caller
        pub(super) fn execute_query(ptr: *const u8, len: usize) -> *const u8;

        /// Execute encoded instruction by providing offset and length
        /// into the IVM's linear memory where instruction is stored
        ///
        /// # Warning
        ///
        /// This function doesn't take ownership of the provided allocation
        /// but it does transfer ownership of the result to the caller
        pub(super) fn execute_instruction(ptr: *const u8, len: usize) -> *const u8;
    }
}

/// Most used items
pub mod prelude {
    pub use crate::{
        DebugExpectExt, DebugUnwrapExt, Iroha,
        data_model::{prelude::*, smart_contract::payloads::SmartContractContext as Context},
        dbg, dbg_panic,
    };
}

#[cfg(test)]
mod tests {
    use std::{mem::ManuallyDrop, slice};

    use data_model::isi::Log;
    use iroha_data_model::query::{QueryOutput, QueryOutputBatchBox};

    use super::*;
    // Removed unused import; tests perform explicit framing locally.

    const ISI_RESULT: Result<(), ValidationFail> = Ok(());

    fn get_test_instruction() -> Log {
        Log::new(Level::INFO, "test instruction".to_owned())
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn _iroha_smart_contract_execute_instruction_mock(
        ptr: *const u8,
        len: usize,
    ) -> *const u8 {
        let _ = unsafe { slice::from_raw_parts(ptr, len) };

        // Encode response (Result<(), ValidationFail>) as Norito header-framed with length prefix
        let body = norito::to_bytes(&ISI_RESULT).expect("encode resp");
        let len_size = core::mem::size_of::<usize>();
        let mut out = Vec::with_capacity(len_size + body.len());
        out.extend_from_slice(&(len_size + body.len()).to_le_bytes());
        out.extend_from_slice(&body);
        ManuallyDrop::new(out.into_boxed_slice()).as_ptr()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn _iroha_smart_contract_execute_query_mock(
        ptr: *const u8,
        len: usize,
    ) -> *const u8 {
        let bytes = unsafe { slice::from_raw_parts(ptr, len) };
        let query_request: QueryRequest =
            norito::codec::Decode::decode(&mut &*bytes).expect("decode query request");

        let response: Result<QueryResponse, ValidationFail> = Ok(match query_request {
            QueryRequest::Singular(_) => {
                QueryResponse::Singular(SingularQueryOutputBox::Parameters(Parameters::default()))
            }
            QueryRequest::Start(_) | QueryRequest::Continue(_) => {
                QueryResponse::Iterable(QueryOutput::new(
                    QueryOutputBatchBoxTuple::new(vec![
                        QueryOutputBatchBox::Permission(Vec::new()),
                    ]),
                    0,
                    None,
                ))
            }
        });
        // Encode response using Norito core with explicit flags to match
        // the adaptive bare decode expectations (packed-struct + compact-len
        // with field bitset and fixed-width offsets).
        let mut body = Vec::new();
        {
            use norito::core::{DecodeFlagsGuard, NoritoSerialize as _, header_flags};
            let mut flags: u8 = 0;
            // Prefer hybrid packed-struct layout with compact lengths.
            flags |= header_flags::PACKED_STRUCT;
            flags |= header_flags::COMPACT_LEN;
            flags |= header_flags::FIELD_BITSET;
            let _guard = DecodeFlagsGuard::enter(flags);
            response.serialize(&mut body).expect("encode resp");
        }
        let len_size = core::mem::size_of::<usize>();
        let mut out = Vec::with_capacity(len_size + body.len());
        out.extend_from_slice(&(len_size + body.len()).to_le_bytes());
        out.extend_from_slice(&body);
        ManuallyDrop::new(out.into_boxed_slice()).as_ptr()
    }

    #[test]
    fn execute_instruction() {
        let host = Iroha;
        host.submit(&get_test_instruction()).unwrap();
    }

    #[test]
    fn execute_query() {
        let host = Iroha;
        let params: Parameters = host.query_single(FindParameters).unwrap();
        assert_eq!(params, Parameters::default());
    }
}
