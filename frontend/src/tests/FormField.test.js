import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import FormFieldHarness from './helpers/FormFieldHarness.svelte';

describe('FormField', () => {
  it('associe le label au champ via for/id', () => {
    const { getByLabelText } = render(FormFieldHarness, {
      props: { id: 'svc-name', label: 'Nom du service' },
    });
    expect(getByLabelText('Nom du service')).toBeInTheDocument();
  });

  it('relie le hint via aria-describedby', () => {
    const { getByLabelText, getByText } = render(FormFieldHarness, {
      props: { id: 'svc-name', label: 'Nom du service', hint: 'Identifiant unique' },
    });
    const input = getByLabelText('Nom du service');
    const hint = getByText('Identifiant unique');
    expect(input.getAttribute('aria-describedby')).toContain(hint.id);
  });

  it('affiche l\'erreur avec role alert et aria-invalid sur le champ', () => {
    const { getByLabelText, getByRole } = render(FormFieldHarness, {
      props: { id: 'svc-name', label: 'Nom du service', error: 'Le nom est requis.' },
    });
    const input = getByLabelText('Nom du service');
    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(getByRole('alert')).toHaveTextContent('Le nom est requis.');
  });

  it('n\'affiche pas aria-invalid quand il n\'y a pas d\'erreur', () => {
    const { getByLabelText } = render(FormFieldHarness, {
      props: { id: 'svc-name', label: 'Nom du service' },
    });
    expect(getByLabelText('Nom du service').getAttribute('aria-invalid')).toBe('false');
  });
});
