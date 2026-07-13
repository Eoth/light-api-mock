import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import XmlResponseBuilder from '../lib/components/XmlResponseBuilder.svelte';

const nestedFields = [
  {
    tag: 'unite_legale',
    nodeType: 'parent',
    children: [
      { tag: 'nom', nodeType: 'value', source: 'fixed', value: 'ACME' },
      {
        tag: 'adresse',
        nodeType: 'parent',
        children: [
          { tag: 'ville', nodeType: 'value', source: 'fixed', value: 'Paris' },
        ],
      },
    ],
  },
];

describe('XmlResponseBuilder — pliage/depliage des noeuds parents', () => {
  it('tout est deplie par defaut (aucune regression sur le rendu existant)', () => {
    const { getByLabelText, getByDisplayValue } = render(XmlResponseBuilder, { props: { fields: nestedFields } });
    expect(getByLabelText('Replier unite_legale')).toHaveAttribute('aria-expanded', 'true');
    expect(getByDisplayValue('nom')).toBeVisible();
    expect(getByDisplayValue('adresse')).toBeVisible();
    expect(getByDisplayValue('ville')).toBeVisible();
  });

  it('replier un noeud parent masque ses enfants et affiche un indicateur, sans muter les donnees', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText, getByDisplayValue } = render(XmlResponseBuilder, { props: { fields: nestedFields, onUpdate } });

    await fireEvent.click(getByLabelText('Replier unite_legale'));

    expect(getByLabelText('Deplier unite_legale')).toHaveAttribute('aria-expanded', 'false');
    expect(getByDisplayValue('nom')).not.toBeVisible();
    expect(getByDisplayValue('adresse')).not.toBeVisible();
    expect(getByDisplayValue('ville')).not.toBeVisible();
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it('depliage restaure le contenu masque, aucune donnee perdue', async () => {
    const { getByLabelText, getByDisplayValue } = render(XmlResponseBuilder, { props: { fields: nestedFields } });

    await fireEvent.click(getByLabelText('Replier unite_legale'));
    await fireEvent.click(getByLabelText('Deplier unite_legale'));

    expect(getByLabelText('Replier unite_legale')).toHaveAttribute('aria-expanded', 'true');
    expect(getByDisplayValue('nom')).toBeVisible();
    expect(getByDisplayValue('ville')).toBeVisible();
  });

  it('un noeud de type "Contenu" (sans enfants) n\'a pas de chevron de pliage', () => {
    const { queryByLabelText } = render(XmlResponseBuilder, { props: { fields: nestedFields } });
    expect(queryByLabelText('Replier nom')).not.toBeInTheDocument();
    expect(queryByLabelText('Deplier nom')).not.toBeInTheDocument();
  });
});
