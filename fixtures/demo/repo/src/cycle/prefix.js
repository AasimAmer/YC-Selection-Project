import { normalizeToken } from "./normalize.js";

export function stripPrefix(token) {
  return token.startsWith("Bearer ") ? normalizeToken(token.slice(7)) : token;
}
