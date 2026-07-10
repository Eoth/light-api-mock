import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import RuleTester from '../lib/components/RuleTester.svelte';
import { testRule } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  testRule: vi.fn(),
}));

const logWithDetail = {
  timestamp: new Date('2026-01-10T12:00:00').getTime(),
  service_name: 'svc-a',
  method: 'GET',
  path: '/svc-a/orders/42',
  mode: 'mock',
  rule_matched: 'r1',
  target_url: null,
  status: 200,
  captured: {
    remaining_path: '/orders/42',
    path_params: {},
    query_params: { foo: 'bar' },
    headers: { 'x-env': 'prod' },
    body: '',
    body_truncated: false,
    content_type: null,
  },
};

const logWithoutDetail = {
  timestamp: new Date('2026-01-10T12:05:00').getTime(),
  service_name: 'svc-a',
  method: 'GET',
  path: '/svc-a/passthrough',
  mode: 'proxy',
  rule_matched: null,
  target_url: 'http://backend/passthrough',
  status: 200,
  captured: null,
};

function draft(overrides = {}) {
  return () => ({
    method: 'GET',
    subPath: '',
    allOf: [],
    anyOf: [],
    ...overrides,
  });
}

describe('RuleTester: filtrage des entrees sans detail', () => {
  it('affiche un message quand aucune requete n\'a ete capturee', () => {
    const { getByText } = render(RuleTester, {
      props: { serviceName: 'svc-a', logs: [], getDraftRule: draft() },
    });
    expect(getByText(/Aucune requête n'a encore été capturée/)).toBeInTheDocument();
  });

  it('affiche un message quand seules des entrees sans detail existent', () => {
    const { getByText } = render(RuleTester, {
      props: { serviceName: 'svc-a', logs: [logWithoutDetail], getDraftRule: draft() },
    });
    expect(getByText(/proxy direct/)).toBeInTheDocument();
  });

  it('ne liste que les entrees avec detail capture dans le selecteur', () => {
    const { getByLabelText, queryByText } = render(RuleTester, {
      props: { serviceName: 'svc-a', logs: [logWithDetail, logWithoutDetail], getDraftRule: draft() },
    });
    const select = getByLabelText('Requête capturée');
    const options = [...select.querySelectorAll('option')].filter((o) => o.value !== '');
    expect(options).toHaveLength(1);
    expect(queryByText(/passthrough/)).not.toBeInTheDocument();
  });
});

describe('RuleTester: appel API et affichage du resultat', () => {
  it('envoie le brouillon de regle et la requete capturee choisie', async () => {
    testRule.mockResolvedValue({
      method_matches: true,
      sub_path_matches: true,
      path_params: {},
      overall_matched: true,
      body_truncated: false,
      all_of: [],
      any_of: [],
    });

    const { getByLabelText, getByRole } = render(RuleTester, {
      props: {
        serviceName: 'svc-a',
        logs: [logWithDetail],
        getDraftRule: draft({
          allOf: [{ source: { type: 'QueryParam', key: 'foo' }, operator: { type: 'Eq', value: 'bar' } }],
        }),
      },
    });

    const select = getByLabelText('Requête capturée');
    await fireEvent.change(select, { target: { value: '0' } });
    await fireEvent.click(getByRole('button', { name: /Tester contre cette requête/ }));

    await waitFor(() => expect(testRule).toHaveBeenCalled());
    const [payload] = testRule.mock.calls[0];
    expect(payload.method).toBe('GET');
    expect(payload.conditions.all_of[0].source.key).toBe('foo');
    expect(payload.request.query_params).toEqual({ foo: 'bar' });
    expect(payload.request.remaining_path).toBe('/orders/42');
  });

  it('affiche le detail condition par condition avec icone et texte (pas seulement de la couleur)', async () => {
    testRule.mockResolvedValue({
      method_matches: true,
      sub_path_matches: true,
      path_params: {},
      overall_matched: false,
      body_truncated: false,
      all_of: [
        {
          condition: { source: { type: 'QueryParam', key: 'foo' }, operator: { type: 'Eq', value: 'bar' } },
          matched: false,
          found_value: null,
          hint: "'foo' n'a pas ete trouve comme parametre de requete, mais est present comme parametre de chemin dans cette requete",
        },
      ],
      any_of: [],
    });

    const { getByLabelText, getByRole, getByText } = render(RuleTester, {
      props: { serviceName: 'svc-a', logs: [logWithDetail], getDraftRule: draft() },
    });

    await fireEvent.change(getByLabelText('Requête capturée'), { target: { value: '0' } });
    await fireEvent.click(getByRole('button', { name: /Tester contre cette requête/ }));

    await waitFor(() => expect(getByText(/ne matcherait pas cette requête/)).toBeInTheDocument());
    expect(getByText(/ne correspond pas/)).toBeInTheDocument();
    expect(getByText(/valeur trouvée : absente/)).toBeInTheDocument();
    expect(getByText(/present comme parametre de chemin/)).toBeInTheDocument();
  });

  it('affiche une banniere d\'avertissement quand le corps capture est tronque', async () => {
    testRule.mockResolvedValue({
      method_matches: true,
      sub_path_matches: true,
      path_params: {},
      overall_matched: false,
      body_truncated: true,
      all_of: [
        {
          condition: { source: { type: 'JsonPointer', key: '/a' }, operator: { type: 'Exists' } },
          matched: false,
          found_value: null,
          hint: null,
        },
      ],
      any_of: [],
    });

    const { getByLabelText, getByRole, getByText } = render(RuleTester, {
      props: { serviceName: 'svc-a', logs: [logWithDetail], getDraftRule: draft() },
    });

    await fireEvent.change(getByLabelText('Requête capturée'), { target: { value: '0' } });
    await fireEvent.click(getByRole('button', { name: /Tester contre cette requête/ }));

    await waitFor(() => expect(getByText(/corps de cette requête a été tronqué/)).toBeInTheDocument());
  });
});
