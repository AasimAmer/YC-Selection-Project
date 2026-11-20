import { authorizeRequest } from "./middleware.js";
import { normalizeToken } from "./cycle/normalize.js";

export function routeRequest(token: string): boolean {
  return authorizeRequest(normalizeToken(token));
}
