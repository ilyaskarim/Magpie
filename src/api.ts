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

export type CopiedOnlyReason =
  | "notTrusted"
  | "noTarget"
  | "targetUnavailable"
  | "pasteFailed";

/** Mirrors `PasteOutcomeDto`. */
export type PasteOutcome =
  | { outcome: "pasted" }
  | { outcome: "copiedOnly"; reason: CopiedOnlyReason };

/**
 * Copies the item, hides the window and pastes it into the app that was
 * frontmost when the window opened. Rejects with a readable message (and the
 * window stays open) if the item is gone.
 */
export function pasteItem(id: string): Promise<PasteOutcome> {
  return invoke<PasteOutcome>("paste_item", { id });
}

export interface AccessibilityStatus {
  /** This platform has a paste permission (macOS Accessibility). */
  applies: boolean;
  /** Magpie may send the synthetic paste keystroke. */
  trusted: boolean;
  /** The first-run explainer was already answered. */
  asked: boolean;
}

export function accessibilityStatus(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("accessibility_status");
}

/** Shows the system prompt; resolves to whether Magpie is trusted now. */
export function requestAccessibility(): Promise<boolean> {
  return invoke<boolean>("request_accessibility");
}

export function openAccessibilitySettings(): Promise<void> {
  return invoke("open_accessibility_settings");
}

/**
 * After a copy-only paste: hides the window and gives focus back to the app
 * that was frontmost when it opened.
 */
export function returnToPreviousApp(): Promise<void> {
  return invoke("return_to_previous_app");
}

/** "Not now": do not show the explainer again. */
export function dismissAccessibilityPrompt(): Promise<void> {
  return invoke("dismiss_accessibility_prompt");
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
