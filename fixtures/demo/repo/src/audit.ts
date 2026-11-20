import { validateSession } from "./session.js";

export function auditSession(token: string): string {
  return validateSession(token) ? "session-present" : "session-missing";
}
