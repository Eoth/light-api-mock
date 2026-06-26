export const auth = $state({
  enabled: false,
  token: null,
  refreshToken: null,
  username: null,
  isSuperAdmin: false,
  groups: [],
});

export function isLoggedIn() {
  return !auth.enabled || auth.token !== null;
}

export function setAuth(data) {
  auth.token = data.access_token;
  auth.refreshToken = data.refresh_token || null;
  auth.username = data.username;
  auth.isSuperAdmin = data.is_super_admin;
  persistAuth();
}

export function logout() {
  auth.token = null;
  auth.refreshToken = null;
  auth.username = null;
  auth.isSuperAdmin = false;
  auth.groups = [];
  localStorage.removeItem('lightmock-auth');
}

export function persistAuth() {
  localStorage.setItem('lightmock-auth', JSON.stringify({
    token: auth.token,
    refreshToken: auth.refreshToken,
    username: auth.username,
    isSuperAdmin: auth.isSuperAdmin,
  }));
}

export function restoreAuth() {
  try {
    const saved = localStorage.getItem('lightmock-auth');
    if (saved) {
      const data = JSON.parse(saved);
      auth.token = data.token || null;
      auth.refreshToken = data.refreshToken || null;
      auth.username = data.username || null;
      auth.isSuperAdmin = data.isSuperAdmin || false;
    }
  } catch {
    // corrupted data, ignore
  }
}
