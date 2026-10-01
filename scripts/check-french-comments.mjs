#!/usr/bin/env node
// Fails when a comment of a covered file is written in French. Comments are in English, the language every reviewer
// and contributor shares; French belongs in the translation catalogues. Strings are not comments: a sentence of the
// interface, a test fixture or example data is never reported, whatever its language.
//
// Each file is scanned with the comment and string syntax of its language (Rust, JavaScript, Svelte, CSS), so that a
// "//" inside a URL, a regular expression or a string is not taken for a comment. A comment line is French when it
// holds one of FRENCH_WORDS or an elision, outside code quoted with backticks.
// Usage: node scripts/check-french-comments.mjs [repository root] (defaults to this script's repository).
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Git pathspecs of the files whose comments must stay in English ("*" stays within a directory, "**" crosses them).
// A path joins the list once its comments are translated, so that it cannot slip back.
export const COVERED = [
  'src/**/*.rs',
  'tests/**/*.rs',
  'build.rs',
  'scripts/*.mjs',
  'frontend/src/main.js',
  'frontend/src/App.svelte',
  'frontend/src/lib/*.js',
  'frontend/src/lib/components/ConditionForm.svelte',
  'frontend/src/lib/components/JsonPasteBuilder.svelte',
  'frontend/src/lib/components/JsonResponseBuilder.svelte',
  'frontend/src/lib/components/RhaiScriptEditor.svelte',
  'frontend/src/lib/components/RuleActionSelector.svelte',
  'frontend/src/lib/components/RuleConditionsEditor.svelte',
  'frontend/src/lib/components/RuleForm.svelte',
  'frontend/src/lib/components/RuleList.svelte',
  'frontend/src/lib/components/RuleResponseSection.svelte',
  'frontend/src/lib/components/RuleScriptSlot.svelte',
  'frontend/src/lib/components/RuleTester.svelte',
  'frontend/src/lib/components/RuleWarnings.svelte',
  'frontend/src/lib/components/Sentence.svelte',
  'frontend/src/lib/components/XmlPasteBuilder.svelte',
  'frontend/src/lib/components/XmlResponseBuilder.svelte',
];

// Frequent French words that English comments do not use, with and without their accents (comments are often typed
// without them). Words that English shares are left out: "en" (a language code), "est" (a time zone), "par", "cas"
// (compare-and-swap), "aux", "sans", "tout", "encore".
export const FRENCH_WORDS = [
  'les', 'pour', 'avec', 'une', 'sont', 'dans', 'qui', 'deja', 'déjà', 'regle', 'règle', 'requete', 'requête', 'meme',
  'même', 'donc', 'sinon', 'aussi', 'mais', 'etre', 'être', 'cette', 'cela', 'lorsque', 'puis', 'chaque', 'des', 'du',
  'de', 'le', 'la', 'un', 'et', 'ou', 'au', 'ce', 'ces', 'il', 'ne', 'pas', 'si', 'sur', 'à', 'où', 'quand', 'comme',
  'doit', 'peut', 'fait', 'tous', 'toujours', 'jamais', 'rien', 'avant', 'apres', 'après', 'selon', 'entre', 'ici',
  'voir', 'cote', 'côté', 'plutot', 'plutôt', 'seul', 'seule', 'deux', 'reste', 'etat', 'état', 'defaut', 'défaut',
  'parce',
];

