export function get(k) { try { return window.localStorage.getItem(k); } catch (e) { return null; } }
export function set(k, v) { window.localStorage.setItem(k, v); }