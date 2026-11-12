// The only module that talks to the Rust backend (AD-9). Types mirror the
// Rust DTOs in src-tauri/src/shell/commands.rs.
import { getName } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ItemKind = "text" | "html" | "image" | "files";

export interface ItemSummary {
  id: string;
  kind: ItemKind;
  /** Plain-text preview, at most 200 characters. */
  preview: string;
  sourceApp: string | null;
  /** Unix milliseconds UTC. */
  createdAt: number;
  /** Unix milliseconds UTC; the list is ordered by this, newest first. */
  lastCopiedAt: number;
  pinned: boolean;
}

export const DEFAULT_LIST_LIMIT = 500;

export function listItems(limit = DEFAULT_LIST_LIMIT): Promise<ItemSummary[]> {
  return invoke<ItemSummary[]>("list_items", { limit });
}

/** Why history is unavailable, or null if the database opened fine. */
export function startupError(): Promise<string | null> {
  return invoke<string | null>("startup_error");
}

export function hideWindow(): Promise<void> {
  return invoke("hide_window");
}

export function appName(): Promise<string> {
  return getName();
}

/** Fires after every change to history; refetch on it (AD-2). */
export function onHistoryChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("history-changed", () => cb());
}

/** Fires whenever the backend shows the window (AD-17). */
export function onWindowShown(cb: () => void): Promise<UnlistenFn> {
  return listen("window-shown", () => cb());
}
