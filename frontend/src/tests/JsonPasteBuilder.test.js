import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import JsonPasteBuilder from '../lib/components/JsonPasteBuilder.svelte';

async function pasteAndParse(getByLabelText, getByText, json) {
  const textarea = getByLabelText(/Collez un exemple/);
  await fireEvent.input(textarea, { target: { value: json } });
  await fireEvent.click(getByText('Analyser et variabiliser'));
}

describe('JsonPasteBuilder (exampleJsonToFields via tpl-utils.js)', () => {
  it('detecte les champs d\'un objet colle et les affiche', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText, getByText } = render(JsonPasteBuilder, { props: { onUpdate } });

    await pasteAndParse(getByLabelText, getByText, '{"siret":"44306184100047","nom":"ACME"}');

    await waitFor(() => expect(getByText('siret')).toBeInTheDocument());
    expect(getByText('nom')).toBeInTheDocument();
    expect(onUpdate).toHaveBeenCalled();
  });

  it('supporte un tableau racine (deja documente dans CLAUDE.md)', async () => {
    const onUpdate = vi.fn();
    const { getByLabelText, getByText } = render(JsonPasteBuilder, { props: { onUpdate } });

    await pasteAndParse(getByLabelText, getByText, '[{"id":1},{"id":2}]');

    await waitFor(() => expect(getByText('id')).toBeInTheDocument());
    const [fields] = onUpdate.mock.calls.at(-1);
    expect(fields[0].value).toBe('1');
  });

  it('affiche une erreur sur un JSON invalide', async () => {
    const { getByLabelText, getByText } = render(JsonPasteBuilder);
    await pasteAndParse(getByLabelText, getByText, '{invalid');
    await waitFor(() => expect(getByText(/JSON invalide/)).toBeInTheDocument());
  });

  it('rejette un tableau vide avec un message explicite', async () => {
    const { getByLabelText, getByText } = render(JsonPasteBuilder);
    await pasteAndParse(getByLabelText, getByText, '[]');
    await waitFor(() => expect(getByText(/tableau est vide/)).toBeInTheDocument());
  });
});
