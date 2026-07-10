import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const API = 'http://localhost:7342/api';

// Le store persiste desormais en write-behind : la mutation en memoire est
// instantanee, l'ecriture sur mock-config.yaml est deleguee a une tache de
// fond (cf CLAUDE.md, src/store/mod.rs). Ce fichier verifie ce mecanisme de
// bout en bout via une VRAIE interaction UI, en relisant directement le
// fichier YAML sur disque plutot que l'API /api/services (qui ne lit que le
// snapshot en memoire et ne prouverait donc rien sur la persistance reelle).
// Une relecture du fichier reflete exactement ce qu'un load_or_init() reel
// lirait au redemarrage du pod — "simuler un redemarrage" sans avoir a tuer
// le process serveur utilise par toute la suite e2e.
//
// Hypothese (partagee avec le reste de la suite e2e qui cible deja
// localhost:7342) : le serveur tourne avec la convention documentee dans
// README.md (DATA_PATH=./data depuis la racine du repo). Surchargeable via
// la variable d'env DATA_PATH si le serveur e2e est lance autrement.
const dataDir = process.env.DATA_PATH || resolve(process.cwd(), '../data');
const CONFIG_FILE = resolve(dataDir, 'mock-config.yaml');

function readConfigFromDisk() {
  return readFileSync(CONFIG_FILE, 'utf-8');
}

function validService(name, overrides = {}) {
  return {
    name,
    listen_path: '/v1/*',
    real_target_url: 'http://backend:8080',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [],
    ...overrides,
  };
}

async function openFirstGroup(page) {
  const group = page.locator('button[aria-expanded]').first();
  if (await group.getAttribute('aria-expanded') === 'false') {
    await group.click();
    await page.waitForTimeout(200);
  }
}

test.describe('Write-behind: persistence after a simulated store restart', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('a service mutation made through the UI survives a re-read of the on-disk config', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('write-behind-svc') });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await openFirstGroup(page);

    const toggle = page.getByRole('switch', { name: 'Mock write-behind-svc' });
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');

    // L'ecriture disque est asynchrone (write-behind) : on attend que le
    // contenu apparaisse reellement sur le fichier plutot que de supposer
    // un delai fixe (le flush n'est pas expose cote HTTP, volontairement —
    // ce n'est pas un mecanisme que les handlers appellent).
    await expect(async () => {
      const yaml = readConfigFromDisk();
      expect(yaml).toContain('name: write-behind-svc');
      expect(yaml).toMatch(/name: write-behind-svc\n(?:.*\n)*?\s*is_mocked: false/);
    }).toPass({ timeout: 5000 });
  });

  test('a service deleted through the API disappears from the on-disk config once persisted', async ({ request }) => {
    await request.post(`${API}/services`, { data: validService('write-behind-delete-svc') });

    await expect(async () => {
      expect(readConfigFromDisk()).toContain('write-behind-delete-svc');
    }).toPass({ timeout: 5000 });

    const del = await request.delete(`${API}/services/write-behind-delete-svc`);
    expect(del.status()).toBe(204);

    await expect(async () => {
      expect(readConfigFromDisk()).not.toContain('write-behind-delete-svc');
    }).toPass({ timeout: 5000 });
  });
});
