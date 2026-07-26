/**
 * Docusaurus plugin that mounts the Torii OpenAPI reference page.
 *
 * The page renders the spec from `docs/portal/static/openapi/torii.json`
 * using a lightweight Redoc embed inside `ToriiOpenApiPage.jsx`.
 */
export default function toriiOpenApiPlugin() {
  return {
    name: 'torii-openapi-plugin',
    async contentLoaded({actions}) {
      actions.addRoute({
        path: '/reference/torii-openapi',
        component: '@site/src/components/ToriiOpenApiPage.jsx',
        exact: true,
      });
    },
  };
}