// A letter, a digit, "_" or "-" next to a listed word makes it part of another word ("de-duplicate", "en-AU").
const WORD_CHAR = String.raw`[\p{L}\p{N}_-]`;
// An elided article or pronoun: `l'état`, `d'un`, `qu'il`, `n'est`.
const ELISION = String.raw`(?:[cdjlmnst]|qu)['’]\p{L}`;
const FRENCH = new RegExp(
  String.raw`(?<!${WORD_CHAR})(?:(?:${FRENCH_WORDS.join('|')})(?!${WORD_CHAR})|${ELISION})`,
  'iu',
);
// Code quoted in a comment (`de`, `la`) is not prose.
const QUOTED_CODE = /`[^`]*`/g;

const SYNTAX_BY_EXTENSION = { '.rs': 'rust', '.js': 'js', '.mjs': 'js', '.svelte': 'svelte', '.css': 'css' };

// Words after which a "/" starts a regular expression rather than a division.
const KEYWORDS_BEFORE_EXPRESSION = new Set([
  'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void', 'throw', 'case', 'do', 'else', 'yield', 'await',
]);

const WORD = /[\p{L}\p{N}_$]+/uy;
const RUST_RAW_STRING = /b?r(#*)"/y;
const RUST_CHAR = /'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'/uy;
const SVELTE_EMBEDDED = /<(script|style)\b[^>]*>/y;

function matchAt(pattern, text, index) {
  pattern.lastIndex = index;
  return pattern.exec(text);
}

// Collects comments as one { line, text } per line they span.
class Comments {
  constructor(text) {
    this.text = text;
    this.lines = [];
    this.lineStarts = [0];
    for (let i = 0; i < text.length; i++) if (text[i] === '\n') this.lineStarts.push(i + 1);
  }

  lineOf(index) {
    let low = 0;
    let high = this.lineStarts.length - 1;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if (this.lineStarts[middle] <= index) low = middle;
      else high = middle - 1;
    }
    return low + 1;
  }

  add(start, end) {
    const first = this.lineOf(start);
    this.text.slice(start, end).split('\n').forEach((text, offset) => {
      this.lines.push({ line: first + offset, text: text.replace(/\r$/, '') });
    });
  }
}

function lineEnd(text, index, limit) {
  const end = text.indexOf('\n', index);
  return end === -1 || end > limit ? limit : end;
}

function after(text, index, limit, closing) {
  const end = text.indexOf(closing, index);
  return end === -1 || end + closing.length > limit ? limit : end + closing.length;
}

// A quoted string of JavaScript or CSS, which cannot span lines unescaped.
function skipQuoted(text, index, limit) {
  const quote = text[index];
  for (let i = index + 1; i < limit; i++) {
    if (text[i] === '\\') i++;
    else if (text[i] === quote) return i + 1;
    else if (text[i] === '\n') return i;
  }
  return limit;
}

// The text of a template literal from `index`, up to its closing backtick or to its next `${`.
function scanTemplateText(text, index, limit) {
  for (let i = index; i < limit; i++) {
    if (text[i] === '\\') i++;
    else if (text[i] === '`') return { index: i + 1, interpolation: false };
    else if (text[i] === '$' && text[i + 1] === '{') return { index: i + 2, interpolation: true };
  }
  return { index: limit, interpolation: false };
}

// The end of a regular expression literal starting at `index`, or -1 when the "/" cannot start one.
function regexEnd(text, index, limit) {
  let inClass = false;
  for (let i = index + 1; i < limit; i++) {
    const c = text[i];
    if (c === '\n') return -1;
    if (c === '\\') i++;
    else if (c === '[') inClass = true;
    else if (c === ']') inClass = false;
    else if (c === '/' && !inClass) {
      let end = i + 1;
      while (end < limit && /[a-z]/.test(text[end])) end++;
      return end;
    }
  }
  return -1;
}

// Scans JavaScript from `index`. Inside a Svelte expression (`inBraces`), stops after the "}" that closes it and
// returns the index that follows; otherwise scans up to `limit`.
function scanJs(comments, index, limit, inBraces) {
  const { text } = comments;
  let depth = 0;
  // Brace depth at which each open template literal entered a `${`.
  const templates = [];
  let regexAllowed = true;
  let i = index;
  const enterTemplate = (from) => {
    const part = scanTemplateText(text, from, limit);
    i = part.index;
    if (part.interpolation) {
      templates.push(depth);
      depth++;
    }
    regexAllowed = part.interpolation;
  };
  while (i < limit) {
    const c = text[i];
    const next = text[i + 1];
    if (c === '/' && next === '/') {
      const end = lineEnd(text, i, limit);
      comments.add(i, end);
      i = end;
    } else if (c === '/' && next === '*') {
      const end = after(text, i + 2, limit, '*/');
      comments.add(i, end);
      i = end;
    } else if (c === '"' || c === "'") {
      i = skipQuoted(text, i, limit);
      regexAllowed = false;
    } else if (c === '`') {
      enterTemplate(i + 1);
    } else if (c === '/') {
      const end = regexAllowed ? regexEnd(text, i, limit) : -1;
      i = end === -1 ? i + 1 : end;
      regexAllowed = end === -1;
    } else if (c === '{') {
      depth++;
      i++;
      regexAllowed = true;
    } else if (c === '}') {
      if (templates.length && templates[templates.length - 1] === depth - 1) {
        templates.pop();
        depth--;
        enterTemplate(i + 1);
      } else if (depth === 0 && inBraces) {
        return i + 1;
      } else {
        depth--;
        i++;
        regexAllowed = true;
      }
    } else if (matchAt(WORD, text, i)) {
      regexAllowed = KEYWORDS_BEFORE_EXPRESSION.has(text.slice(i, WORD.lastIndex));
      i = WORD.lastIndex;
    } else {
      if (!/\s/.test(c)) regexAllowed = c !== ')' && c !== ']';
      i++;
    }
  }
  return limit;
}

function scanCss(comments, index, limit) {
  const { text } = comments;
  let i = index;
  while (i < limit) {
    if (text[i] === '/' && text[i + 1] === '*') {
      const end = after(text, i + 2, limit, '*/');
      comments.add(i, end);
      i = end;
    } else if (text[i] === '"' || text[i] === "'") {
      i = skipQuoted(text, i, limit);
    } else {
      i++;
    }
  }
}

