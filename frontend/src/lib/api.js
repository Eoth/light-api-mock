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
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body.error) msg = body.error;
    } catch {}
    throw new Error(msg);
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

export function createService(service) {
  return request('POST', '/services', service);
}

export function updateService(name, service) {
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

export function resetConfig() {
  return request('DELETE', '/config/reset');
}
