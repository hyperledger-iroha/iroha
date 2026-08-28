/// Return the explicit MCP decision for a cataloged method/path pair.
///
/// `None` means the operation is not represented by one of `groups`. Callers
/// which generate tools must treat that as deny: OpenAPI presence alone is not
/// authorization to expose a route through MCP.
fn catalog_mcp_projection_decision(
    groups: &[CatalogProjectionGroup],
    method: &Method,
    path: &str,
) -> Option<bool> {
    let method = catalog_method(method)?;
    for group in groups {
        let catalog = RouteCatalog::new(group.routes);
        let is_cataloged = catalog
            .routes()
            .iter()
            .any(|route| route.method() == method && route.path() == path);
        if !is_cataloged {
            continue;
        }
        return Some(
            catalog
                .project(CatalogProjection::Mcp, group.enabled_features)
                .into_iter()
                .any(|route| route.method() == method && route.path() == path),
        );
    }
    None
}

fn catalog_route_mounted_decision(
    groups: &[CatalogProjectionGroup],
    method: &Method,
    path: &str,
) -> Option<bool> {
    let method = catalog_method(method)?;
    for group in groups {
        let catalog = RouteCatalog::new(group.routes);
        let Some(route) = catalog
            .routes()
            .iter()
            .find(|route| route.method() == method && route.path() == path)
        else {
            continue;
        };
        return Some(route.feature_gate().is_enabled(group.enabled_features));
    }
    None
}

fn tool_requires_catalog_mcp_projection(name: &str) -> bool {
    name.starts_with("torii.")
}

fn retain_catalog_mcp_tools(tools: &mut Vec<ToolSpec>, groups: &[CatalogProjectionGroup]) {
    tools.retain(|tool| {
        if tool_requires_catalog_mcp_projection(&tool.name) {
            return catalog_mcp_projection_decision(
                groups,
                &tool.method,
                tool.path_template.as_str(),
            ) == Some(true);
        }
        catalog_route_mounted_decision(groups, &tool.method, tool.path_template.as_str())
            .unwrap_or(true)
    });
}
