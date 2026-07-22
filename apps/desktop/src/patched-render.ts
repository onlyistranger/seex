import type { AppState } from "./types";

/** Return true when one of the selected server fields changed. */
export function stateFieldsChanged<K extends keyof AppState>(
  previous: AppState | null,
  next: AppState,
  fields: readonly K[],
): boolean {
  return previous === null || fields.some((field) => !Object.is(previous[field], next[field]));
}

/** Compare the tuple lists used by history and matched clipboard entries. */
export function tupleListChanged(
  previous: readonly [string, string][] | null,
  next: readonly [string, string][],
): boolean {
  if (previous === null || previous.length !== next.length) return true;
  return next.some(
    ([first, second], index) => previous[index][0] !== first || previous[index][1] !== second,
  );
}

/** Build a compact signature for local page collections before replacing DOM. */
export function renderSignature(value: unknown): string {
  return JSON.stringify(value) ?? "";
}
