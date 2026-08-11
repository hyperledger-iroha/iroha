const apiBase = process.env.VITE_PUBLIC_API_BASE;
const dataMode = process.env.VITE_DATA_MODE;

function fail(message) {
  console.error(`split-app frontend build validation failed: ${message}`);
  process.exit(1);
}

if (apiBase !== "/api") {
  fail("VITE_PUBLIC_API_BASE must be exactly '/api' for production builds.");
}

if (typeof apiBase === "string" && /^(https?:)?\/\//i.test(apiBase)) {
  fail("VITE_PUBLIC_API_BASE must stay same-host and must not be an absolute URL.");
}

if (dataMode !== "live") {
  fail("VITE_DATA_MODE must be exactly 'live' for production builds.");
}

if (typeof dataMode === "string" && /demo|static/i.test(dataMode)) {
  fail("VITE_DATA_MODE must not point at demo or static data.");
}
