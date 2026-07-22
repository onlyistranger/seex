// Thin, typed layer over Tauri IPC commands.
//
// The desktop frontend historically scattered 50+ raw `invoke("...")` calls
// across `main.ts`, each followed by an ad-hoc `await refreshState()` and
// hand-rolled `errorMessage()` conversion. This module centralises the shared
// pieces without (yet) rewriting every call site:
//
// - `errorMessage(error)` — the single error→string helper used everywhere.
// - `invokeState()` — typed `get_state` fetch returning the `AppState` mirror.
// - `withRefresh(refresh, operation)` — runs an async operation, then calls
//   `refresh()` regardless of success/failure, mirroring the
//   `await invoke(...); await refreshState();` boilerplate. The refresh
//   callback is injected so this module doesn't need to know how the root
//   renders state.
//
// Future page modules (Phase 2) should prefer these helpers over raw
// `invoke`. Call-site migration happens incrementally per page; nothing here
// changes the IPC contract or command/payload names.

import { invoke } from "@tauri-apps/api/core";

import type { AppState } from "./types";

/** Convert a thrown value into a user-facing string (used by all error paths). */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/** Fetch the full backend state projection. */
export function invokeState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}

/**
 * Run `operation`, then refresh backend state via `refresh`, even if the
 * operation throws. The original error is re-thrown after refreshing so callers
 * can still surface it. This mirrors the pervasive
 * `await invoke(...); await refreshState();` pattern in `main.ts` while making
 * the "always refresh" contract explicit.
 */
export async function withRefresh<T>(
  refresh: () => Promise<void>,
  operation: () => Promise<T>,
): Promise<T> {
  try {
    const result = await operation();
    await refresh();
    return result;
  } catch (error) {
    await refresh().catch(() => {
      // Ignore refresh errors; the original operation error is more useful.
    });
    throw error;
  }
}
