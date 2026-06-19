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
