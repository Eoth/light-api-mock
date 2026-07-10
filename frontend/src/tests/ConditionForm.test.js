import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ConditionForm from '../lib/components/ConditionForm.svelte';

async function setInput(el, value) {
  el.value = value;
  await fireEvent.input(el);
}

describe('ConditionForm: selection stricte du path param', () => {
  it('masque l\'option "Parametre de chemin" quand aucun path param n\'est disponible', () => {
    const { getByLabelText } = render(ConditionForm, { props: { availablePathParams: [] } });
    const options = [...getByLabelText('Source').querySelectorAll('option')].map((o) => o.value);
    expect(options).not.toContain('PathParam');
  });

  it('affiche l\'option "Parametre de chemin" quand des path params sont disponibles', () => {
    const { getByLabelText } = render(ConditionForm, { props: { availablePathParams: ['id'] } });
    const options = [...getByLabelText('Source').querySelectorAll('option')].map((o) => o.value);
    expect(options).toContain('PathParam');
  });

  it('affiche un badge du nombre de path params disponibles avant ouverture du select', () => {
    const { getByText } = render(ConditionForm, { props: { availablePathParams: ['id', 'orderId', 'userId'] } });
    expect(getByText(/3 paramètres de chemin disponibles : id, orderId, userId/)).toBeInTheDocument();
  });

  it('propose un select ferme (pas de saisie libre) pour la cle quand PathParam est choisi', async () => {
    const { getByLabelText } = render(ConditionForm, { props: { availablePathParams: ['id', 'orderId'] } });
    const sourceSelect = getByLabelText('Source');
    await fireEvent.change(sourceSelect, { target: { value: 'PathParam' } });

    const keyField = getByLabelText('Paramètre de chemin');
    expect(keyField.tagName).toBe('SELECT');
    const options = [...keyField.querySelectorAll('option')].map((o) => o.value).filter(Boolean);
    expect(options).toEqual(['id', 'orderId']);
  });

  it('conserve une condition PathParam existante meme si la liste devient vide', () => {
    const { getByLabelText } = render(ConditionForm, {
      props: {
        availablePathParams: [],
        condition: { source: { type: 'PathParam', key: 'legacyParam' }, operator: { type: 'Exists' } },
      },
    });
    const sourceSelect = getByLabelText('Source');
    const options = [...sourceSelect.querySelectorAll('option')].map((o) => o.value);
    expect(options).toContain('PathParam');
    const keyField = getByLabelText('Paramètre de chemin');
    const keyOptions = [...keyField.querySelectorAll('option')].map((o) => o.value).filter(Boolean);
    expect(keyOptions).toContain('legacyParam');
  });
});

describe('ConditionForm: autocompletion query param', () => {
  it('propose une datalist non contraignante basee sur les suggestions', async () => {
    const { getByLabelText, container } = render(ConditionForm, {
      props: { queryParamSuggestions: ['debug', 'trace'] },
    });
    const keyField = getByLabelText('Clé / Chemin');
    expect(keyField.getAttribute('list')).toBe('cond-query-param-suggestions');
    const datalist = container.querySelector('#cond-query-param-suggestions');
    const options = [...datalist.querySelectorAll('option')].map((o) => o.value);
    expect(options).toEqual(['debug', 'trace']);
  });

  it('permet toujours de saisir un nom de query param inedit', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container } = render(ConditionForm, {
      props: { queryParamSuggestions: ['debug'], onSave },
    });
    await setInput(getByLabelText('Clé / Chemin'), 'brand-new-param');
    await setInput(getByLabelText('Valeur attendue'), '1');
    await fireEvent.submit(container.querySelector('form'));
    expect(onSave).toHaveBeenCalledWith({
      source: { type: 'QueryParam', key: 'brand-new-param' },
      operator: { type: 'Eq', value: '1' },
    });
  });
});

describe('ConditionForm: distinction visuelle des sources', () => {
  it('utilise des libelles distincts pour PathParam et QueryParam', () => {
    const { getByLabelText } = render(ConditionForm, { props: { availablePathParams: ['id'] } });
    const optionTexts = [...getByLabelText('Source').querySelectorAll('option')].map((o) => o.textContent);
    const pathLabel = optionTexts.find((t) => t.includes('chemin'));
    const queryLabel = optionTexts.find((t) => t.includes('requete'));
    expect(pathLabel).not.toEqual(queryLabel);
    expect(pathLabel).toContain('{param}');
    expect(queryLabel).toContain('?cle=valeur');
  });
});
