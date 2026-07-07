/**
 * Reduce any rejected value from `invoke` to a human-readable string:
 *
 * - `string` — the common case (`Result<_, String>` on the BE).
 * - `Error` — defensive; if a Tauri JS layer ever wraps rejection in an Error.
 * - `{ message: string }` — defensive; if the BE returns a structured error.
 * - fallback — `String(e)`, which at worst yields `"[object Object]"` (still
 *   better than throwing an opaque value).
 *
 * Use this from store actions to rethrow a standardized {@link InvokeError};
 * use {@link invokeErrorMessage} from onSubmit handlers to derive a description.
 */
export function formatInvokeError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (typeof e === 'object' && e !== null && 'message' in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return String(e);
}
