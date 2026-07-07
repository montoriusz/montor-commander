/**
 * Alias suggestion + sanitization shared by the providers form and its yup
 * schema so the placeholder shown in the UI and the value the FE validates
 * against (when a provider's `alias` is empty but `name` is set) stay in
 * lock-step with the backend's `is_valid_alias` grammar.
 */

/**
 * Sanitize a free-form name into an alias matching the grammar enforced by the
 * FE yup schema and the BE `is_valid_alias`: lowercase alphanumeric with `_`/
 * `-` allowed only inside (not at ends). Runs of non-`[a-z0-9_-]` collapse to a
 * single `_`, and leading/trailing `_`/`-` are stripped.
 *
 * Returns `''` for empty or all-symbol input, so callers can treat a falsy
 * result as "no suggestion".
 */
export function sanitizeAlias(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '_')
    .replace(/^[_-]+|[_-]+$/g, '');
}
