const BASE = '/api';

async function request(method, path, body) {
  const opts = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (body !== undefined) {
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(`${BASE}${path}`, opts);
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText}`);
  }
  if (res.status === 204) return null;
  return res.json();
}

export function getServices() {
  return request('GET', '/services');
}

export function getService(name) {
  return request('GET', `/services/${encodeURIComponent(name)}`);
}

export function putService(name, service) {
  return request('PUT', `/services/${encodeURIComponent(name)}`, service);
}

export function deleteService(name) {
  return request('DELETE', `/services/${encodeURIComponent(name)}`);
}

export function toggleService(name, isMocked) {
  return request('PUT', `/services/${encodeURIComponent(name)}/toggle`, { is_mocked: isMocked });
}

export function reorderRules(serviceName, order) {
  return request('PUT', `/services/${encodeURIComponent(serviceName)}/rules/reorder`, { order });
}

export function getConfig() {
  return request('GET', '/config');
}

export function putConfig(config) {
  return request('PUT', '/config', config);
}

export function getLogs(limit = 50) {
  return request('GET', `/logs?limit=${limit}`);
}
