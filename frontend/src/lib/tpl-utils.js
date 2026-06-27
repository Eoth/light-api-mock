/**
 * Template utilities for lightMock's template format.
 *
 * Template format (consumed by Rust backend):
 *   { and } = literal braces (normal JSON/XML)
 *   {{expr}} = template variable (evaluated at runtime)
 *   {{expr | pipe}} = variable with transformation
 *
 * This module is the SINGLE source of truth for parsing and validating
 * this format on the frontend side.
 */

// ── Serialization: Fields → Template string ──────────────────────────

export function fieldsToTemplate(fields) {
  if (fields.length === 0) return '{}';
  const parts = fields.filter(f => f.key?.trim()).map(f => fieldToTpl(f));
  return `{${parts.join(',')}}`;
}

function fieldToTpl(field) {
  const k = field.key?.trim();
  if (!k) return '';
  const ft = field.fieldType || 'value';

  if (ft === 'value') {
    const expr = buildExpr(field);
    return field.asNumber ? `"${k}":${expr}` : `"${k}":"${expr}"`;
  }
  if (ft === 'object') {
    const inner = (field.children || []).filter(c => c.key?.trim()).map(c => fieldToTpl(c)).join(',');
    return `"${k}":{${inner}}`;
  }
  if (ft === 'array-values') {
    const items = (field.items || []).map(item => {
      const expr = buildExpr(item);
      return item.asNumber ? expr : `"${expr}"`;
    }).join(',');
    return `"${k}":[${items}]`;
  }
  if (ft === 'array-objects') {
    const inner = (field.template || []).filter(c => c.key?.trim()).map(c => fieldToTpl(c)).join(',');
    return `"${k}":[{${inner}}]`;
  }
  return '';
}

export function buildExpr(f) {
  if (f.source === 'fixed') return f.value ?? '';
  let varPart;
  switch (f.source) {
    case 'path': varPart = `path.${f.value}`; break;
    case 'query': varPart = `query.${f.value}`; break;
    case 'header': varPart = `header.${f.value}`; break;
    case 'body': varPart = `body.${f.value}`; break;
    case 'fake': varPart = `fake.${f.value}`; break;
    case 'script': varPart = f.value ? `script.${f.value}` : 'script'; break;
    case 'uuid': varPart = 'uuid'; break;
    case 'now_ms': varPart = 'now_ms'; break;
    case 'now_iso': varPart = 'now_iso'; break;
    case 'seq': varPart = 'seq'; break;
    default: return f.value ?? '';
  }
  const pipe = f.pipe?.trim();
  return pipe ? `{{${varPart} | ${pipe}}}` : `{{${varPart}}}`;
}

// ── Validation: Template string → test JSON ──────────────────────────

export function templateToTestJson(tpl) {
  let out = '';
  let i = 0;
  let inString = false;
  while (i < tpl.length) {
    if (tpl[i] === '\\' && inString) {
      out += tpl[i] + tpl[i + 1]; i += 2; continue;
    }
    if (tpl[i] === '{' && i + 1 < tpl.length && tpl[i + 1] === '{') {
      const end = findDoubleClose(tpl, i + 2);
      if (end !== -1) {
        out += inString ? '__var__' : '"__var__"';
        i = end + 2;
        continue;
      }
    }
    if (tpl[i] === '"') inString = !inString;
    out += tpl[i]; i++;
  }
  return out;
}

function findDoubleClose(str, start) {
  let inQuotes = false;
  let parenDepth = 0;
  for (let i = start; i < str.length; i++) {
    if (str[i] === '"' && parenDepth === 0) inQuotes = !inQuotes;
    if (str[i] === '(' && !inQuotes) parenDepth++;
    if (str[i] === ')' && !inQuotes) parenDepth = Math.max(0, parenDepth - 1);
    if (!inQuotes && parenDepth === 0 && str[i] === '}' && i + 1 < str.length && str[i + 1] === '}') return i;
  }
  return -1;
}

export function validateTemplateAsJson(tpl) {
  if (!tpl.trim()) return null;
  const testStr = templateToTestJson(tpl);
  try {
    JSON.parse(testStr);
    return null;
  } catch (e) {
    return `JSON invalide : ${e.message}`;
  }
}

export function validateTemplateAsXml(tpl) {
  if (!tpl.trim()) return null;
  const testXml = stripTemplateVars(tpl);
  try {
    const parser = new DOMParser();
    const doc = parser.parseFromString(testXml, 'application/xml');
    if (doc.querySelector('parsererror')) {
      return 'XML invalide : verifiez les tags (noms vides, imbrication incorrecte).';
    }
    return null;
  } catch {
    return 'XML malformed.';
  }
}

