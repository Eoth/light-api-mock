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
    case 'xpath': varPart = `xpath.${f.value}`; break;
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

export function findPipeSeparator(expr) {
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
  if (varName.startsWith('xpath.')) return { source: 'xpath', value: varName.slice(6) };
  if (varName.startsWith('fake.')) return { source: 'fake', value: varName.slice(5) };
  if (varName === 'script') return { source: 'script', value: '' };
  if (varName.startsWith('script.')) return { source: 'script', value: varName.slice(7) };
  if (varName === 'uuid') return { source: 'uuid', value: '' };
  if (varName === 'now_ms') return { source: 'now_ms', value: '' };
  if (varName === 'now_iso') return { source: 'now_iso', value: '' };
  if (varName === 'seq') return { source: 'seq', value: '' };
  return { source: 'fixed', value: varName };
}

// ── Example JSON: raw value → Fields (mode "coller un exemple") ──────
// Contrairement a templateToFields (qui parse un template {{...}} deja
// existant), cette fonction part d'un exemple JSON brut (litteral, sans
// {{}}) tel que colle par l'utilisateur : chaque valeur devient un champ
// fieldType:'value', source:'fixed' pre-rempli avec la valeur collee, que
// l'utilisateur peut ensuite reassigner (path/query/fake/etc.) dans le
// builder guide.

export function exampleJsonToFields(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError('exampleJsonToFields attend un objet JSON en racine.');
  }
  return objectToFields(value);
}

function objectToFields(obj) {
  return Object.entries(obj).map(([key, value]) => {
    if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
      return { key, fieldType: 'object', children: objectToFields(value) };
    }
    if (Array.isArray(value)) {
      if (value.length > 0 && typeof value[0] === 'object' && value[0] !== null) {
        return { key, fieldType: 'array-objects', template: objectToFields(value[0]) };
      }
      return {
        key, fieldType: 'array-values',
        items: value.map(v => ({ source: 'fixed', value: String(v), pipe: '', asNumber: typeof v === 'number' })),
      };
    }
    return {
      key, fieldType: 'value', source: 'fixed',
      value: String(value ?? ''), pipe: '',
      asNumber: typeof value === 'number' || typeof value === 'boolean',
    };
  });
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
//
// `attributes` (optionnel) : liste de {name, source, value, pipe} portee
// par un noeud (racine incluse via `rootAttributes`) ou par un champ
// value/parent. Retro-compatible : un champ/racine sans `attributes`
// produit exactement le meme texte qu'avant, `xmlAttrsToTpl` renvoyant ''
// pour une liste vide/absente. Aucun echappement des valeurs d'attribut
// (guillemets compris) : coherent avec le choix deliberement fait pour le
// contenu texte des elements (resolve_variable ne re-echappe jamais) --
// le template reste du texte brut de bout en bout.

export function xmlFieldsToTemplate(fields, rootTag = 'response', rootAttributes = []) {
  const attrs = xmlAttrsToTpl(rootAttributes);
  const inner = fields.filter(f => f.tag?.trim()).map(f => xmlNodeToTpl(f)).join('');
  return `<${rootTag}${attrs}>${inner}</${rootTag}>`;
}

function xmlAttrsToTpl(attributes) {
  return (attributes || [])
    .filter(a => a.name?.trim())
    .map(a => ` ${a.name.trim()}="${buildExpr(a)}"`)
    .join('');
}

function xmlNodeToTpl(field) {
  const t = field.tag?.trim();
  if (!t) return '';
  const attrs = xmlAttrsToTpl(field.attributes);
  if ((field.nodeType || 'value') === 'parent') {
    const inner = (field.children || []).filter(c => c.tag?.trim()).map(c => xmlNodeToTpl(c)).join('');
    return `<${t}${attrs}>${inner}</${t}>`;
  }
  return `<${t}${attrs}>${buildExpr(field)}</${t}>`;
}

// ── Example XML: raw text → Fields (mode "coller un exemple", XML) ────
// Miroir de exampleJsonToFields (voir plus haut) pour le XML : part d'un
// exemple XML brut (litteral, sans {{}}) tel que colle par l'utilisateur
// (typiquement une reponse SOAP reelle), et retourne { rootTag,
// rootAttributes, fields } ou `fields` est la liste des ELEMENTS ENFANTS
// DIRECTS de la racine, au meme format que celui consomme par
// XmlResponseBuilder.svelte (tag/nodeType/source/value/children), etendu
// avec `attributes`.
//
// Limites assumees et documentees (pas des bugs a corriger silencieusement,
// meme esprit que la limite deja assumee pour parse_xml_items cote Rhai) :
//  - Prefixes de namespace ("soap:Envelope") et declarations xmlns/xmlns:*
//    sont preserves TELS QUELS comme du texte litteral dans le nom de
//    tag/attribut (DOMParser les restitue deja ainsi via .tagName/.name) --
//    aucune resolution semantique (pas de mapping prefixe -> URI). Suffisant
//    pour reconstruire un template fidele au XML colle sans jamais planter
//    dessus ; une validation stricte des namespaces est hors scope.
//  - Contenu mixte (texte + elements enfants sur le meme noeud) : les
//    elements enfants gagnent (noeud traite comme 'parent'), le texte
//    direct du noeud est ignore -- cas rare pour des reponses API/SOAP
//    typiques (texte pur en feuille XOR structure imbriquee).
//  - Une racine sans aucun element enfant (uniquement du texte) est
//    rejetee avec un message explicite, comme un tableau JSON vide cote
//    exampleJsonToFields.

