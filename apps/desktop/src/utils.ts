// Small, stateless DOM/string helpers shared across every page module.
//
// These were originally inlined at the top of `main.ts`. They have no page
// affinity and no shared mutable state, so they live here as plain exports.
// Page modules import them directly; the root file re-exports thin shims only
// where existing call sites expected the old names.

import { translate } from "./i18n";

/** Look up an element by id and assert it exists (mirrors the old `$` shim). */
export function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

/** Escape a string for use inside HTML element text content. */
export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Escape a string for use inside an HTML attribute value. */
export function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/**
 * Promote `[title]` / `[data-tooltip]` elements to accessible tooltips under
 * `root` (default: whole document). Idempotent.
 */
export function applyTooltips(root: ParentNode = document): void {
  root.querySelectorAll<HTMLElement>("[title], [data-tooltip]").forEach((element) => {
    const message = element.dataset.tooltip || element.getAttribute("title");
    if (!message) return;

    element.dataset.tooltip = message;
    element.classList.add("has-tooltip");
    if (!element.getAttribute("title")) {
      element.setAttribute("title", message);
    }
    if (
      element.matches("button, select, input, label, [role=button]") &&
      !element.getAttribute("aria-label")
    ) {
      element.setAttribute("aria-label", message);
    }
  });
}

/** Translate `key`, then substitute `{name}` placeholders with `values`. */
export function formatMessage(key: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (message, [name, value]) => message.split(`{${name}}`).join(value),
    translate(key),
  );
}

/**
 * Keep an input in sync with server-provided values without clobbering a
 * user's in-flight local draft. Tracks the last synced value via a data attr.
 */
export function syncInputValue(id: string, serverValue: string): void {
  const input = $(id) as HTMLInputElement;
  const syncedValue = input.dataset.syncedValue;

  if (syncedValue === undefined) {
    input.value = serverValue;
    input.dataset.syncedValue = serverValue;
    return;
  }

  const hasLocalDraft = input.value !== syncedValue;
  if (!hasLocalDraft || input.value === serverValue) {
    input.value = serverValue;
    input.dataset.syncedValue = serverValue;
  }
}

/** Push a server value into a select element without flicker. */
export function syncSelectValue(id: string, serverValue: string): void {
  const select = $(id) as HTMLSelectElement;
  if (select.value !== serverValue) {
    select.value = serverValue;
  }
}

/** Parse a positive integer, falling back to `fallback` on invalid input. */
export function parsePositiveIntOrFallback(value: string, fallback: number): number {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) && parsed >= 1 ? parsed : fallback;
}

/** Validate and normalise an optional `#RRGGBB` / `#RRGGBBAA` colour string. */
export function parseOptionalHexColor(value: string): {
  normalized: string | null;
  valid: boolean;
} {
  const trimmed = value.trim();
  if (!trimmed) {
    return { normalized: null, valid: true };
  }

  const match = /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.exec(trimmed);
  if (!match) {
    return { normalized: null, valid: false };
  }

  return { normalized: `#${match[1].toUpperCase()}`, valid: true };
}