function stripTemplateVars(tpl) {
  let out = '';
  let i = 0;
  while (i < tpl.length) {
    if (tpl[i] === '{' && i + 1 < tpl.length && tpl[i + 1] === '{') {
      const end = findDoubleClose(tpl, i + 2);
      if (end !== -1) { out += 'x'; i = end + 2; continue; }
    }
    out += tpl[i]; i++;
  }
  return out;
}

// ── Preview: Template string → human-readable ────────────────────────

export function templateToPreview(tpl) {
  let out = '';
  let i = 0;
  while (i < tpl.length) {
    if (tpl[i] === '{' && i + 1 < tpl.length && tpl[i + 1] === '{') {
      const end = findDoubleClose(tpl, i + 2);
      if (end !== -1) {
        out += `«${tpl.slice(i, end + 2)}»`;
        i = end + 2;
        continue;
      }
    }
    out += tpl[i]; i++;
  }
  return out;
}

// ── Deserialization: Template string → Fields ────────────────────────

export function templateToFields(tpl) {
  const trimmed = tpl.trim();
  if (!trimmed || trimmed === '{}') return [];
  const testStr = templateToTestJson(trimmed);
  const parsed = JSON.parse(testStr);
  if (typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new TypeError('Le JSON doit etre un objet pour etre converti en vue guidee.');
  }
  return parseTplObject(trimmed);
}

function parseTplObject(tpl) {
  const entries = extractTplEntries(tpl);
  return entries.map(([key, rawValue]) => {
    const trimmed = rawValue.trim();
    if (trimmed.startsWith('{') && !isVarOpen(trimmed, 0)) {
      try {
        return { key, fieldType: 'object', children: parseTplObject(trimmed) };
      } catch { /* fall through */ }
    }
    if (trimmed.startsWith('[')) {
      return parseTplArray(key, trimmed);
    }
    return parseTplValue(key, trimmed);
  });
}

function isVarOpen(str, i) {
  return str[i] === '{' && i + 1 < str.length && str[i + 1] === '{';
}

function parseTplValue(key, raw) {
  const trimmed = raw.trim();
  const isQuoted = trimmed.startsWith('"') && trimmed.endsWith('"');
  const inner = isQuoted ? trimmed.slice(1, -1) : trimmed;

  const varMatch = inner.match(/^\{\{([^}].*?)\}\}$/);
  if (varMatch) {
    const expr = varMatch[1];
    const pipeIdx = findPipeSeparator(expr);
    let varName, pipes;
    if (pipeIdx >= 0) {
      varName = expr.slice(0, pipeIdx).trim();
      pipes = expr.slice(pipeIdx + 1).trim();
    } else {
      varName = expr.trim();
      pipes = '';
    }
    const { source, value } = varNameToSource(varName);
    return { key, fieldType: 'value', source, value, pipe: pipes, asNumber: !isQuoted };
  }
  return { key, fieldType: 'value', source: 'fixed', value: inner, pipe: '', asNumber: !isQuoted };
}

function findPipeSeparator(expr) {
  let depth = 0;
  for (let i = 0; i < expr.length; i++) {
    if (expr[i] === '(') depth++;
    if (expr[i] === ')') depth--;
    if (expr[i] === '|' && depth === 0) return i;
  }
  return -1;
}

function parseTplArray(key, raw) {
  const inner = raw.trim().slice(1, -1).trim();
  if (!inner) return { key, fieldType: 'array-values', items: [] };
  if (inner.startsWith('{') && !isVarOpen(inner, 0)) {
    try {
      const template = parseTplObject(inner);
      return { key, fieldType: 'array-objects', template };
    } catch { /* fall through */ }
  }
  const items = splitTplArray(inner).map(item => {
    const f = parseTplValue('', item.trim());
    return { source: f.source, value: f.value, pipe: f.pipe || '', asNumber: f.asNumber };
  });
  return { key, fieldType: 'array-values', items };
}

export function varNameToSource(varName) {
  if (varName.startsWith('path.')) return { source: 'path', value: varName.slice(5) };
  if (varName.startsWith('query.')) return { source: 'query', value: varName.slice(6) };
  if (varName.startsWith('header.')) return { source: 'header', value: varName.slice(7) };
  if (varName.startsWith('body.')) return { source: 'body', value: varName.slice(5) };
  if (varName.startsWith('fake.')) return { source: 'fake', value: varName.slice(5) };
  if (varName === 'script') return { source: 'script', value: '' };
  if (varName.startsWith('script.')) return { source: 'script', value: varName.slice(7) };
  if (varName === 'uuid') return { source: 'uuid', value: '' };
  if (varName === 'now_ms') return { source: 'now_ms', value: '' };
  if (varName === 'now_iso') return { source: 'now_iso', value: '' };
  if (varName === 'seq') return { source: 'seq', value: '' };
  return { source: 'fixed', value: varName };
}

