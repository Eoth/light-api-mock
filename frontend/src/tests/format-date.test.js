import { describe, it, expect } from 'vitest';
import { formatDateTime, formatDateTimePrecise } from '../lib/format-date.js';

// Source unique de verite pour le formatage date/heure, regroupant les 3
// fidelites d'affichage historiquement dupliquees (RequestLog/MessagingLog,
// BackupManager, RuleTester). Chaque site d'appel doit continuer a produire
// exactement le meme rendu qu'avant la centralisation.
describe('formatDateTime', () => {
  const ts = new Date('2026-01-10T12:05:09').getTime();

  it('sans locale ni options : equivalent a toLocaleString() (usage RuleTester)', () => {
    expect(formatDateTime(ts)).toBe(new Date(ts).toLocaleString());
  });

  it('avec locale seule, sans options : equivalent a toLocaleString(locale) (usage BackupManager)', () => {
    expect(formatDateTime(ts, undefined, 'fr-FR')).toBe(new Date(ts).toLocaleString('fr-FR'));
  });

  it('avec locale et options : equivalent a toLocaleString(locale, options)', () => {
    const options = { year: 'numeric', month: 'long' };
    expect(formatDateTime(ts, options, 'en-US')).toBe(new Date(ts).toLocaleString('en-US', options));
  });
});

describe('formatDateTimePrecise', () => {
  const ts = new Date('2026-01-10T12:05:09').getTime();

  it('formate en fr-FR avec jour/mois/annee/heure/minute/seconde en 2 chiffres (usage RequestLog/MessagingLog)', () => {
    const expected = new Date(ts).toLocaleString('fr-FR', {
      day: '2-digit',
      month: '2-digit',
      year: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
    expect(formatDateTimePrecise(ts)).toBe(expected);
  });

  it('accepte une locale explicite differente', () => {
    const expected = new Date(ts).toLocaleString('en-US', {
      day: '2-digit',
      month: '2-digit',
      year: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
    expect(formatDateTimePrecise(ts, 'en-US')).toBe(expected);
  });
});
