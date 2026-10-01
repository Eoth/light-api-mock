// Configuration read when the UI starts, before the app is mounted (main.js) and before any call through api.js: the
// base URL of the API, for an infrastructure that routes /api to another origin than the one serving the UI (e.g.
// Kubernetes with Gloo Edge, a VirtualService for the UI and a separate route table or upstream for the API). The
// server reads it from its environment (API_BASE_URL), so one build serves every deployment.
//
// Served at /runtime-config.json, outside /api: this file tells the UI where the API is, so it must come through the
// same route as index.html and the bundle.
//
// '' when the file is missing, unreachable or invalid: relative /api/... URLs on the current host, what a deployment
// serving the UI and the API together needs, without any configuration.
let apiBaseUrl = '';

export async function loadRuntimeConfig() {
  try {
    const res = await fetch('/runtime-config.json');
    if (res.ok) {
      const data = await res.json();
      apiBaseUrl = (data.api_base_url || '').trim().replace(/\/+$/, '');
    }
  } catch {
    // Server unreachable or invalid answer: keep '' (relative URLs); the app starts all the same.
  }
}

export function getApiBaseUrl() {
  return apiBaseUrl;
}

// For tests only: `apiBaseUrl` lives in the module, not in a component, so it would carry over from one test to the
// next.
export function _resetForTests() {
  apiBaseUrl = '';
}
