// Client REST pour l'API backend Mimicway.
// Toutes les fonctions exportees appellent le backend via fetch().
// Le token Keycloak (si auth activee) est injecte automatiquement.
// En dev, le proxy Vite redirige /api vers http://localhost:7342.
//
// L'URL de base de l'API (getApiBaseUrl()) est '' par defaut (chemin relatif
// /api/..., derive du Host courant par le navigateur — comportement
// historique) sauf si /runtime-config.json en fournit une autre au
// demarrage de l'app (voir runtime-config.js) : permet de cibler une API
// exposee sur une origine distincte de celle qui sert la SPA.
import { auth, logout } from './auth.svelte.js';
import { getApiBaseUrl } from './runtime-config.js';
import { t, getLocale } from './i18n.svelte.js';

const BASE = '/api';

async function request(method, path, body) {
  const opts = {
    method,
    // The server words its error messages in the language of the interface.
    headers: { 'Content-Type': 'application/json', 'Accept-Language': getLocale() },
  };
  if (auth.token) {
    opts.headers['Authorization'] = `Bearer ${auth.token}`;
  }
  if (body !== undefined) {
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(`${getApiBaseUrl()}${BASE}${path}`, opts);
  if (res.status === 401 && auth.enabled) {
    logout();
    throw new Error(t("Session expired, please sign in again"));
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
//
// Un service est identifie sans ambiguite par (group_name, name) : le backend
// autorise deux services du meme nom dans des groupes differents (le nom
// seul ne suffit pas). `servicePath` est la source unique de
// verite pour construire le bon chemin : `/groups/:group/services/:name...`
// quand `groupName` est fourni (service groupe), `/services/:name...` sinon
// (perimetre "sans groupe").
function servicePath(name, groupName, suffix = '') {
  return groupName
    ? `/groups/${encodeURIComponent(groupName)}/services/${encodeURIComponent(name)}${suffix}`
    : `/services/${encodeURIComponent(name)}${suffix}`;
}

export function getServices() {
  return request('GET', '/services');
}

export function getService(name, groupName = null) {
  return request('GET', servicePath(name, groupName));
}

export function createService(service) {
  return request('POST', '/services', service);
}

export function updateService(name, groupName, service) {
  return request('PUT', servicePath(name, groupName), service);
}

export function deleteService(name, groupName = null) {
  return request('DELETE', servicePath(name, groupName));
}

export function toggleService(name, groupName, isMocked) {
  return request('PUT', servicePath(name, groupName, '/toggle'), { is_mocked: isMocked });
}

export function pingService(name, groupName = null) {
  return request('POST', servicePath(name, groupName, '/ping'));
}

export function reorderRules(serviceName, groupName, order) {
  return request('PUT', servicePath(serviceName, groupName, '/rules/reorder'), { order });
}

// Observation de trafic proxy (niveau service, is_mocked=false uniquement) :
// active/desactive EXPLICITEMENT par l'utilisateur, jamais automatique.
// getObservationStatus() liste TOUS les services actuellement observes
// (visibles par l'utilisateur courant, filtre cote backend) — pas de
// endpoint "statut d'un seul service", le composant filtre localement.
export function observeService(name, groupName = null) {
  return request('POST', servicePath(name, groupName, '/observe'));
}

export function unobserveService(name, groupName = null) {
  return request('DELETE', servicePath(name, groupName, '/observe'));
}

export function getObservationStatus() {
  return request('GET', '/observation/status');
}

// Recalculee a la demande cote backend (aucun etat en cache) : chaque appel
// relit le trafic reellement observe depuis le dernier appel a
// observeService(). `outcome` de chaque element : "Unconditional" (une seule
// regle, sans condition), "Conditional" (une regle par valeur distincte
// d'un champ discriminant), "VarianceUnexplained" (signal seul, rien a
// proposer).
export function getServiceSuggestions(name, groupName = null) {
  return request('GET', servicePath(name, groupName, '/suggestions'));
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

// Testeur de regle : rejeu en lecture seule d'un brouillon de regle (pas
// necessairement sauvegarde) contre une requete deja capturee dans les logs.
// Endpoint stateless, non scope par service (aucun service n'est charge cote
// backend) — pas de servicePath() ici.
export function testRule(payload) {
  return request('POST', '/rule-test', payload);
}

// Detecteur de conflit entre regles, appele a la SAUVEGARDE d'une regle
// (RuleForm) : compare le brouillon aux autres regles du service et signale
// les chevauchements evidents (memes conditions, ou conditions incluses),
// sans jamais bloquer la sauvegarde. Endpoint stateless comme /rule-test,
// non scope par service (aucun service n'est charge cote backend) — pas de
// servicePath() ici.
export function checkRuleConflicts(payload) {
  return request('POST', '/rule-conflicts', payload);
}

// Messaging (Kafka) — routes absentes (404) sur un binaire compile sans la
// feature "messaging-kafka" ; les appelants doivent gerer cet echec (voir
// App.svelte, verification au demarrage).
export function getMessagingStatus() {
  return request('GET', '/messaging/status');
}

export function getMessagingLogs(limit = 200) {
  return request('GET', `/messaging/logs?limit=${limit}`);
}

export function simulateMessage(topic, payload, headers = {}) {
  return request('POST', '/messaging/simulate', { topic, payload, headers });
}

export function validateScript(script) {
  return request('POST', '/script/validate', { script });
}

// TCP brut (protocoles binaires non-HTTP, mock seul — pas de proxy). Routes
// absentes (404) sur un binaire compile sans la feature "tcp-mock" ; les
// appelants doivent gerer cet echec (meme pattern que Messaging, voir
// App.svelte). /tcp/status est en lecture seule et sans auth cote backend ;
// /tcp/services est le CRUD, mutation reservee aux super-admins si l'auth
// est active.
export function getTcpStatus() {
  return request('GET', '/tcp/status');
}

export function getTcpServices() {
  return request('GET', '/tcp/services');
}

export function createTcpService(service) {
  return request('POST', '/tcp/services', service);
}

export function updateTcpService(name, service) {
  return request('PUT', `/tcp/services/${encodeURIComponent(name)}`, service);
}

export function deleteTcpService(name) {
  return request('DELETE', `/tcp/services/${encodeURIComponent(name)}`);
}

export function resetConfig() {
  return request('DELETE', '/config/reset');
}

export function getBackups() {
  return request('GET', '/config/backups');
}

export function restoreBackup(filename) {
  return request('POST', `/config/restore/${encodeURIComponent(filename)}`);
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
