import { describe, it, expect } from 'vitest';
import {
  fieldsToTemplate,
  templateToTestJson,
  templateToFields,
  validateTemplateAsJson,
  validateTemplateAsXml,
  templateToPreview,
  buildExpr,
  varNameToSource,
  xmlFieldsToTemplate,
} from '../lib/tpl-utils.js';

// ── fieldsToTemplate ─────────────────────────────────────────────────

describe('fieldsToTemplate', () => {
  it('empty fields produce empty object', () => {
    expect(fieldsToTemplate([])).toBe('{}');
  });

  it('single fixed string field', () => {
    const fields = [{ key: 'name', fieldType: 'value', source: 'fixed', value: 'Alice', pipe: '', asNumber: false }];
    expect(fieldsToTemplate(fields)).toBe('{"name":"Alice"}');
  });

  it('single numeric field', () => {
    const fields = [{ key: 'count', fieldType: 'value', source: 'fixed', value: '42', pipe: '', asNumber: true }];
    expect(fieldsToTemplate(fields)).toBe('{"count":42}');
  });

  it('variable with pipe', () => {
    const fields = [{ key: 'siren', fieldType: 'value', source: 'path', value: 'siret', pipe: 'first(9)', asNumber: false }];
    expect(fieldsToTemplate(fields)).toBe('{"siren":"{{path.siret | first(9)}}"}');
  });

  it('nested object', () => {
    const fields = [{
      key: 'data',
      fieldType: 'object',
      children: [{ key: 'id', fieldType: 'value', source: 'uuid', value: '', pipe: '', asNumber: false }],
    }];
    const tpl = fieldsToTemplate(fields);
    expect(tpl).toBe('{"data":{"id":"{{uuid}}"}}');
  });

  it('array of values', () => {
    const fields = [{
      key: 'tags',
      fieldType: 'array-values',
      items: [
        { source: 'fixed', value: 'a', pipe: '', asNumber: false },
        { source: 'fixed', value: 'b', pipe: '', asNumber: false },
      ],
    }];
    expect(fieldsToTemplate(fields)).toBe('{"tags":["a","b"]}');
  });

  it('array of objects', () => {
    const fields = [{
      key: 'items',
      fieldType: 'array-objects',
      template: [{ key: 'id', fieldType: 'value', source: 'seq', value: '', pipe: '', asNumber: true }],
    }];
    expect(fieldsToTemplate(fields)).toBe('{"items":[{"id":{{seq}}}]}');
  });
});

// ── templateToTestJson ───────────────────────────────────────────────

describe('templateToTestJson', () => {
  it('passes normal JSON through', () => {
    expect(templateToTestJson('{"a":"b"}')).toBe('{"a":"b"}');
  });

  it('replaces {{var}} with placeholder', () => {
    expect(templateToTestJson('{"k":"{{path.x}}"}')).toBe('{"k":"__var__"}');
  });

  it('handles pipe inside variable', () => {
    expect(templateToTestJson('{"k":"{{path.x | first(9)}}"}')).toBe('{"k":"__var__"}');
  });

  it('handles nested objects', () => {
    const tpl = '{"a":{"b":"{{uuid}}"}}';
    const test = templateToTestJson(tpl);
    expect(test).toBe('{"a":{"b":"__var__"}}');
    expect(() => JSON.parse(test)).not.toThrow();
  });

  it('handles numbers without quotes', () => {
    expect(templateToTestJson('{"n":{{seq}}}')).toBe('{"n":"__var__"}');
  });

  it('preserves arrays', () => {
    const tpl = '{"a":["x","{{uuid}}"]}';
    const test = templateToTestJson(tpl);
    expect(test).toBe('{"a":["x","__var__"]}');
    expect(() => JSON.parse(test)).not.toThrow();
  });
});

// ── validateTemplateAsJson ───────────────────────────────────────────

describe('validateTemplateAsJson', () => {
  it('accepts valid template', () => {
    expect(validateTemplateAsJson('{"name":"{{path.siret}}"}')).toBeNull();
  });

  it('accepts empty template', () => {
    expect(validateTemplateAsJson('')).toBeNull();
    expect(validateTemplateAsJson('  ')).toBeNull();
  });

  it('accepts nested objects with pipes', () => {
    expect(validateTemplateAsJson('{"data":{"siren":"{{path.siret | first(9)}}"}}')).toBeNull();
  });

  it('rejects malformed JSON', () => {
    const err = validateTemplateAsJson('{"name":}');
    expect(err).not.toBeNull();
    expect(err).toContain('JSON invalide');
  });

  it('rejects unbalanced braces', () => {
    const err = validateTemplateAsJson('{"name":"val"');
    expect(err).not.toBeNull();
  });

  it('accepts what fieldsToTemplate produces', () => {
    const fields = [
      { key: 'siret', fieldType: 'value', source: 'path', value: 'siret', pipe: '', asNumber: false },
      { key: 'siren', fieldType: 'value', source: 'path', value: 'siret', pipe: 'first(9)', asNumber: false },
      { key: 'data', fieldType: 'object', children: [
        { key: 'nom', fieldType: 'value', source: 'fake', value: 'CompanyName', pipe: '', asNumber: false },
        { key: 'ts', fieldType: 'value', source: 'now_ms', value: '', pipe: '', asNumber: true },
      ]},
    ];
    const tpl = fieldsToTemplate(fields);
    expect(validateTemplateAsJson(tpl)).toBeNull();
  });
});

