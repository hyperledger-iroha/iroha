//! Attribute-like macro for instrumenting `isi` for `prometheus`
//! metrics. See [`macro@metrics`] for more details.

mod emitter_ext;

use emitter_ext::EmitterExt;
use manyhow::{Emitter, Result, emit, manyhow};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{FnArg, LitStr, Path, Type, parse::Parse, punctuated::Punctuated, token::Comma};

// Procedural macro crates cannot export ordinary constants, so dedicated
// `metric_*_label!` helpers below provide a public surface for downstream crates.

/// Emit the canonical "total" metric label as a string literal.
///
/// Usage: `metric_total_label!()` expands to `"total"`.
/// The macro accepts no input tokens.
#[cfg(feature = "metric-instrumentation")]
#[proc_macro]
pub fn metric_total_label(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if !input.is_empty() {
        return "compile_error!(\"metric_total_label!() takes no arguments\");"
            .parse()
            .expect("valid TokenStream");
    }
    "\"total\"".parse().expect("valid TokenStream")
}

/// Emit the canonical "success" metric label as a string literal.
///
/// Usage: `metric_success_label!()` expands to `"success"`.
/// The macro accepts no input tokens.
#[cfg(feature = "metric-instrumentation")]
#[proc_macro]
pub fn metric_success_label(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if !input.is_empty() {
        return "compile_error!(\"metric_success_label!() takes no arguments\");"
            .parse()
            .expect("valid TokenStream");
    }
    "\"success\"".parse().expect("valid TokenStream")
}

fn type_has_metrics_field(ty: &Type) -> bool {
    match ty {
        // This may seem fragile, but it isn't. We use the same convention
        // everywhere in the code base, and if you follow `CONTRIBUTING.md`
        // you'll likely have `use iroha_core::{StateTransaction, StateSnapshot}`
        // somewhere. If you don't, you're violating the `CONTRIBUTING.md` in
        // more than one way.
        Type::Path(pth) => {
            let Path { segments, .. } = pth.path.clone();
            let type_name = &segments
                .last()
                .expect("Should have at least one segment")
                .ident;
            *type_name == "StateTransaction"
        }
        Type::ImplTrait(impl_trait) => impl_trait.bounds.iter().any(|bounds| match bounds {
            syn::TypeParamBound::Trait(trt) => {
                let Path { segments, .. } = trt.path.clone();
                let type_name = &segments
                    .last()
                    .expect("Should have at least one segment")
                    .ident;
                *type_name == "StateReadOnly"
            }
            _ => false,
        }),
        _ => false,
    }
}

/// The identifier of the first argument that has a type which has
/// metrics.
///
/// # Errors
/// If no argument is of type `StateTransaction` of `StateSnapshot`.
fn arg_metrics(input: &Punctuated<FnArg, Comma>) -> Result<syn::Ident, &Punctuated<FnArg, Comma>> {
    input
        .iter()
        .find(|arg| match arg {
            FnArg::Typed(typ) => match &*typ.ty {
                syn::Type::Reference(typ) => type_has_metrics_field(&typ.elem),
                _ => false,
            },
            _ => false,
        })
        .and_then(|arg| match arg {
            FnArg::Typed(typ) => match *typ.pat.clone() {
                syn::Pat::Ident(ident) => Some(ident.ident),
                _ => None,
            },
            _ => None,
        })
        .ok_or(input)
}

struct MetricSpecs(#[allow(dead_code)] Vec<MetricSpec>); // `HashSet` — idiomatic; slow

impl Parse for MetricSpecs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vars = Punctuated::<MetricSpec, Comma>::parse_terminated(input)?;
        Ok(Self(vars.into_iter().collect()))
    }
}

struct MetricSpec {
    metric_name: LitStr,
    #[cfg(feature = "metric-instrumentation")]
    timing: bool,
}

impl Parse for MetricSpec {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let timing_requested = <syn::Token![+]>::parse(input).is_ok();
        let metric_name_lit = syn::Lit::parse(input)?;

        let metric_name = match metric_name_lit {
            syn::Lit::Str(lit_str) => {
                if lit_str.value().contains(' ') {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Spaces are not allowed. Use underscores '_'",
                    ));
                }
                lit_str
            }
            _ => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "Must be a string literal. Format `[+]\"name_of_metric\"`.",
                ));
            }
        };
        #[cfg(not(feature = "metric-instrumentation"))]
        let _ = timing_requested;
        Ok(Self {
            metric_name,
            #[cfg(feature = "metric-instrumentation")]
            timing: timing_requested,
        })
    }
}

