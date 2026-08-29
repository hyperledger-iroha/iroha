//! A wrapper type around [`manyhow::Emitter`] that provides a more ergonomic API.
use drop_bomb::DropBomb;
use manyhow::ToTokensError;
use proc_macro2::TokenStream;
/// A wrapper type around [`manyhow::Emitter`] that provides a more ergonomic API.
///
/// This type accumulates errors during parsing and code generation. Call one of the
/// `finish*` methods before dropping to avoid a panic.
pub struct Emitter {
    inner: manyhow::Emitter,
    bomb: DropBomb,
}
impl Emitter {
    /// Creates a new emitter. Must be consumed before dropping or it will panic.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: manyhow::Emitter::new(),
            bomb: DropBomb::new("Emitter dropped without consuming accumulated errors"),
        }
    }
    /// Add a new error to the emitter.
    pub fn emit<E: ToTokensError + 'static>(&mut self, err: E) {
        self.inner.emit(err);
    }
    /// Handle a [`manyhow::Result`] by either returning the value or emitting the error.
    ///
    /// If the passed value is `Err`, the error will be emitted and `None` will be returned.
    pub fn handle<E: ToTokensError + 'static, T>(
        &mut self,
        result: manyhow::Result<T, E>,
    ) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                self.emit(err);
                None
            }
        }
    }
    /// Consume the emitter, returning a [`manyhow::Error`] if any errors were emitted.
    ///
    /// # Errors
    ///
    /// This function returns an error if the emitter has some errors accumulated.
    pub(crate) fn finish(mut self) -> manyhow::Result<()> {
        self.bomb.defuse();
        self.inner.into_result()
    }
    fn finish_to_token_stream(self, tokens: &mut TokenStream) {
        if let Err(error) = self.finish() {
            error.to_tokens(tokens);
        }
    }
    /// Consume the emitter, convert all errors into a token stream.
    pub fn finish_token_stream(self) -> TokenStream {
        self.finish_token_stream_with(TokenStream::new())
    }
    /// Consume the emitter, convert all errors into a token stream and append it to the given token stream.
    pub fn finish_token_stream_with(self, mut tokens: TokenStream) -> TokenStream {
        self.finish_to_token_stream(&mut tokens);
        tokens
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

// `manyhow::emit!` reports one or more diagnostics through `Extend`, so this
// implementation is part of the emitter's required integration surface.
impl<E: ToTokensError + 'static> Extend<E> for Emitter {
    fn extend<T: IntoIterator<Item = E>>(&mut self, iter: T) {
        self.inner.extend(iter);
    }
}