// ── Round-trip: Fields → Template → Fields ───────────────────────────

describe('round-trip: fields -> template -> fields', () => {
  function roundTrip(fields) {
    const tpl = fieldsToTemplate(fields);
    const err = validateTemplateAsJson(tpl);
    expect(err).toBeNull();
    const back = templateToFields(tpl);
    return back;
  }

  it('simple fixed value', () => {
    const fields = [{ key: 'name', fieldType: 'value', source: 'fixed', value: 'Alice', pipe: '', asNumber: false }];
    const back = roundTrip(fields);
    expect(back).toHaveLength(1);
    expect(back[0].key).toBe('name');
    expect(back[0].source).toBe('fixed');
    expect(back[0].value).toBe('Alice');
  });

  it('variable with pipe', () => {
    const fields = [{ key: 'siren', fieldType: 'value', source: 'path', value: 'siret', pipe: 'first(9)', asNumber: false }];
    const back = roundTrip(fields);
    expect(back[0].source).toBe('path');
    expect(back[0].value).toBe('siret');
    expect(back[0].pipe).toBe('first(9)');
  });

  it('nested object', () => {
    const fields = [{
      key: 'data',
      fieldType: 'object',
      children: [
        { key: 'id', fieldType: 'value', source: 'uuid', value: '', pipe: '', asNumber: false },
        { key: 'name', fieldType: 'value', source: 'fake', value: 'FirstName', pipe: '', asNumber: false },
      ],
    }];
    const back = roundTrip(fields);
    expect(back[0].fieldType).toBe('object');
    expect(back[0].children).toHaveLength(2);
    expect(back[0].children[0].source).toBe('uuid');
    expect(back[0].children[1].source).toBe('fake');
    expect(back[0].children[1].value).toBe('FirstName');
  });

  it('multiple pipes chained', () => {
    const fields = [{ key: 'v', fieldType: 'value', source: 'path', value: 'name', pipe: 'lower | first(5)', asNumber: false }];
    const back = roundTrip(fields);
    expect(back[0].pipe).toBe('lower | first(5)');
  });

  it('numeric value without quotes', () => {
    const fields = [{ key: 'seq', fieldType: 'value', source: 'seq', value: '', pipe: '', asNumber: true }];
    const back = roundTrip(fields);
    expect(back[0].asNumber).toBe(true);
    expect(back[0].source).toBe('seq');
  });

  it('full INSEE-like template', () => {
    const fields = [
      { key: 'siret', fieldType: 'value', source: 'path', value: 'siret', pipe: '', asNumber: false },
      { key: 'siren', fieldType: 'value', source: 'path', value: 'siret', pipe: 'first(9)', asNumber: false },
      { key: 'unite_legale', fieldType: 'object', children: [
        { key: 'denomination', fieldType: 'value', source: 'fake', value: 'CompanyName', pipe: '', asNumber: false },
        { key: 'adresse', fieldType: 'object', children: [
          { key: 'ville', fieldType: 'value', source: 'fake', value: 'CityFR', pipe: '', asNumber: false },
        ]},
      ]},
      { key: 'meta', fieldType: 'object', children: [
        { key: 'timestamp', fieldType: 'value', source: 'now_ms', value: '', pipe: '', asNumber: true },
        { key: 'seq', fieldType: 'value', source: 'seq', value: '', pipe: '', asNumber: true },
      ]},
    ];
    const back = roundTrip(fields);
    expect(back).toHaveLength(4);
    expect(back[0].source).toBe('path');
    expect(back[1].pipe).toBe('first(9)');
    expect(back[2].fieldType).toBe('object');
    expect(back[2].children[1].fieldType).toBe('object');
    expect(back[3].children[0].asNumber).toBe(true);
  });
});

// ── templateToFields error cases ─────────────────────────────────────

describe('templateToFields error cases', () => {
  it('throws on array root', () => {
    expect(() => templateToFields('[1,2,3]')).toThrow();
  });

  it('throws on malformed JSON', () => {
    expect(() => templateToFields('{"a":}')).toThrow();
  });

  it('returns empty for empty object', () => {
    expect(templateToFields('{}')).toEqual([]);
  });
});

// ── templateToPreview ────────────────────────────────────────────────

describe('templateToPreview', () => {
  it('renders variables with angle brackets', () => {
    const result = templateToPreview('{"name":"{{path.x}}"}');
    expect(result).toBe('{"name":"«{{path.x}}»"}');
  });

  it('renders nested objects correctly', () => {
    const result = templateToPreview('{"a":{"b":"v"}}');
    expect(result).toBe('{"a":{"b":"v"}}');
  });
});

// ── validateTemplateAsXml ────────────────────────────────────────────

