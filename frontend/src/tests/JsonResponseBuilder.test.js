import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import JsonResponseBuilder from '../lib/components/JsonResponseBuilder.svelte';

const nestedFields = [
  {
    key: 'unite_legale',
    fieldType: 'object',
    children: [
      { key: 'nom', fieldType: 'value', source: 'fixed', value: 'ACME', pipe: '', asNumber: false },
      {
        key: 'adresse',
        fieldType: 'object',
        children: [
          { key: 'ville', fieldType: 'value', source: 'fixed', value: 'Paris', pipe: '', asNumber: false },
        ],
      },
    ],
  },
];

describe('JsonResponseBuilder — breadcrumb de navigation', () => {
  it('n\'affiche pas de breadcrumb tant qu\'on est a la racine', () => {
    const { queryByLabelText } = render(JsonResponseBuilder, { props: { fields: nestedFields } });
    expect(queryByLabelText('Chemin des donnees')).not.toBeInTheDocument();
  });

  it('affiche le rendu complet par defaut (pas de perte de fonctionnalite)', () => {
    const { getByDisplayValue } = render(JsonResponseBuilder, { props: { fields: nestedFields } });
    expect(getByDisplayValue('unite_legale')).toBeInTheDocument();
    expect(getByDisplayValue('nom')).toBeInTheDocument();
    expect(getByDisplayValue('adresse')).toBeInTheDocument();
    expect(getByDisplayValue('ville')).toBeInTheDocument();
  });

  it('navigue dans un objet imbrique via le bouton "Naviguer"', async () => {
    const { getByLabelText, getByRole } = render(JsonResponseBuilder, { props: { fields: nestedFields } });

    await fireEvent.click(getByLabelText('Naviguer dans unite_legale'));

    const breadcrumb = getByRole('navigation', { name: 'Chemin des donnees' });
    expect(breadcrumb).toBeInTheDocument();
    expect(breadcrumb).toHaveTextContent('racine');
    expect(breadcrumb).toHaveTextContent('unite_legale');
  });

  it('permet de remonter en cliquant sur "racine" dans le breadcrumb', async () => {
    const { getByLabelText, getByText, getByDisplayValue } = render(JsonResponseBuilder, { props: { fields: nestedFields } });

    await fireEvent.click(getByLabelText('Naviguer dans unite_legale'));
    await fireEvent.click(getByLabelText('Naviguer dans adresse'));
    expect(getByDisplayValue('ville')).toBeInTheDocument();

    await fireEvent.click(getByText('racine'));
    expect(getByDisplayValue('unite_legale')).toBeInTheDocument();
  });

  it('le dernier segment du breadcrumb n\'est pas cliquable (aria-current page)', async () => {
    const { getByLabelText, getByText } = render(JsonResponseBuilder, { props: { fields: nestedFields } });
    await fireEvent.click(getByLabelText('Naviguer dans unite_legale'));

    const current = getByText('unite_legale', { selector: 'span' }).closest('li');
    expect(current).toHaveAttribute('aria-current', 'page');
  });

  it('ajoute un nouveau champ dans le sous-niveau focus, pas a la racine', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText, getByText } = render(JsonResponseBuilder, {
      props: { fields: nestedFields, onUpdate },
    });

    await fireEvent.click(getByLabelText('Naviguer dans unite_legale'));
    await fireEvent.click(getByText('+ Ajouter un champ'));

    const [updated] = onUpdate.mock.calls.at(-1);
    expect(updated[0].children).toHaveLength(3);
    expect(updated).toHaveLength(1);
  });
});
