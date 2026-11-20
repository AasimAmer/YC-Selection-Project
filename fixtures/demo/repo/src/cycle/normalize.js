import { stripPrefix } from "./prefix.js";

export function normalizeToken(token) {
  return stripPrefix(token.trim());
}
