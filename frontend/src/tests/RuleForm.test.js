import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import RuleForm from '../lib/components/RuleForm.svelte';
import { getLogs, checkRuleConflicts } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  validateScript: vi.fn(),
  getLogs: vi.fn(),
  checkRuleConflicts: vi.fn(),
}));

async function setInput(el, value) {
  el.value = value;
  await fireEvent.input(el);
}

async function submitForm(container) {
  const form = container.querySelector('form');
  await fireEvent.submit(form);
}

describe('RuleForm: rule name uniqueness', () => {
  it('refuse un nom de regle deja existant dans le service', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(RuleForm, {
      props: { existingRules: [{ name: 'existing-rule' }], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'existing-rule');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('existe deja');
  });

  it('refuse un doublon insensible a la casse', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(RuleForm, {
      props: { existingRules: [{ name: 'My-Rule' }], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'my-rule');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('existe deja');
  });

  it('accepte un nom unique', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const { getByLabelText, container } = render(RuleForm, {
      props: { existingRules: [{ name: 'other-rule' }], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);
    await waitFor(() => expect(onSave).toHaveBeenCalled());
  });

  it('accepte le meme nom en edition (exclus de la liste)', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const existingRule = {
      name: 'edit-me',
      action: 'mock',
      conditions: { all_of: [], any_of: [] },
      response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'ok' }], chaos: null },
    };
    const { container } = render(RuleForm, {
      props: { rule: existingRule, existingRules: [], onSave },
    });

    await submitForm(container);
    await waitFor(() => expect(onSave).toHaveBeenCalled());
  });

  it('refuse un nom de regle vide', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(RuleForm, {
      props: { onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), '');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('requis');
  });
});

describe('RuleForm: pre_script / post_script', () => {
  it('envoie pre_script et post_script a null quand les toggles restent desactives', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const { getByLabelText, container } = render(RuleForm, { props: { onSave } });

    await setInput(getByLabelText('Nom de la regle'), 'r1');
    await submitForm(container);

    await waitFor(() => expect(onSave).toHaveBeenCalled());
    const [payload] = onSave.mock.calls[0];
    expect(payload.pre_script).toBeNull();
    expect(payload.post_script).toBeNull();
  });

  it('affiche les toggles Pré-script et Post-script', () => {
    const { getByRole } = render(RuleForm);
    expect(getByRole('switch', { name: 'Pré-script (préparation)' })).toBeInTheDocument();
    expect(getByRole('switch', { name: 'Post-script (finalisation)' })).toBeInTheDocument();
  });

  it('inclut pre_script et post_script dans le payload une fois actives et remplis', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const { getByLabelText, getByRole, container } = render(RuleForm, { props: { onSave } });

    await setInput(getByLabelText('Nom de la regle'), 'r2');
    await fireEvent.click(getByRole('switch', { name: 'Pré-script (préparation)' }));
    await fireEvent.click(getByRole('switch', { name: 'Post-script (finalisation)' }));

    const preTextarea = container.querySelector('#rule-pre-script');
    const postTextarea = container.querySelector('#rule-post-script');
    expect(preTextarea).toBeInTheDocument();
    expect(postTextarea).toBeInTheDocument();

    await setInput(preTextarea, '"pre-result"');
    await setInput(postTextarea, '"post-result"');
    await submitForm(container);

    await waitFor(() => expect(onSave).toHaveBeenCalled());
    const [payload] = onSave.mock.calls[0];
    expect(payload.pre_script).toBe('"pre-result"');
    expect(payload.post_script).toBe('"post-result"');
  });

  it('n\'envoie pas pre_script si le champ reste vide meme toggle actif', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const { getByLabelText, getByRole, container } = render(RuleForm, { props: { onSave } });

    await setInput(getByLabelText('Nom de la regle'), 'r3');
    await fireEvent.click(getByRole('switch', { name: 'Pré-script (préparation)' }));
    await submitForm(container);

    await waitFor(() => expect(onSave).toHaveBeenCalled());
    const [payload] = onSave.mock.calls[0];
    expect(payload.pre_script).toBeNull();
  });
});

describe('RuleForm: assistance de saisie path/query param', () => {
  it('ne rend pas le testeur de regle sans serviceName (retrocompat)', () => {
    const { queryByText } = render(RuleForm);
    expect(queryByText('Tester contre une requête réelle')).not.toBeInTheDocument();
  });

  it('rend le testeur de regle quand serviceName est fourni', async () => {
    getLogs.mockResolvedValue([]);
    const { getByText } = render(RuleForm, { props: { serviceName: 'svc-a' } });
    await waitFor(() => expect(getByText('Tester contre une requête réelle')).toBeInTheDocument());
  });

  it('combine les path params du service et du sous-chemin de la regle', async () => {
    getLogs.mockResolvedValue([]);
    const { getByLabelText, getByRole } = render(RuleForm, {
      props: { serviceName: 'svc-a', listenPath: '/orders/{id}' },
    });
    await setInput(getByLabelText('Sous-chemin (optionnel)'), '/items/{itemId}');
    await fireEvent.click(getByRole('button', { name: '+ Condition ET' }));
    const sourceSelect = getByLabelText('Source');
    expect(sourceSelect.querySelector('option[value="PathParam"]')).toBeInTheDocument();
  });
});

