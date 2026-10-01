// Tests of check-french-comments.mjs: French comments are reported in every syntax, French strings never are, and the
// command fails on a French comment in each covered path. Run with: node --test scripts/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { COVERED, checkComments, commentLines } from './check-french-comments.mjs';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'check-french-comments.mjs');

function check(files) {
  return checkComments(Object.keys(files), (file) => files[file]);
}

test('English comments pass, a French line comment is reported with its line', () => {
  const js = '// Keeps the order of the rules.\nconst a = 1;\n// Garde les regles dans leur ordre.\n';
  assert.deepEqual(check({ 'a.js': '// Keeps the order of the rules.\n' }), []);
  assert.deepEqual(check({ 'a.js': js }), ['a.js:3: // Garde les regles dans leur ordre.']);
});

test('strings, templates, regular expressions and URLs are not comments', () => {
  const js = [
    't("Les règles sont appliquées dans cet ordre");',
    "const url = 'http://example.com/les/regles'; const b = \"// pour\";",
    'const tpl = `// une ligne ${t("dans une chaine")} // avec`;',
    'const re = /\\/\\/ avec|["\']/g; const half = total / 2 / 3; // two divisions',
    'const nested = `a ${`b // pour ${c}`} d`; // still code before this comment',
  ].join('\n');
  assert.deepEqual(check({ 'a.js': js }), []);
  assert.deepEqual(
    commentLines(js, 'js').map((c) => c.line),
    [4, 5],
  );
});

test('block comments report the line that holds French', () => {
  const js = '/**\n * Parses a template.\n * Retourne les champs du template.\n */\nexport function f() {}\n';
  assert.deepEqual(check({ 'a.js': js }), ['a.js:3: * Retourne les champs du template.']);
});

test('Svelte: HTML comments, script, expressions and style are read; markup text is not', () => {
  const svelte = [
    '<script>',
    "  const label = t(\"Les services\"); // the label's text",
    '</script>',
    '<!-- Liste des services, avec leur groupe -->',
    '<p title="L\'URL des services">Les services sont ici</p>',
    '<button onclick={() => {',
    '  // Ouvre le formulaire pour un nouveau service',
    '  open = true;',
    '}}>{t("Ajouter")}</button>',
    '{#if open}<p>{label}</p>{/if}',
    '<style>',
    '  .a { content: "/* pour */"; } /* Marge pour les cartes */',
    '</style>',
  ].join('\n');
  assert.deepEqual(check({ 'A.svelte': svelte }), [
    'A.svelte:4: <!-- Liste des services, avec leur groupe -->',
    'A.svelte:7: // Ouvre le formulaire pour un nouveau service',
    'A.svelte:12: /* Marge pour les cartes */',
  ]);
});

test('Rust: strings, raw strings, character literals and lifetimes do not hide or fake a comment', () => {
  const rust = [
    'fn f<\'a>(s: &\'a str) -> char { \'"\' } // returns a quote',
    'const A: &str = "// pour les tests";',
    'const B: &str = r#"une " chaine // avec"#; // English',
    "const C: u8 = b'\\''; // puis la suite",
    '/* outer /* inner */ toujours dans le commentaire, avec des mots */',
  ].join('\n');
  assert.deepEqual(check({ 'a.rs': rust }), [
    "a.rs:4: // puis la suite",
    'a.rs:5: /* outer /* inner */ toujours dans le commentaire, avec des mots */',
  ]);
});

test('a file in an unknown syntax is reported rather than skipped', () => {
  assert.deepEqual(check({ 'a.py': '# pour' }), ['a.py: no comment syntax known for this kind of file']);
});

// A file name that each covered pathspec matches.
function sampleOf(pattern) {
  return pattern.replace('**/', '').replace('*', 'sample');
}

test('the command fails on a French comment in every covered path, and ignores the others', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'french-comments-'));
  try {
    const files = Object.fromEntries(COVERED.map((pattern) => [sampleOf(pattern), '// The first line is fine.\n']));
    files['notes/sample.js'] = '// Pas encore traduit, pour plus tard.\n';
    for (const [file, text] of Object.entries(files)) {
      mkdirSync(path.join(root, path.dirname(file)), { recursive: true });
      writeFileSync(path.join(root, file), text);
    }
    execFileSync('git', ['init', '-q'], { cwd: root });
    execFileSync('git', ['add', '.'], { cwd: root });
    const clean = spawnSync(process.execPath, [SCRIPT, root], { encoding: 'utf8' });
    assert.equal(clean.status, 0, clean.stderr);

    for (const pattern of COVERED) {
      const file = sampleOf(pattern);
      writeFileSync(path.join(root, file), '// The first line is fine.\n// Mais pas la seconde.\n');
      const broken = spawnSync(process.execPath, [SCRIPT, root], { encoding: 'utf8' });
      assert.equal(broken.status, 1, `${pattern}: ${broken.stdout}`);
      assert.match(broken.stderr, new RegExp(`^${file.replaceAll('.', '\\.')}:2: // Mais pas la seconde\\.$`, 'm'));
      writeFileSync(path.join(root, file), '// The first line is fine.\n');
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
