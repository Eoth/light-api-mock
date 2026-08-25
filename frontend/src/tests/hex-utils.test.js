import { describe, it, expect } from 'vitest';
import { textToHex, hexToBytes, hexToTextOrNull, isValidHex } from '../lib/hex-utils.js';

describe('hex-utils', () => {
  it('textToHex encode "pong" comme le backend (miroir de tcp::hex::encode)', () => {
    expect(textToHex('pong')).toBe('706f6e67');
  });

  it('textToHex sur une chaine vide donne une chaine vide', () => {
    expect(textToHex('')).toBe('');
  });

  it('textToHex gere l\'UTF-8 multi-octets', () => {
    expect(textToHex('é')).toBe('c3a9');
  });

  it('hexToBytes decode un hex valide', () => {
    expect(Array.from(hexToBytes('706f6e67'))).toEqual([0x70, 0x6f, 0x6e, 0x67]);
  });

  it('hexToBytes accepte la casse haute et basse', () => {
    expect(Array.from(hexToBytes('AB'))).toEqual([0xab]);
    expect(Array.from(hexToBytes('ab'))).toEqual([0xab]);
  });

  it('hexToBytes renvoie null sur une longueur impaire', () => {
    expect(hexToBytes('abc')).toBeNull();
  });

  it('hexToBytes renvoie null sur un caractere invalide', () => {
    expect(hexToBytes('zz')).toBeNull();
  });

  it('hexToTextOrNull fait l\'aller-retour avec textToHex', () => {
    expect(hexToTextOrNull(textToHex('hello world'))).toBe('hello world');
  });

  it('hexToTextOrNull renvoie null sur du hex invalide', () => {
    expect(hexToTextOrNull('zz')).toBeNull();
  });

  it('hexToTextOrNull renvoie null sur des octets qui ne forment pas de l\'UTF-8 valide', () => {
    // 0xff seul n'est jamais un debut de sequence UTF-8 valide.
    expect(hexToTextOrNull('ff')).toBeNull();
  });

  it('isValidHex accepte une chaine vide (aucun octet)', () => {
    expect(isValidHex('')).toBe(true);
  });

  it('isValidHex rejette une longueur impaire', () => {
    expect(isValidHex('abc')).toBe(false);
  });

  it('isValidHex rejette un caractere hors hexadecimal', () => {
    expect(isValidHex('zz')).toBe(false);
  });
});
