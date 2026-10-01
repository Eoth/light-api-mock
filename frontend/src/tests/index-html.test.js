import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const frontend = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

// Files that index.html loads from the root are copied from public/ by the build (the entry script under /src/ is
// bundled instead). One that does not exist there makes every page load end with a 404.
describe('index.html', () => {
  it('only refers to public files that exist', () => {
    const html = readFileSync(path.join(frontend, 'index.html'), 'utf8');
    const rooted = [...html.matchAll(/(?:href|src)="\/([^"]+)"/g)]
      .map((match) => match[1])
      .filter((file) => !file.startsWith('src/'));
    expect(rooted.length).toBeGreaterThan(0);
    for (const file of rooted) {
      expect(existsSync(path.join(frontend, 'public', file)), `public/${file}`).toBe(true);
    }
  });
});
