import { mount } from 'svelte';
import App from './App.svelte';
import { loadRuntimeConfig } from './lib/runtime-config.js';
import './tokens-aurora.css';
import './app.css';

// Charge la configuration runtime (URL de base de l'API, cf
// lib/runtime-config.js) AVANT le montage de l'app : tout appel API declenche
// par le montage (ex. GET /api/auth/status) doit deja connaitre la bonne
// cible. IIFE plutot qu'un top-level await : la cible de build esbuild
// configuree par Vite ne le supporte pas (ecrase silencieusement en
// production sinon).
(async () => {
  await loadRuntimeConfig();
  mount(App, { target: document.getElementById('app') });
})();
