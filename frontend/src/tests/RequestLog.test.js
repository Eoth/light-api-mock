import { render, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import RequestLog from '../lib/components/RequestLog.svelte';
import { getLogs } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  getLogs: vi.fn(),
}));

describe('RequestLog date/heure', () => {
  it('affiche la date et l\'heure de la requete, pas seulement l\'heure', async () => {
    const ts = new Date('2026-01-15T10:30:00').getTime();
    getLogs.mockResolvedValue([
      { timestamp: ts, service_name: 'svc-a', method: 'GET', path: '/svc-a/foo', mode: 'mock', rule_matched: 'r1', target_url: null, status: 200 },
    ]);

    const { getByText, container } = render(RequestLog);

    await waitFor(() => expect(getByText('svc-a')).toBeInTheDocument());

    const expected = new Date(ts).toLocaleString('fr-FR', {
      day: '2-digit', month: '2-digit', year: '2-digit',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
    const cell = container.querySelector('.col-time');
    expect(cell.textContent).toBe(expected);
    expect(cell.textContent).toMatch(/\d{2}\/\d{2}\/\d{2}/);
  });

  it('affiche l\'en-tete de colonne "Date/Heure"', async () => {
    getLogs.mockResolvedValue([
      { timestamp: Date.now(), service_name: 'svc-a', method: 'GET', path: '/svc-a/foo', mode: 'mock', rule_matched: 'r1', target_url: null, status: 200 },
    ]);
    const { getByText } = render(RequestLog);
    await waitFor(() => expect(getByText('Date/Heure')).toBeInTheDocument());
  });
});
