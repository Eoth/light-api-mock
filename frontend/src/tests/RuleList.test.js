import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import RuleList from '../lib/components/RuleList.svelte';

const mockRules = [
  { name: 'rule-siret', conditions: { all_of: [{ source: { type: 'QueryParam', key: 'q' }, operator: { type: 'Eq', value: '1' } }], any_of: [] }, response: { status: 200, body: [] } },
  { name: 'catch-all', conditions: { all_of: [], any_of: [] }, response: { status: 200, body: [] } },
];

describe('RuleList', () => {
  it('affiche un message quand pas de regles', () => {
    const { getByText } = render(RuleList, { props: { rules: [] } });
    expect(getByText(/Aucune regle definie/)).toBeInTheDocument();
  });

  it('affiche toutes les regles', () => {
    const { getByText } = render(RuleList, { props: { rules: mockRules } });
    expect(getByText('rule-siret')).toBeInTheDocument();
    expect(getByText('catch-all')).toBeInTheDocument();
  });

  it('affiche le nombre de conditions', () => {
    const { getByText } = render(RuleList, { props: { rules: mockRules } });
    expect(getByText('1 condition')).toBeInTheDocument();
    expect(getByText('Catch-all')).toBeInTheDocument();
  });

  it('le bouton ajouter appelle onAddRule', async () => {
    const onAddRule = vi.fn();
    const { getByText } = render(RuleList, { props: { rules: mockRules, onAddRule } });
    await fireEvent.click(getByText('+ Ajouter une regle'));
    expect(onAddRule).toHaveBeenCalledOnce();
  });

  it('le bouton modifier appelle onEditRule avec l index', async () => {
    const onEditRule = vi.fn();
    const { getAllByTitle } = render(RuleList, { props: { rules: mockRules, onEditRule } });
    const editBtns = getAllByTitle('Modifier');
    await fireEvent.click(editBtns[0]);
    expect(onEditRule).toHaveBeenCalledWith(0);
  });

  it('le bouton supprimer appelle onDeleteRule avec l index', async () => {
    const onDeleteRule = vi.fn();
    const { getAllByTitle } = render(RuleList, { props: { rules: mockRules, onDeleteRule } });
    const deleteBtns = getAllByTitle('Supprimer');
    await fireEvent.click(deleteBtns[1]);
    expect(onDeleteRule).toHaveBeenCalledWith(1);
  });

  it('le bouton ajouter fonctionne meme avec des regles existantes', async () => {
    const onAddRule = vi.fn();
    const { getByText } = render(RuleList, { props: { rules: mockRules, onAddRule } });
    const btn = getByText('+ Ajouter une regle');
    expect(btn).not.toBeDisabled();
    await fireEvent.click(btn);
    expect(onAddRule).toHaveBeenCalledOnce();
  });

  it('les boutons monter/descendre sont desactives aux extremites', () => {
    const { getAllByTitle } = render(RuleList, { props: { rules: mockRules } });
    const upBtns = getAllByTitle('Monter');
    const downBtns = getAllByTitle('Descendre');
    expect(upBtns[0]).toBeDisabled();
    expect(downBtns[downBtns.length - 1]).toBeDisabled();
    expect(downBtns[0]).not.toBeDisabled();
    expect(upBtns[upBtns.length - 1]).not.toBeDisabled();
  });
});
