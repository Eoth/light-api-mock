import { intlLocale } from './i18n.svelte.js';

// Dates and times as the UI shows them; no component formats one on its own. Screens need different precision, so
// `options` are those of toLocaleString: none gives the locale's usual form (backups, rule tester).
export function formatDateTime(timestamp, options, locale) {
  return new Date(timestamp).toLocaleString(locale, options);
}

const PRECISE_DATETIME_OPTIONS = {
  day: '2-digit',
  month: '2-digit',
  year: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
};

// Down to the second, for the logs (RequestLog.svelte, MessagingLog.svelte).
export function formatDateTimePrecise(timestamp, locale = intlLocale()) {
  return formatDateTime(timestamp, PRECISE_DATETIME_OPTIONS, locale);
}
