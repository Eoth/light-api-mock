// Client REST pour l'API backend lightMock.
// Toutes les fonctions exportees appellent le backend via fetch().
// Le token Keycloak (si auth activee) est injecte automatiquement.
// En dev, le proxy Vite redirige /api vers http://localhost:7342.
import { auth, logout } from './auth.svelte.js';

const BASE = '/api';

async function request(method, path, body) {
  const opts = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (auth.token) {
    opts.headers['Authorization'] = `Bearer ${auth.token}`;
  }
  if (body !== undefined) {
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(`${BASE}${path}`, opts);
  if (res.status === 401 && auth.enabled) {
    logout();
    throw new Error('Session expiree, veuillez vous reconnecter');
  }
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

// Auth
export function getAuthStatus() {
  return request('GET', '/auth/status');
}

export function login(username, password) {
  return request('POST', '/auth/login', { username, password });
}

export function validateToken(token) {
  return request('POST', '/auth/validate', { token });
}

export function getMe() {
  return request('GET', '/auth/me');
}

// Services
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

// Config
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

// Groups
export function getGroups() {
  return request('GET', '/groups');
}

export function createGroup(group) {
  return request('POST', '/groups', group);
}

export function updateGroup(name, group) {
  return request('PUT', `/groups/${encodeURIComponent(name)}`, group);
}

export function deleteGroup(name) {
  return request('DELETE', `/groups/${encodeURIComponent(name)}`);
}

export function updateGroupMembers(name, members) {
  return request('PUT', `/groups/${encodeURIComponent(name)}/members`, members);
}
