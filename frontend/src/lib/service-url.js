// The URL to call a service, built here only, so that the list (ServiceCard.svelte), the page of a service
// (ServiceDetail.svelte) and its form (ServiceForm.svelte) show the same one: prefixed with the short code of the
// service's group when it has one.
export function buildServiceTestUrl({ name, listenPath = '', groupCode = '', baseUrl = '' }) {
  const n = (name || '').trim() || '...';
  const prefix = groupCode ? `/${groupCode}/${n}` : `/${n}`;
  const p = (listenPath || '').trim();
  const path = p ? (p.startsWith('/') ? p : '/' + p) : '/*';
  return `${baseUrl}${prefix}${path}`;
}
