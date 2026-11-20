import { validateSession as validateCurrent } from "./session.js";
import { validateSession as validateLegacy } from "./legacy/compatibility-for-checkout-integrations/historical-session-adapters-for-regional-storefronts/archive-of-client-facing-authentication-and-session-migrations/maintained-implementations-for-offline-and-international-checkout/以前のセッション検証の実装と統合/旧セッション.js";

export function runValidation(token, legacy, adapters) {
  const validate = legacy ? validateLegacy : validateCurrent;
  return validate(token) && adapters.current(token);
}
