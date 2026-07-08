import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import RuleForm from '../lib/components/RuleForm.svelte';

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
      props: { existingRuleNames: ['existing-rule'], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'existing-rule');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('existe deja');
  });

  it('refuse un doublon insensible a la casse', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(RuleForm, {
      props: { existingRuleNames: ['My-Rule'], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'my-rule');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('existe deja');
  });

  it('accepte un nom unique', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container } = render(RuleForm, {
      props: { existingRuleNames: ['other-rule'], onSave },
    });

    await setInput(getByLabelText('Nom de la regle'), 'new-rule');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('accepte le meme nom en edition (exclus de la liste)', async () => {
    const onSave = vi.fn();
    const existingRule = {
      name: 'edit-me',
      action: 'mock',
      conditions: { all_of: [], any_of: [] },
      response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'ok' }], chaos: null },
    };
    const { container } = render(RuleForm, {
      props: { rule: existingRule, existingRuleNames: [], onSave },
    });

    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
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
    const onSave = vi.fn();
    const { getByLabelText, container } = render(RuleForm, { props: { onSave } });

    await setInput(getByLabelText('Nom de la regle'), 'r1');
    await submitForm(container);

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

    const [payload] = onSave.mock.calls[0];
    expect(payload.pre_script).toBe('"pre-result"');
    expect(payload.post_script).toBe('"post-result"');
  });

  it('n\'envoie pas pre_script si le champ reste vide meme toggle actif', async () => {
    const onSave = vi.fn();
    const { getByLabelText, getByRole, container } = render(RuleForm, { props: { onSave } });

    await setInput(getByLabelText('Nom de la regle'), 'r3');
    await fireEvent.click(getByRole('switch', { name: 'Pré-script (préparation)' }));
    await submitForm(container);

    const [payload] = onSave.mock.calls[0];
    expect(payload.pre_script).toBeNull();
  });
});