impl ToTokens for MetricSpec {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.metric_name.to_tokens(tokens);
    }
}

/// Macro for instrumenting an `isi`'s `impl execute` to track a given
/// metric.  To specify a metric, put it as an attribute parameter
/// inside quotes.
///
/// This will increment the `prometheus::IntVec` metric
/// corresponding to the literal provided in quotes, with the second
/// argument being `METRIC_TOTAL_LABEL == "total"`. If the execution of the
/// `Fn`'s body doesn't result in an [`Err`] variant, another metric
/// with the same first argument and `METRIC_SUCCESS_LABEL = "success"` is also
/// incremented. Thus one can infer the number of rejected
/// transactions based on this parameter. If necessary, this macro
/// should be edited to record different [`Err`] variants as different
/// rejections, so we could (in theory), record the number of
/// transactions that got rejected because of
/// e.g. `SignatureCondition` failure.
///
/// If you also want to track the execution time of the `isi`, you
/// should prefix the quoted metric with the `+` symbol. Timing metrics
/// are emitted only when the `metric-instrumentation` feature is enabled;
/// without it the `+` prefix is accepted but timings are a no-op.
///
/// # Examples
///
/// ```rust
/// use iroha_telemetry_derive::metrics;
///
/// # struct DummyCounter;
/// # impl DummyCounter {
/// #     fn with_label_values(&self, _: &[&str]) -> DummyCounterHandle {
/// #         DummyCounterHandle
/// #     }
/// # }
/// # struct DummyCounterHandle;
/// # impl DummyCounterHandle {
/// #     fn inc(&self) {}
/// # }
/// # struct DummyHistogram;
/// # impl DummyHistogram {
/// #     fn with_label_values(&self, _: &[&str]) -> DummyHistogramHandle {
/// #         DummyHistogramHandle
/// #     }
/// # }
/// # struct DummyHistogramHandle;
/// # impl DummyHistogramHandle {
/// #     fn observe(&self, _: f64) {}
/// # }
/// # struct Metrics {
/// #     isi: DummyCounter,
/// #     isi_times: DummyHistogram,
/// # }
/// # impl Default for Metrics {
/// #     fn default() -> Self {
/// #         Self {
/// #             isi: DummyCounter,
/// #             isi_times: DummyHistogram,
/// #         }
/// #     }
/// # }
/// # struct StateTransaction {
/// #     metrics: Metrics,
/// # }
///
/// #[metrics(+"test_query", "another_test_query_without_timing")]
/// fn execute(state: &StateTransaction) -> Result<(), ()> {
///     Ok(())
/// }
/// ```
#[manyhow]
#[proc_macro_attribute]
pub fn metrics(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(func): Option<syn::ItemFn> = emitter.handle(syn::parse2(item)) else {
        return emitter.finish_token_stream();
    };

    // This is a good sanity check. Possibly redundant.
    if func.sig.ident != "execute" {
        emit!(
            emitter,
            func.sig.ident,
            "Function should be an `impl execute`"
        );
    }

    if func.sig.inputs.is_empty() {
        emit!(
            emitter,
            func.sig,
            "Function must have at least one argument of type `StateTransaction` or `StateReadOnly`."
        );
        return emitter.finish_token_stream();
    }

    let Some(metric_specs): Option<MetricSpecs> = emitter.handle(syn::parse2(attr)) else {
        return emitter.finish_token_stream();
    };

    let result = impl_metrics(&mut emitter, &metric_specs, &func);

    emitter.finish_token_stream_with(result)
}

