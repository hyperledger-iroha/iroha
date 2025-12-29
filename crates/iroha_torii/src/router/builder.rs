//! Minimal builder for composing Torii routers in statement form.

use axum::Router;

use crate::SharedAppState;

/// Convenience wrapper around `Router<SharedAppState>` that supports
/// step-by-step composition without repeated reassignments inside
/// [`Torii::create_api_router`](crate::Torii::create_api_router).
pub struct RouterBuilder {
    router: Router<SharedAppState>,
    app_state: SharedAppState,
}

impl RouterBuilder {
    /// Construct a builder starting from an empty router with v0.7
    /// compatibility checks disabled.
    #[must_use]
    pub fn new(app_state: SharedAppState) -> Self {
        Self::from_router(Router::new().without_v07_checks(), app_state)
    }

    /// Construct a builder from an existing router and shared state.
    #[must_use]
    pub fn from_router(router: Router<SharedAppState>, app_state: SharedAppState) -> Self {
        Self { router, app_state }
    }

    /// Apply a transformation that only needs the router.
    pub fn apply<F>(&mut self, transform: F)
    where
        F: FnOnce(Router<SharedAppState>) -> Router<SharedAppState>,
    {
        self.router = transform(std::mem::take(&mut self.router));
    }

    /// Apply a transformation that also needs a clone of the shared state.
    pub fn apply_with_state<F>(&mut self, transform: F)
    where
        F: FnOnce(Router<SharedAppState>, SharedAppState) -> Router<SharedAppState>,
    {
        let state = self.app_state.clone();
        self.apply(|router| transform(router, state));
    }

    /// Get a reference to the shared state held by the builder.
    #[must_use]
    pub fn state(&self) -> &SharedAppState {
        &self.app_state
    }

    /// Finish composition and return the built router.
    #[must_use = "use the returned router to serve requests"]
    pub fn finish(self) -> Router<SharedAppState> {
        self.router
    }
}
