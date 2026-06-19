import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ToggleSwitch from '../lib/components/ToggleSwitch.svelte';

describe('ToggleSwitch', () => {
  it('affiche le label', () => {
    const { getByText } = render(ToggleSwitch, { props: { label: 'Mock service-a', checked: false } });
    expect(getByText('Mock service-a')).toBeInTheDocument();
  });

  it('affiche OFF quand desactive', () => {
    const { getByText } = render(ToggleSwitch, { props: { label: 'Test', checked: false } });
    expect(getByText('OFF')).toBeInTheDocument();
  });

  it('affiche ON quand active', () => {
    const { getByText } = render(ToggleSwitch, { props: { label: 'Test', checked: true } });
    expect(getByText('ON')).toBeInTheDocument();
  });

  it('a le role switch avec aria-checked', () => {
    const { getByRole } = render(ToggleSwitch, { props: { label: 'Test', checked: true } });
    const btn = getByRole('switch');
    expect(btn).toHaveAttribute('aria-checked', 'true');
  });

  it('appelle onchange au clic', async () => {
    const onchange = vi.fn();
    const { getByRole } = render(ToggleSwitch, { props: { label: 'Test', checked: false, onchange } });
    await fireEvent.click(getByRole('switch'));
    expect(onchange).toHaveBeenCalledWith(true);
  });

  it('ne declenche pas onchange si disabled', async () => {
    const onchange = vi.fn();
    const { getByRole } = render(ToggleSwitch, { props: { label: 'Test', checked: false, disabled: true, onchange } });
    await fireEvent.click(getByRole('switch'));
    expect(onchange).not.toHaveBeenCalled();
  });
});
