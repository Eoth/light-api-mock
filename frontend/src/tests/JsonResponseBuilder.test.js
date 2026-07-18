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

describe('JsonResponseBuilder — pliage/depliage des noeuds imbriques', () => {
  it('tout est deplie par defaut (aucune regression sur le rendu existant)', () => {
    const { getByLabelText, getByDisplayValue } = render(JsonResponseBuilder, { props: { fields: nestedFields } });
    expect(getByLabelText('Replier unite_legale')).toHaveAttribute('aria-expanded', 'true');
    expect(getByDisplayValue('nom')).toBeVisible();
    expect(getByDisplayValue('adresse')).toBeVisible();
    expect(getByDisplayValue('ville')).toBeVisible();
  });

  it('replier un noeud masque son contenu et affiche un indicateur, sans perdre les donnees', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText, getByDisplayValue } = render(JsonResponseBuilder, { props: { fields: nestedFields, onUpdate } });

    const toggle = getByLabelText('Replier unite_legale');
    await fireEvent.click(toggle);

    expect(getByLabelText('Deplier unite_legale')).toHaveAttribute('aria-expanded', 'false');
    expect(getByDisplayValue('nom')).not.toBeVisible();
    expect(getByDisplayValue('adresse')).not.toBeVisible();
    expect(getByDisplayValue('ville')).not.toBeVisible();
    // Rien n'a ete mute : replier est un pur affichage.
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it('depliage restaure le contenu masque, aucune donnee perdue', async () => {
    const { getByLabelText, getByDisplayValue } = render(JsonResponseBuilder, { props: { fields: nestedFields } });

    await fireEvent.click(getByLabelText('Replier unite_legale'));
    await fireEvent.click(getByLabelText('Deplier unite_legale'));

    expect(getByLabelText('Replier unite_legale')).toHaveAttribute('aria-expanded', 'true');
    expect(getByDisplayValue('nom')).toBeVisible();
    expect(getByDisplayValue('adresse')).toBeVisible();
    expect(getByDisplayValue('ville')).toBeVisible();
  });

  it('un champ de type "valeur" (sans enfants) n\'a pas de chevron de pliage', () => {
    const { queryByLabelText } = render(JsonResponseBuilder, { props: { fields: nestedFields } });
    expect(queryByLabelText('Replier nom')).not.toBeInTheDocument();
    expect(queryByLabelText('Deplier nom')).not.toBeInTheDocument();
  });
});

// Correctif "diagnostic reponse JSON/XML" : `needsValueInput` n'incluait pas
// 'script', contrairement au mode "par exemple" (JsonPasteBuilder.svelte) --
// choisir cette source masquait le champ qui permet de preciser QUELLE cle
// du resultat de script utiliser (buildExpr produit `script.${valeur}`, cf
// tpl-utils.js).
describe('JsonResponseBuilder — source "Resultat du script"', () => {
  const scriptField = [{ key: 'nom', fieldType: 'value', source: 'fixed', value: '', pipe: '', asNumber: false }];

  it('affiche le champ de saisie de valeur quand la source "script" est choisie', async () => {
    const { getByLabelText } = render(JsonResponseBuilder, { props: { fields: scriptField } });

    await fireEvent.change(getByLabelText('Source de la valeur'), { target: { value: 'script' } });

    expect(getByLabelText('Valeur')).toBeInTheDocument();
  });

  it('transmet la cle du script saisie via onUpdate', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText } = render(JsonResponseBuilder, { props: { fields: scriptField, onUpdate } });

    await fireEvent.change(getByLabelText('Source de la valeur'), { target: { value: 'script' } });
    await fireEvent.input(getByLabelText('Valeur'), { target: { value: 'total' } });

    const [updated] = onUpdate.mock.calls.at(-1);
    expect(updated[0].source).toBe('script');
    expect(updated[0].value).toBe('total');
  });
});
