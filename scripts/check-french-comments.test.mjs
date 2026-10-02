// Tests of check-french-comments.mjs: French comments are reported in every syntax, French strings never are, and the
// command fails on a French comment in each covered path. Run with: node --test scripts/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { COVERED, checkComments, commentLines, isFrench } from './check-french-comments.mjs';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'check-french-comments.mjs');

function check(files) {
  return checkComments(Object.keys(files), (file) => files[file]);
}

test('English comments pass, a French line comment is reported with its line', () => {
  const js = '// Keeps the order of the rules.\nconst a = 1;\n// Garde les regles dans leur ordre.\n';
  assert.deepEqual(check({ 'a.js': '// Keeps the order of the rules.\n' }), []);
  assert.deepEqual(check({ 'a.js': js }), ['a.js:3: // Garde les regles dans leur ordre.']);
});

test('short French comments are recognised, with or without accents, by their words or their elisions', () => {
  const french = [
    "// Réinitialise l'état.",
    '// Valeur par defaut',
    '// Voir plus haut',
    '// Le serveur ne répond pas',
    "// qu'il soit vide",
  ];
  const js = french.join('\nx();\n');
  assert.deepEqual(
    check({ 'a.js': js }).map((p) => p.split(': ')[0]),
    ['a.js:1', 'a.js:3', 'a.js:5', 'a.js:7', 'a.js:9'],
  );
});

test('terse French, as test titles write it without articles, is recognised', () => {
  // Titles the end-to-end suite once had, which the articles and pronouns alone did not reveal.
  const titles = [
    'groupe: nom accentue accepte',
    'UI servie sans aucun service',
    'Service purement mocke : comportement reseau',
    'service: toggle mock/proxy fonctionne',
    'identite: suppression sans fausse erreur',
    'Runner (scenarios JSON) - lot 7 (pliage JSON + options avancees)',
    'Runner (scenarios JSON) - lot 10 (XML par exemple)',
    'Runner (scenarios JSON) - lot 16 (diagnostic reponse JSON/XML)',
  ];
  assert.deepEqual(titles.filter((title) => !isFrench(title)), []);
});

test('English that shares letters with French words is not reported', () => {
  const english = [
    '// De-duplicate the en-AU and fr-CA entries.',
    "// It's the user's choice: don't retry, and don't tout it.",
    '// The `de` and `la` fields, `si` and `ou` flags.',
    '// A CAS loop, times in EST, on par with sans-serif fonts, an encore.',
  ].join('\n');
  assert.deepEqual(check({ 'a.js': english }), []);
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

// A file of comment lines, written in the comment syntax of `file`: in Svelte markup and in CSS, "//" is not a comment.
function commented(file, ...lines) {
  const extension = path.posix.extname(file);
  const [open, close] = { '.svelte': ['<!-- ', ' -->'], '.css': ['/* ', ' */'] }[extension] ?? ['// ', ''];
  return lines.map((line) => `${open}${line}${close}\n`).join('');
}

test('the command fails on a French comment in every covered path, and ignores the others', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'french-comments-'));
  try {
    const files = Object.fromEntries(
      COVERED.map((pattern) => [sampleOf(pattern), commented(sampleOf(pattern), 'The first line is fine.')]),
    );
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
      writeFileSync(path.join(root, file), commented(file, 'The first line is fine.', 'Mais pas la seconde.'));
      const broken = spawnSync(process.execPath, [SCRIPT, root], { encoding: 'utf8' });
      assert.equal(broken.status, 1, `${pattern}: ${broken.stdout}`);
      assert.match(broken.stderr, new RegExp(`^${file.replaceAll('.', '\\.')}:2: .*Mais pas la seconde\\.`, 'm'));
      writeFileSync(path.join(root, file), commented(file, 'The first line is fine.'));
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
