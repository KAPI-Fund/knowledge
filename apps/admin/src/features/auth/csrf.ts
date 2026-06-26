const CSRF_STORAGE_KEY = "knowledge.csrfToken";

// Stored in localStorage, not sessionStorage: the session cookie is shared
// across tabs and survives tab close, so the CSRF token must outlive a single
// tab too. A per-tab sessionStorage token leaves new tabs authenticated by the
// cookie but unable to mutate, producing "invalid csrf token".
export function getCsrfToken(): string {
  return window.localStorage.getItem(CSRF_STORAGE_KEY) ?? "";
}

export function setCsrfToken(token: string): void {
  window.localStorage.setItem(CSRF_STORAGE_KEY, token);
}
