import { render, waitFor, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MessagingLog from '../lib/components/MessagingLog.svelte';
import { getMessagingLogs, simulateMessage } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  getMessagingLogs: vi.fn(),
  simulateMessage: vi.fn(),
}));

describe('MessagingLog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('affiche un message matche avec son service/regle', async () => {
    getMessagingLogs.mockResolvedValue([
      {
        timestamp: Date.now(), direction: 'in', topic: 'orders.in',
        service_name: 'svc-a', rule_matched: 'rule-1', matched: true,
        body_preview: '{"type":"order.created"}', body_truncated: false, body_size_bytes: 25,
      },
    ]);
    const { getByText } = render(MessagingLog);
    await waitFor(() => expect(getByText('orders.in')).toBeInTheDocument());
    expect(getByText('svc-a / rule-1')).toBeInTheDocument();
    expect(getByText('Matche')).toBeInTheDocument();
  });

  it('affiche un badge "Tronque" quand le corps a ete tronque', async () => {
    getMessagingLogs.mockResolvedValue([
      {
        timestamp: Date.now(), direction: 'in', topic: 'orders.in',
        service_name: null, rule_matched: null, matched: false,
        body_preview: 'x'.repeat(16384), body_truncated: true, body_size_bytes: 50000,
      },
    ]);
    const { getByText } = render(MessagingLog);
    await waitFor(() => expect(getByText('Tronque')).toBeInTheDocument());
    expect(getByText('Non matche')).toBeInTheDocument();
  });

  it('n\'affiche pas de badge "Tronque" quand le corps n\'est pas tronque', async () => {
    getMessagingLogs.mockResolvedValue([
      {
        timestamp: Date.now(), direction: 'out', topic: 'orders.reply',
        service_name: 'svc-a', rule_matched: 'rule-1', matched: true,
        body_preview: 'short', body_truncated: false, body_size_bytes: 5,
      },
    ]);
    const { getByText, queryByText } = render(MessagingLog);
    await waitFor(() => expect(getByText('orders.reply')).toBeInTheDocument());
    expect(queryByText('Tronque')).not.toBeInTheDocument();
  });

  it('affiche l\'etat vide quand aucun message n\'est journalise', async () => {
    getMessagingLogs.mockResolvedValue([]);
    const { getByText } = render(MessagingLog);
    await waitFor(() => expect(getByText('Aucun message Kafka traite pour le moment.')).toBeInTheDocument());
  });

  it('envoie une simulation via le formulaire et rafraichit le journal', async () => {
    getMessagingLogs.mockResolvedValue([]);
    simulateMessage.mockResolvedValue(null);
    const onNotify = vi.fn();
    const { getByLabelText, getByText, container } = render(MessagingLog, { onNotify });
    await waitFor(() => expect(getMessagingLogs).toHaveBeenCalledTimes(1));

    const topicInput = getByLabelText('Topic du message simule');
    await fireEvent.input(topicInput, { target: { value: 'orders.in' } });

    await fireEvent.click(getByText('Simuler'));

    await waitFor(() => expect(simulateMessage).toHaveBeenCalledWith('orders.in', expect.any(String)));
    await waitFor(() => expect(getMessagingLogs).toHaveBeenCalledTimes(2));
    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('orders.in'), 'success');
  });

  it('refuse de simuler sans topic renseigne', async () => {
    getMessagingLogs.mockResolvedValue([]);
    const onNotify = vi.fn();
    const { getByText } = render(MessagingLog, { onNotify });
    await waitFor(() => expect(getMessagingLogs).toHaveBeenCalled());

    await fireEvent.click(getByText('Simuler'));

    expect(simulateMessage).not.toHaveBeenCalled();
    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('Le topic est requis'), 'error');
  });
});
