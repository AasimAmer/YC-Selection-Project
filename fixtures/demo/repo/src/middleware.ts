import { validateSession } from "./session.js";

export function authorizeRequest(token: string): boolean {
  return validateSession(token);
}