describe('RuleForm: detecteur de conflit a la sauvegarde', () => {
  const existingRules = [
    {
      name: 'existing-rule',
      method: 'GET',
      sub_path: null,
      conditions: { all_of: [], any_of: [] },
    },
  ];

  it('sauvegarde directement sans avertissement quand aucun conflit n\'est detecte', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const { getByLabelText, container, queryByTestId } = render(RuleForm, {
      props: { existingRules, draftPosition: 1, onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(queryByTestId('rule-form-conflict-warning')).not.toBeInTheDocument();
  });

  it('affiche un avertissement non bloquant quand un conflit est detecte, sans appeler onSave', async () => {
    checkRuleConflicts.mockResolvedValue({
      conflicts: [{ other_rule_name: 'existing-rule', winner: 'other' }],
    });
    const onSave = vi.fn();
    const { getByLabelText, container, findByTestId, getByRole } = render(RuleForm, {
      props: { existingRules, draftPosition: 1, onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);

    const warning = await findByTestId('rule-form-conflict-warning');
    expect(warning).toHaveTextContent('existing-rule');
    expect(getByRole('alert')).toBe(warning);
    expect(onSave).not.toHaveBeenCalled();
  });

  it('permet de sauvegarder quand meme malgre l\'avertissement', async () => {
    checkRuleConflicts.mockResolvedValue({
      conflicts: [{ other_rule_name: 'existing-rule', winner: 'draft' }],
    });
    const onSave = vi.fn();
    const { getByLabelText, container, findByTestId, getByTestId } = render(RuleForm, {
      props: { existingRules, draftPosition: 0, onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);
    await findByTestId('rule-form-conflict-warning');

    await fireEvent.click(getByTestId('rule-form-conflict-save-anyway-button'));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave.mock.calls[0][0].name).toBe('new-rule');
  });

  it('permet d\'annuler l\'avertissement pour continuer a modifier la regle', async () => {
    checkRuleConflicts.mockResolvedValue({
      conflicts: [{ other_rule_name: 'existing-rule', winner: 'other' }],
    });
    const onSave = vi.fn();
    const { getByLabelText, container, findByTestId, getByTestId, queryByTestId } = render(RuleForm, {
      props: { existingRules, draftPosition: 1, onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);
    await findByTestId('rule-form-conflict-warning');

    await fireEvent.click(getByTestId('rule-form-conflict-cancel-button'));
    expect(queryByTestId('rule-form-conflict-warning')).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
  });

  it('sauvegarde quand meme si la verification de conflit echoue (fail-open)', async () => {
    checkRuleConflicts.mockRejectedValue(new Error('reseau indisponible'));
    const onSave = vi.fn();
    const { getByLabelText, container } = render(RuleForm, {
      props: { existingRules, draftPosition: 1, onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
  });
});

describe('RuleForm: action Proxy masquee pour un service purement mocke (sujet 22)', () => {
  it('affiche les deux actions (Mock et Proxy) quand le service a une cible', () => {
    const { getByTestId, queryByTestId } = render(RuleForm, {
      props: { isPurelyMocked: false },
    });

    expect(getByTestId('rule-form-action-mock-radio')).toBeInTheDocument();
    expect(getByTestId('rule-form-action-proxy-radio')).toBeInTheDocument();
    expect(queryByTestId('rule-form-purely-mocked-hint')).not.toBeInTheDocument();
  });

  it('masque l\'action Proxy quand le service est purement mocke', () => {
    const { getByTestId, queryByTestId } = render(RuleForm, {
      props: { isPurelyMocked: true },
    });

    expect(getByTestId('rule-form-action-mock-radio')).toBeInTheDocument();
    expect(queryByTestId('rule-form-action-proxy-radio')).not.toBeInTheDocument();
  });

  it('une regle heritee en action=proxy repasse en mock a l\'ouverture si le service est purement mocke', async () => {
    checkRuleConflicts.mockResolvedValue({ conflicts: [] });
    const onSave = vi.fn();
    const staleRule = {
      name: 'stale-proxy-rule',
      action: 'proxy',
      conditions: { all_of: [], any_of: [] },
      response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'ok' }], chaos: null },
    };
    const { container } = render(RuleForm, {
      props: { rule: staleRule, existingRules: [], isPurelyMocked: true, onSave },
    });

    await submitForm(container);
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].action).toBe('mock');
  });
});
