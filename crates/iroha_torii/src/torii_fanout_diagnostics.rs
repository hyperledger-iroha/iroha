#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ToriiFanoutDiagnostics {
    attempted_routes: usize,
    succeeded_routes: usize,
    denied_routes: usize,
    not_found_routes: usize,
    unavailable_routes: usize,
    first_failure_class: Option<&'static str>,
}

impl ToriiFanoutDiagnostics {
    fn record_attempt(&mut self) {
        self.attempted_routes = self.attempted_routes.saturating_add(1);
    }

    fn record_success(&mut self) {
        self.succeeded_routes = self.succeeded_routes.saturating_add(1);
    }

    fn record_denied(&mut self) {
        self.attempted_routes = self.attempted_routes.saturating_add(1);
        self.denied_routes = self.denied_routes.saturating_add(1);
        self.record_failure_class("permission_denied");
    }

    fn record_failure_class(&mut self, class: &'static str) {
        if self.first_failure_class.is_none() {
            self.first_failure_class = Some(class);
        }
    }

    fn record_skipped_response(&mut self, response: &Response) {
        if torii_response_has_reject_code(response, "route_unavailable") {
            self.unavailable_routes = self.unavailable_routes.saturating_add(1);
            self.record_failure_class("route_unavailable");
        } else if response.status() == StatusCode::NOT_FOUND {
            self.not_found_routes = self.not_found_routes.saturating_add(1);
            self.record_failure_class("not_found");
        } else {
            self.record_failure_class("error");
        }
    }

    fn failed_routes(self) -> usize {
        self.attempted_routes.saturating_sub(self.succeeded_routes)
    }
}

#[cfg(feature = "app_api")]
#[derive(Debug)]
struct ToriiFanoutJsonPayloads {
    payloads: Vec<Value>,
    diagnostics: ToriiFanoutDiagnostics,
    budget: ToriiRoutedReadMemoryBudget,
}

#[cfg(feature = "app_api")]
#[derive(Debug)]
struct ToriiFanoutRoutedJsonPayloads {
    payloads: Vec<(RoutingDecision, Value)>,
    diagnostics: ToriiFanoutDiagnostics,
    budget: ToriiRoutedReadMemoryBudget,
}

fn torii_alias_routes_denied_warning_header() -> HeaderValue {
    HeaderValue::from_static(r#"199 - "one or more alias routes were denied""#)
}

fn insert_torii_fanout_headers(response: &mut Response, diagnostics: ToriiFanoutDiagnostics) {
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-attempted",
        diagnostics.attempted_routes,
    );
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-succeeded",
        diagnostics.succeeded_routes,
    );
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-failed",
        diagnostics.failed_routes(),
    );
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-denied",
        diagnostics.denied_routes,
    );
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-unavailable",
        diagnostics.unavailable_routes,
    );
    insert_usize_header(
        response,
        "x-iroha-fanout-routes-not-found",
        diagnostics.not_found_routes,
    );
    if let Some(class) = diagnostics.first_failure_class {
        response.headers_mut().insert(
            HeaderName::from_static("x-iroha-fanout-first-failure"),
            HeaderValue::from_static(class),
        );
    }
}

fn with_torii_fanout_headers(
    mut response: Response,
    diagnostics: ToriiFanoutDiagnostics,
) -> Response {
    if diagnostics.denied_routes > 0 && response.status().is_success() {
        response.headers_mut().append(
            axum::http::header::WARNING,
            torii_alias_routes_denied_warning_header(),
        );
    }
    insert_torii_fanout_headers(&mut response, diagnostics);
    response
}