export function exampleXmlToFields(xmlString) {
  const text = xmlString.trim();
  if (!text) {
    throw new TypeError('Collez un XML valide.');
  }
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, 'application/xml');
  if (doc.querySelector('parsererror')) {
    throw new TypeError('XML invalide : verifiez les tags (noms vides, imbrication incorrecte).');
  }
  const root = doc.documentElement;
  if (!root) {
    throw new TypeError('XML invalide : aucun element racine trouve.');
  }
  const childElements = Array.from(root.children || []);
  if (childElements.length === 0) {
    throw new TypeError('La racine XML ne contient aucun element imbrique. Collez un XML avec au moins un sous-element.');
  }
  return {
    rootTag: root.tagName,
    rootAttributes: xmlAttributesToFields(root),
    fields: childElements.map(xmlElementToField),
  };
}

// ── XML template string → Fields (restauration de la vue d'origine) ──
// Miroir XML de templateToFields() (JSON) : contrairement a
// exampleXmlToFields (qui part d'un exemple XML LITTERAL, sans {{}}, colle
// par l'utilisateur), cette fonction part d'un TEMPLATE deja rendu par
// xmlFieldsToTemplate() (avec {{expr | pipe}} deja en place) et reconstruit
// la structure Fields (tag/nodeType/source/value/pipe/children/attributes)
// consommee aussi bien par XmlResponseBuilder.svelte (guide) que
// XmlPasteBuilder.svelte (par exemple) — ces deux modes partagent la meme
// forme de Fields. Utilise DOMParser comme exampleXmlToFields (les
// caracteres {, }, |, ( ) sont du texte XML litteral valide, aucun
// echappement necessaire) ; seule la lecture de la feuille differe : on y
// detecte un eventuel {{expr | pipe}} au lieu de toujours traiter comme une
// valeur fixe.
export function templateToXmlFields(tpl) {
  const text = tpl.trim();
  if (!text) {
    throw new TypeError('Template XML vide.');
  }
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, 'application/xml');
  if (doc.querySelector('parsererror')) {
    throw new TypeError('Template XML invalide : impossible de le reanalyser en vue structuree.');
  }
  const root = doc.documentElement;
  if (!root) {
    throw new TypeError('Template XML invalide : aucun element racine trouve.');
  }
  return {
    rootTag: root.tagName,
    rootAttributes: xmlAttributesToTplFields(root),
    fields: Array.from(root.children || []).map(xmlElementToTplField),
  };
}

function xmlAttributesToTplFields(el) {
  return Array.from(el.attributes || []).map(attr => ({
    name: attr.name, ...parseXmlLeafExpr(attr.value),
  }));
}

function xmlElementToTplField(el) {
  const tag = el.tagName;
  const attributes = xmlAttributesToTplFields(el);
  const childElements = Array.from(el.children || []);
  if (childElements.length > 0) {
    return { tag, nodeType: 'parent', attributes, children: childElements.map(xmlElementToTplField) };
  }
  return { tag, nodeType: 'value', attributes, ...parseXmlLeafExpr(el.textContent ?? '') };
}

// Lit le contenu d'une feuille XML (texte d'element ou valeur d'attribut) :
// si c'est EXACTEMENT une expression {{expr | pipe}}, la decompose comme
// parseTplValue() le fait pour JSON (reutilise varNameToSource +
// findPipeSeparator) ; sinon, valeur fixe litterale. Pas de notion
// d'asNumber/guillemets ici (contrairement a JSON) : le texte XML n'a pas
// cette distinction.
function parseXmlLeafExpr(raw) {
  const trimmed = (raw ?? '').trim();
  const varMatch = trimmed.match(/^\{\{([^}].*?)\}\}$/);
  if (varMatch) {
    const expr = varMatch[1];
    const pipeIdx = findPipeSeparator(expr);
    let varName, pipe;
    if (pipeIdx >= 0) {
      varName = expr.slice(0, pipeIdx).trim();
      pipe = expr.slice(pipeIdx + 1).trim();
    } else {
      varName = expr.trim();
      pipe = '';
    }
    const { source, value } = varNameToSource(varName);
    return { source, value, pipe };
  }
  return { source: 'fixed', value: trimmed, pipe: '' };
}

function xmlAttributesToFields(el) {
  return Array.from(el.attributes || []).map(attr => ({
    name: attr.name, source: 'fixed', value: attr.value, pipe: '',
  }));
}

function xmlElementToField(el) {
  const tag = el.tagName;
  const attributes = xmlAttributesToFields(el);
  const childElements = Array.from(el.children || []);
  if (childElements.length > 0) {
    return { tag, nodeType: 'parent', attributes, children: childElements.map(xmlElementToField) };
  }
  return { tag, nodeType: 'value', source: 'fixed', value: el.textContent ?? '', pipe: '', attributes };
}
