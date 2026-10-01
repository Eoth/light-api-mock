#!/usr/bin/env node
// Fails when a tracked file, or its path, names the design system the UI used to borrow its tokens from. That design
// system belongs to someone else and nothing of it stays here, not even its name.
//
// The names are not written in this repository: the script compares the SHA-256 digest of every word to DIGESTS.
// A word is a run of letters or digits, lowercased, so "--x-color", "x_tokens", "X" and "xTheme" all yield "x".
// Files Git would show as binary (a NUL byte in their first 8000 bytes) are skipped: images and fonts hold no prose.
// Usage: node scripts/check-former-design-system.mjs [repository root]
//        node scripts/check-former-design-system.mjs --stdin   (checks the text read from standard input)
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const DIGESTS = new Set([
  '756f29d770e8563f9bfc2c1dfb683123134b8076fa7ffe673c45a0ca61b39c49',
  '9b89025ce7a6d932b28f6e15132a70d402f723874a425e9b4c7cc3b179fa66ce',
  'a925617886e8799cc5be05aee17f9b70fa3e91370a471e06c21277c10804d0dc',
]);

// Lower-to-upper case and letter-to-digit changes split a word too: "fooBar" and "foo2" hold "foo".
const WORD = /\p{Lu}?\p{Ll}+|\p{Lu}+(?!\p{Ll})|\p{L}+|\p{N}+/gu;

export function words(text) {
  return (text.match(WORD) ?? []).map((word) => word.toLowerCase());
}

const digests = new Map();
function isListed(word) {
  let listed = digests.get(word);
  if (listed === undefined) {
    listed = DIGESTS.has(createHash('sha256').update(word).digest('hex'));
    digests.set(word, listed);
  }
  return listed;
}

/** Returns "line:column" of each listed word in `text`, without the word itself. */
export function findNames(text) {
  const found = [];
  text.split('\n').forEach((line, index) => {
    for (const match of line.matchAll(WORD)) {
      if (isListed(match[0].toLowerCase())) found.push(`${index + 1}:${match.index + 1}`);
    }
  });
  return found;
}

const isBinary = (bytes) => bytes.subarray(0, 8000).includes(0);

/**
 * Returns the places where `files` (repository-relative paths) name the former design system, as "file" for a path
 * and "file:line:column" for a content; `read(file)` returns the bytes of a file.
 */
export function checkFiles(files, read) {
  const problems = [];
  for (const file of files) {
    if (findNames(file).length) problems.push(`${file} (in its path)`);
    const bytes = read(file);
    if (isBinary(bytes)) continue;
    for (const place of findNames(bytes.toString('utf8'))) problems.push(`${file}:${place}`);
  }
  return problems;
}

function main() {
  if (process.argv[2] === '--stdin') {
    const found = findNames(readFileSync(0, 'utf8'));
    if (found.length) {
      console.error(`The text names the former design system at ${found.join(', ')} (line:column).`);
      process.exit(1);
    }
    return;
  }
  const root = path.resolve(process.argv[2] ?? path.join(path.dirname(fileURLToPath(import.meta.url)), '..'));
  // The files Git tracks, so that a local run checks exactly what CI checks.
  const files = execFileSync('git', ['ls-files', '-z'], { cwd: root, encoding: 'utf8' }).split('\0').filter(Boolean);
  const problems = checkFiles(files, (file) => readFileSync(path.join(root, file)));
  if (problems.length) {
    console.error(problems.join('\n'));
    console.error(
      `\n${problems.length} place(s) name the former design system: remove the name, or write "the former design system".`,
    );
    process.exit(1);
  }
  console.log(`None of the ${files.length} tracked files names the former design system.`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