describe('validateTemplateAsXml', () => {
  it('accepts valid XML template', () => {
    expect(validateTemplateAsXml('<root><id>{{uuid}}</id></root>')).toBeNull();
  });

  it('rejects invalid XML', () => {
    const err = validateTemplateAsXml('<root><unclosed>');
    expect(err).not.toBeNull();
  });

  it('accepts empty', () => {
    expect(validateTemplateAsXml('')).toBeNull();
  });
});

// ── buildExpr ────────────────────────────────────────────────────────

describe('buildExpr', () => {
  it('fixed returns value directly', () => {
    expect(buildExpr({ source: 'fixed', value: 'hello' })).toBe('hello');
  });

  it('path without pipe', () => {
    expect(buildExpr({ source: 'path', value: 'id', pipe: '' })).toBe('{{path.id}}');
  });

  it('path with pipe', () => {
    expect(buildExpr({ source: 'path', value: 'siret', pipe: 'first(9)' })).toBe('{{path.siret | first(9)}}');
  });

  it('uuid', () => {
    expect(buildExpr({ source: 'uuid', value: '', pipe: '' })).toBe('{{uuid}}');
  });

  it('script', () => {
    expect(buildExpr({ source: 'script', value: '', pipe: '' })).toBe('{{script}}');
  });

  it('script with field', () => {
    expect(buildExpr({ source: 'script', value: 'result', pipe: '' })).toBe('{{script.result}}');
  });
});

// ── varNameToSource ──────────────────────────────────────────────────

describe('varNameToSource', () => {
  it('parses path', () => {
    expect(varNameToSource('path.siret')).toEqual({ source: 'path', value: 'siret' });
  });
  it('parses uuid', () => {
    expect(varNameToSource('uuid')).toEqual({ source: 'uuid', value: '' });
  });
  it('parses fake', () => {
    expect(varNameToSource('fake.Email')).toEqual({ source: 'fake', value: 'Email' });
  });
  it('parses script', () => {
    expect(varNameToSource('script')).toEqual({ source: 'script', value: '' });
  });
  it('parses script.field', () => {
    expect(varNameToSource('script.result')).toEqual({ source: 'script', value: 'result' });
  });
  it('unknown falls back to fixed', () => {
    expect(varNameToSource('unknown')).toEqual({ source: 'fixed', value: 'unknown' });
  });
});

// ── xmlFieldsToTemplate ──────────────────────────────────────────────

describe('xmlFieldsToTemplate', () => {
  it('empty fields produce empty root', () => {
    expect(xmlFieldsToTemplate([])).toBe('<response></response>');
  });

  it('custom root tag', () => {
    expect(xmlFieldsToTemplate([], 'data')).toBe('<data></data>');
  });

  it('single value node', () => {
    const fields = [{ tag: 'id', nodeType: 'value', source: 'fixed', value: '42', pipe: '' }];
    expect(xmlFieldsToTemplate(fields)).toBe('<response><id>42</id></response>');
  });

  it('variable with pipe', () => {
    const fields = [{ tag: 'siren', nodeType: 'value', source: 'path', value: 'siret', pipe: 'first(9)' }];
    expect(xmlFieldsToTemplate(fields)).toBe('<response><siren>{{path.siret | first(9)}}</siren></response>');
  });

  it('nested parent node', () => {
    const fields = [{
      tag: 'data',
      nodeType: 'parent',
      children: [
        { tag: 'id', nodeType: 'value', source: 'uuid', value: '', pipe: '' },
        { tag: 'name', nodeType: 'value', source: 'fake', value: 'FirstName', pipe: '' },
      ],
    }];
    const tpl = xmlFieldsToTemplate(fields, 'root');
    expect(tpl).toBe('<root><data><id>{{uuid}}</id><name>{{fake.FirstName}}</name></data></root>');
  });

  it('what xmlFieldsToTemplate produces is valid XML', () => {
    const fields = [
      { tag: 'id', nodeType: 'value', source: 'uuid', value: '', pipe: '' },
      { tag: 'info', nodeType: 'parent', children: [
        { tag: 'city', nodeType: 'value', source: 'fake', value: 'CityFR', pipe: 'upper' },
      ]},
    ];
    const tpl = xmlFieldsToTemplate(fields, 'resp');
    expect(validateTemplateAsXml(tpl)).toBeNull();
  });
});

// ── XML validation edge cases ────────────────────────────────────────

describe('validateTemplateAsXml edge cases', () => {
  it('accepts variables with pipes inside tags', () => {
    expect(validateTemplateAsXml('<r><v>{{path.x | upper}}</v></r>')).toBeNull();
  });

  it('accepts multiple variables in same tag', () => {
    expect(validateTemplateAsXml('<r>{{fake.FirstName}} {{fake.LastName}}</r>')).toBeNull();
  });

  it('rejects mismatched tags', () => {
    expect(validateTemplateAsXml('<a><b></a>')).not.toBeNull();
  });

  it('rejects empty tag names', () => {
    expect(validateTemplateAsXml('<></>') ).not.toBeNull();
  });
});