fn impl_metrics(emitter: &mut Emitter, specs: &MetricSpecs, func: &syn::ItemFn) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = func;

    match sig.output.clone() {
        syn::ReturnType::Default => emit!(
            emitter,
            sig.output,
            "`Fn` must return `Result`. Returns nothing instead. "
        ),
        #[allow(clippy::implicit_clone)]
        syn::ReturnType::Type(_, typ) => match *typ {
            Type::Path(pth) => {
                let Path { segments, .. } = pth.path;
                let type_name = &segments.last().expect("non-empty path").ident;
                if *type_name != "Result" {
                    emit!(
                        emitter,
                        type_name,
                        "Should return `Result`. Found {}",
                        type_name
                    );
                }
            }
            _ => emit!(
                emitter,
                typ,
                "Should return `Result`. Non-path result type specification found"
            ),
        },
    }

    // Again this may seem fragile, but if we move the metrics from
    // the `WorldStateView`, we'd need to refactor many things anyway
    #[cfg_attr(not(feature = "metric-instrumentation"), allow(unused_variables))]
    let metric_arg_ident = match arg_metrics(&sig.inputs) {
        Ok(ident) => ident,
        Err(args) => {
            emit!(
                emitter,
                args,
                "At least one argument must be a `StateTransaction` or `StateReadOnly`."
            );
            return quote!();
        }
    };

    #[cfg(feature = "metric-instrumentation")]
    let generated = {
        let (totals, successes, times, needs_timing) = write_metrics(&metric_arg_ident, specs);
        let timing_start = if needs_timing {
            quote!(
                #[cfg(feature = "telemetry")]
                let started_at = std::time::Instant::now();
            )
        } else {
            TokenStream::new()
        };
        quote!(
            #(#attrs)* #vis #sig {
                #timing_start
                let res = (|| #block)();
                #times
                #totals
                if res.is_ok() {
                    #successes
                }
                res
            }
        )
    };

    #[cfg(not(feature = "metric-instrumentation"))]
    let generated = {
        let _ = specs;
        quote!(
            #(#attrs)* #vis #sig {
                #block
            }
        )
    };

    generated
}

// NOTE: metrics wiring stays behind `metric-instrumentation` so builds can
// opt in to ISI counters/timing without forcing telemetry overhead by default.
#[cfg(feature = "metric-instrumentation")]
fn write_metrics(
    metric_arg_ident: &proc_macro2::Ident,
    specs: &MetricSpecs,
) -> (TokenStream, TokenStream, TokenStream, bool) {
    let record_total = |spec: &MetricSpec| {
        let body = quote!(#metric_arg_ident.metrics().record_isi_total(#spec););
        wrap_metrics(metric_arg_ident, &body)
    };
    let record_success = |spec: &MetricSpec| {
        let body = quote!(#metric_arg_ident.metrics().record_isi_success(#spec););
        wrap_metrics(metric_arg_ident, &body)
    };
    let record_time = |spec: &MetricSpec| {
        let body =
            quote!(#metric_arg_ident.metrics().record_isi_time(#spec, started_at.elapsed()););
        wrap_metrics(metric_arg_ident, &body)
    };
    let totals: TokenStream = specs.0.iter().map(record_total).collect();
    let successes: TokenStream = specs.0.iter().map(record_success).collect();
    let needs_timing = specs.0.iter().any(|spec| spec.timing);
    let times: TokenStream = specs
        .0
        .iter()
        .filter(|spec| spec.timing)
        .map(record_time)
        .collect();
    (totals, successes, times, needs_timing)
}

#[cfg(feature = "metric-instrumentation")]
fn wrap_metrics(metric_arg_ident: &proc_macro2::Ident, body: &TokenStream) -> TokenStream {
    quote!(
        #[cfg(feature = "telemetry")]
        {
            #body
        }
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = &#metric_arg_ident;
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_spec_parses_plus_prefix() {
        let spec: MetricSpec = syn::parse_str("+\"timed_metric\"").expect("parse metric spec");
        assert_eq!(spec.metric_name.value(), "timed_metric");
        #[cfg(feature = "metric-instrumentation")]
        assert!(spec.timing);
    }

    #[test]
    fn metric_spec_parses_plain_name() {
        let spec: MetricSpec = syn::parse_str("\"plain_metric\"").expect("parse metric spec");
        assert_eq!(spec.metric_name.value(), "plain_metric");
        #[cfg(feature = "metric-instrumentation")]
        assert!(!spec.timing);
    }

    #[cfg(feature = "metric-instrumentation")]
    #[test]
    fn wrap_metrics_emits_cfg_blocks() {
        let metric_ident = proc_macro2::Ident::new("state_tx", proc_macro2::Span::call_site());
        let body = quote!(state_tx.metrics().record_isi_total("x"););
        let wrapped = wrap_metrics(&metric_ident, &body);
        let compact: String = wrapped
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        assert!(compact.contains("record_isi_total"));
        assert!(compact.contains("cfg(feature=\"telemetry\")"));
        assert!(compact.contains("cfg(not(feature=\"telemetry\"))"));
        assert!(compact.contains("let_=&state_tx;"));
    }
}