// Markup: HTML comments, then JavaScript in <script> and in `{…}` expressions, CSS in <style>. Text and attribute
// values are not code: an apostrophe there opens no string.
function scanSvelte(comments) {
  const { text } = comments;
  let i = 0;
  while (i < text.length) {
    const embedded = text[i] === '<' && matchAt(SVELTE_EMBEDDED, text, i);
    if (text.startsWith('<!--', i)) {
      const end = after(text, i + 4, text.length, '-->');
      comments.add(i, end);
      i = end;
    } else if (embedded) {
      const bodyStart = SVELTE_EMBEDDED.lastIndex;
      const close = text.indexOf(`</${embedded[1]}`, bodyStart);
      const bodyEnd = close === -1 ? text.length : close;
      if (embedded[1] === 'script') scanJs(comments, bodyStart, bodyEnd, false);
      else scanCss(comments, bodyStart, bodyEnd);
      i = bodyEnd;
    } else if (text[i] === '{' && text[i + 1] === '/') {
      // The end of a block ({/if}, {/each}…) holds no expression.
      i = after(text, i, text.length, '}');
    } else if (text[i] === '{') {
      i = scanJs(comments, i + (/[#:@]/.test(text[i + 1]) ? 2 : 1), text.length, true);
    } else {
      i++;
    }
  }
}

function scanRust(comments) {
  const { text } = comments;
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    const next = text[i + 1];
    const raw = matchAt(RUST_RAW_STRING, text, i);
    if (c === '/' && next === '/') {
      const end = lineEnd(text, i, text.length);
      comments.add(i, end);
      i = end;
    } else if (raw) {
      i = after(text, RUST_RAW_STRING.lastIndex, text.length, `"${raw[1]}`);
    } else if (c === '/' && next === '*') {
      // Block comments nest in Rust.
      let depth = 1;
      let end = i + 2;
      while (end < text.length && depth > 0) {
        if (text.startsWith('/*', end)) {
          depth++;
          end += 2;
        } else if (text.startsWith('*/', end)) {
          depth--;
          end += 2;
        } else {
          end++;
        }
      }
      comments.add(i, end);
      i = end;
    } else if (c === '"') {
      i++;
      while (i < text.length && text[i] !== '"') i += text[i] === '\\' ? 2 : 1;
      i++;
    } else if (c === "'") {
      // A character literal, or else the quote of a lifetime ('a, 'static).
      i = matchAt(RUST_CHAR, text, i) ? RUST_CHAR.lastIndex : i + 1;
    } else if (matchAt(WORD, text, i)) {
      // Whole identifiers, so that the "r" or "br" of a raw string is only looked for where a token starts.
      i = WORD.lastIndex;
    } else {
      i++;
    }
  }
}

/** The comment lines of `text`, written in `syntax` ('rust', 'js', 'svelte' or 'css'), as { line, text }. */
export function commentLines(text, syntax) {
  const comments = new Comments(text);
  if (syntax === 'rust') scanRust(comments);
  else if (syntax === 'js') scanJs(comments, 0, text.length, false);
  else if (syntax === 'svelte') scanSvelte(comments);
  else if (syntax === 'css') scanCss(comments, 0, text.length);
  else throw new Error(`unknown syntax ${syntax}`);
  return comments.lines;
}

export function isFrench(commentText) {
  return FRENCH.test(commentText.replace(QUOTED_CODE, ''));
}

/**
 * Returns the French comment lines found in `files` (repository-relative paths with `/` separators), as
 * "file:line: comment"; `read(file)` returns the text of a file.
 */
export function checkComments(files, read) {
  const problems = [];
  for (const file of files) {
    const syntax = SYNTAX_BY_EXTENSION[path.posix.extname(file)];
    if (!syntax) {
      problems.push(`${file}: no comment syntax known for this kind of file`);
      continue;
    }
    for (const { line, text } of commentLines(read(file), syntax)) {
      if (isFrench(text)) problems.push(`${file}:${line}: ${text.trim()}`);
    }
  }
  return problems;
}

function main() {
  const root = path.resolve(process.argv[2] ?? path.join(path.dirname(fileURLToPath(import.meta.url)), '..'));
  // The files Git tracks, so that a local run checks exactly what CI checks.
  const pathspecs = COVERED.map((pattern) => `:(glob)${pattern}`);
  const files = execFileSync('git', ['ls-files', '--', ...pathspecs], { cwd: root, encoding: 'utf8' })
    .split('\n')
    .filter(Boolean);
  const problems = checkComments(files, (file) => readFileSync(path.join(root, file), 'utf8'));
  if (problems.length) {
    console.error(problems.join('\n'));
    console.error(`\n${problems.length} French comment line(s): comments are written in English.`);
    process.exit(1);
  }
  console.log(`The comments of ${files.length} covered files are in English.`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
