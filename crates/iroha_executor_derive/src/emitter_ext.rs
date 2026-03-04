use manyhow::{Emitter, ToTokensError};
use proc_macro2::TokenStream;

/// Extension trait providing convenience methods for [`manyhow::Emitter`].
pub trait EmitterExt {
    fn handle<E: ToTokensError + 'static, T>(&mut self, result: manyhow::Result<T, E>)
    -> Option<T>;

    #[allow(dead_code)]
    fn handle_or_default<E: ToTokensError + 'static, T: Default>(
        &mut self,
        result: manyhow::Result<T, E>,
    ) -> T;

    fn finish_token_stream(self) -> TokenStream
    where
        Self: Sized;

    fn finish_token_stream_with(self, tokens: TokenStream) -> TokenStream
    where
        Self: Sized;
}

impl EmitterExt for Emitter {
    fn handle<E: ToTokensError + 'static, T>(
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

    fn handle_or_default<E: ToTokensError + 'static, T: Default>(
        &mut self,
        result: manyhow::Result<T, E>,
    ) -> T {
        self.handle(result).unwrap_or_default()
    }

    fn finish_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        self.finish_token_stream_with(TokenStream::new())
    }

    fn finish_token_stream_with(mut self, mut tokens: TokenStream) -> TokenStream
    where
        Self: Sized,
    {
        if let Err(err) = self.into_result() {
            err.to_tokens(&mut tokens);
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::Error;

    use super::*;

    #[test]
    fn handle_ok() {
        let mut e = Emitter::new();
        let value: Option<i32> = e.handle::<Error, _>(Ok(42));
        assert_eq!(value, Some(42));
        assert!(e.finish_token_stream().is_empty());
    }

    #[test]
    fn handle_err() {
        let mut e = Emitter::new();
        let value: Option<i32> =
            e.handle::<Error, _>(Err(Error::new(proc_macro2::Span::call_site(), "oops")));
        assert!(value.is_none());
        assert!(!e.finish_token_stream().is_empty());
    }

    #[test]
    fn handle_or_default_returns_default() {
        let mut e = Emitter::new();
        let value: i32 = e
            .handle_or_default::<Error, _>(Err(Error::new(proc_macro2::Span::call_site(), "oops")));
        assert_eq!(value, 0);
    }

    #[test]
    fn finish_token_stream_with_appends_tokens() {
        let mut e = Emitter::new();
        e.emit(Error::new(proc_macro2::Span::call_site(), "err"));
        let tokens = e.finish_token_stream_with(quote! { initial });
        let token_string = tokens.to_string();
        assert!(token_string.contains("initial"));
        assert!(token_string.len() > "initial".len());
    }
}