// ── Low-level JSON-aware parser ─────────────────────────────────────

function extractTplEntries(objStr) {
  const trimmed = objStr.trim();
  let inner = trimmed;
  if (trimmed.startsWith('{') && !isVarOpen(trimmed, 0)) {
    inner = trimmed.slice(1, -1);
  }

  const entries = [];
  let i = 0;
  while (i < inner.length) {
    while (i < inner.length && /[\s,]/.test(inner[i])) i++;
    if (i >= inner.length || inner[i] !== '"') break;
    const keyEnd = inner.indexOf('"', i + 1);
    if (keyEnd === -1) break;
    const key = inner.slice(i + 1, keyEnd);
    i = keyEnd + 1;
    while (i < inner.length && /[\s:]/.test(inner[i])) i++;
    const [value, consumed] = readTplToken(inner, i);
    entries.push([key, value]);
    i += consumed;
  }
  return entries;
}

function readTplToken(str, start) {
  let i = start;
  if (i >= str.length) return ['', 0];

  if (str[i] === '"') {
    let j = i + 1;
    while (j < str.length) {
      if (str[j] === '\\') { j += 2; continue; }
      if (str[j] === '"') return [str.slice(i, j + 1), j + 1 - i];
      j++;
    }
    return [str.slice(i), str.length - i];
  }

  if (isVarOpen(str, i)) {
    const end = findDoubleClose(str, i + 2);
    if (end !== -1) return [str.slice(i, end + 2), end + 2 - i];
  }

  if (str[i] === '{') {
    let depth = 0, j = i, inStr = false;
    while (j < str.length) {
      if (str[j] === '\\' && inStr) { j += 2; continue; }
      if (str[j] === '"') { inStr = !inStr; j++; continue; }
      if (!inStr) {
        if (str[j] === '{' && j + 1 < str.length && str[j + 1] === '{') {
          const end = findDoubleClose(str, j + 2);
          if (end !== -1) { j = end + 2; continue; }
        }
        if (str[j] === '{') { depth++; j++; continue; }
        if (str[j] === '}') {
          depth--;
          if (depth === 0) return [str.slice(i, j + 1), j + 1 - i];
          j++; continue;
        }
      }
      j++;
    }
    return [str.slice(i), str.length - i];
  }

  if (str[i] === '[') {
    let depth = 0, j = i, inStr = false;
    while (j < str.length) {
      if (str[j] === '\\' && inStr) { j += 2; continue; }
      if (str[j] === '"') { inStr = !inStr; j++; continue; }
      if (!inStr) {
        if (str[j] === '[') { depth++; j++; continue; }
        if (str[j] === ']') {
          depth--;
          if (depth === 0) return [str.slice(i, j + 1), j + 1 - i];
          j++; continue;
        }
      }
      j++;
    }
    return [str.slice(i), str.length - i];
  }

  let j = i;
  while (j < str.length && str[j] !== ',' && str[j] !== '}' && str[j] !== ']') j++;
  return [str.slice(i, j).trim(), j - i];
}

function splitTplArray(inner) {
  const items = [];
  let i = 0, start = 0;
  while (i < inner.length) {
    if (inner[i] === '"') { const [, c] = readTplToken(inner, i); i += c; continue; }
    if (isVarOpen(inner, i)) { const end = findDoubleClose(inner, i + 2); if (end !== -1) { i = end + 2; continue; } }
    if (inner[i] === '{') { const [, c] = readTplToken(inner, i); i += c; continue; }
    if (inner[i] === '[') { const [, c] = readTplToken(inner, i); i += c; continue; }
    if (inner[i] === ',') { items.push(inner.slice(start, i)); start = i + 1; }
    i++;
  }
  if (start < inner.length) items.push(inner.slice(start));
  return items;
}

// ── XML: Fields → Template string ────────────────────────────────────

export function xmlFieldsToTemplate(fields, rootTag = 'response') {
  const inner = fields.filter(f => f.tag?.trim()).map(f => xmlNodeToTpl(f)).join('');
  return `<${rootTag}>${inner}</${rootTag}>`;
}

function xmlNodeToTpl(field) {
  const t = field.tag?.trim();
  if (!t) return '';
  if ((field.nodeType || 'value') === 'parent') {
    const inner = (field.children || []).filter(c => c.tag?.trim()).map(c => xmlNodeToTpl(c)).join('');
    return `<${t}>${inner}</${t}>`;
  }
  return `<${t}>${buildExpr(field)}</${t}>`;
}
