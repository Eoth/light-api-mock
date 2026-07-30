// Source unique de verite pour le formatage date/heure (meme principe que
// service-url.js/tpl-utils.js/path-params.js : cette logique ne doit jamais
// etre re-dupliquee dans un composant). Regroupe les 3 fidelites d'affichage
// deja utilisees dans le projet plutot que d'en imposer une seule, pour ne
// rien changer visuellement aux sites d'appel existants.
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

// Format utilise par RequestLog.svelte et MessagingLog.svelte (journaux).
export function formatDateTimePrecise(timestamp, locale = 'fr-FR') {
  return formatDateTime(timestamp, PRECISE_DATETIME_OPTIONS, locale);
}
