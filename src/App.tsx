import { useCallback, useEffect, useRef, useState } from "react";
import {
  accessibilityStatus,
  appName as fetchAppName,
  hideWindow,
  listItems,
  onHistoryChanged,
  onWindowShown,
  pasteItem,
  returnToPreviousApp,
  startupError,
  type AccessibilityStatus,
  type ItemSummary,
} from "./api";
import { HistoryList, LISTBOX_ID, optionId } from "./components/HistoryList";
import { PermissionPrompt } from "./components/PermissionPrompt";
import "./App.css";

/** How long the inline "copied" notice stays before focus goes back. */
const NOTICE_MS = 1500;

interface Notice {
  text: string;
  /** Offer to turn on automatic paste (macOS, permission missing). */
  offerPermission: boolean;
}

function copiedNotice(perm: AccessibilityStatus | null): string {
  // `applies` is true on macOS only.
  return `Copied — press ${perm?.applies ? "⌘V" : "Ctrl+V"} to paste`;
}

function App() {
  const [appName, setAppName] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemSummary[]>([]);
  // Selection is kept by id, not index, so a new capture landing at the top
  // does not move it (AD-17).
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [fatal, setFatal] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Paste failures, kept apart from `error`, which every refresh clears.
  const [pasteError, setPasteError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [perm, setPerm] = useState<AccessibilityStatus | null>(null);
  const [showPermission, setShowPermission] = useState(false);
  // After a paste that hid the window but could not paste, the notice waits
  // here until the next time the window is shown.
  const pendingNotice = useRef<Notice | null>(null);
  // Pending "return to the previous app" after an inline notice.
  const returnTimer = useRef<number | null>(null);
  const pasting = useRef(false);
  const searchRef = useRef<HTMLInputElement>(null);
  // Only the latest refresh may apply its result; older ones can resolve
  // later and would overwrite a newer list.
  const refreshSeq = useRef(0);

  const refresh = useCallback(async (resetSelection: boolean) => {
    const seq = ++refreshSeq.current;
    try {
      const list = await listItems();
      if (seq !== refreshSeq.current) return;
      setItems(list);
      setNow(Date.now());
      setSelectedId((prev) =>
        resetSelection || prev === null || !list.some((i) => i.id === prev)
          ? (list[0]?.id ?? null)
          : prev,
      );
      setError(null);
    } catch (err) {
      if (seq !== refreshSeq.current) return;
      setError(String(err));
    }
  }, []);

  const checkPermission = useCallback(async () => {
    try {
      const s = await accessibilityStatus();
      setPerm(s);
      setShowPermission(s.applies && !s.trusted && !s.asked);
    } catch (err) {
      console.error(err);
    }
  }, []);

  const cancelReturn = useCallback(() => {
    if (returnTimer.current !== null) {
      window.clearTimeout(returnTimer.current);
      returnTimer.current = null;
    }
  }, []);

  const paste = useCallback(
    async (id: string) => {
      if (pasting.current) return;
      pasting.current = true;
      cancelReturn();
      setPasteError(null);
      setNotice(null);
      try {
        const out = await pasteItem(id);
        if (out.outcome !== "copiedOnly") return;
        if (out.reason === "notTrusted" || out.reason === "noTarget") {
          // The window is still open: say so now, then give focus back.
          setNotice({
            text:
              out.reason === "noTarget"
                ? "Copied — paste it where you need it"
                : copiedNotice(perm),
            offerPermission: !!perm?.applies && out.reason === "notTrusted",
          });
          returnTimer.current = window.setTimeout(() => {
            returnTimer.current = null;
            returnToPreviousApp().catch((err) => setError(String(err)));
          }, NOTICE_MS);
        } else {
          // The window was hidden before the paste failed.
          pendingNotice.current = { text: copiedNotice(perm), offerPermission: false };
        }
      } catch (err) {
        // The item is gone or the clipboard failed; the window stays open.
        setPasteError(String(err));
      } finally {
        pasting.current = false;
      }
    },
    [perm, cancelReturn],
  );

  // Keep relative times ("just now") fresh while the window stays open.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    fetchAppName().then(setAppName).catch(() => setAppName(""));
    startupError()
      .then((e) => {
        setFatal(e);
        if (!e) void refresh(true);
      })
      .catch((err) => setError(String(err)));

    void checkPermission();

    const unlisteners = [
      onHistoryChanged(() => void refresh(false)),
      onWindowShown(() => {
        cancelReturn();
        setNotice(pendingNotice.current);
        pendingNotice.current = null;
        setPasteError(null);
        void checkPermission();
        setQuery("");
        searchRef.current?.focus();
        void refresh(true);
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((off) => off()).catch(() => {}));
    };
  }, [refresh, checkPermission, cancelReturn]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelReturn();
        hideWindow().catch((err) => setError(String(err)));
        return;
      }
      if (e.key === "Enter") {
        // Let buttons (permission prompt, notice) handle their own Enter.
        if (e.target instanceof HTMLButtonElement) return;
        if (e.isComposing || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
        e.preventDefault();
        if (selectedId) void paste(selectedId);
        return;
      }
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      if (e.isComposing || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
      e.preventDefault();
      if (items.length === 0) return;
      const idx = items.findIndex((i) => i.id === selectedId);
      const next =
        e.key === "ArrowDown"
          ? Math.min(idx + 1, items.length - 1)
          : Math.max(idx - 1, 0);
      setSelectedId(items[next].id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items, selectedId, paste, cancelReturn]);

  return (
    <main className="shell">
      <input
        ref={searchRef}
        className="search"
        type="text"
        placeholder={`Search ${appName} history`}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoFocus
        spellCheck={false}
        role="combobox"
        aria-expanded={items.length > 0}
        aria-controls={LISTBOX_ID}
        aria-activedescendant={selectedId ? optionId(selectedId) : undefined}
      />
      {fatal && (
        <div className="error" role="alert">
          {fatal}
        </div>
      )}
      {error && !fatal && (
        <div className="error" role="alert">
          {error}
        </div>
      )}
      {pasteError && (
        <div className="error" role="alert">
          {pasteError}
        </div>
      )}
      {notice && (
        <div className="notice" role="status">
          <span>{notice.text}</span>
          {notice.offerPermission && !showPermission && (
            <button
              type="button"
              className="link"
              onClick={() => {
                // Stay open so the user can read the explainer.
                cancelReturn();
                setShowPermission(true);
              }}
            >
              Paste automatically…
            </button>
          )}
        </div>
      )}
      {showPermission && (
        <PermissionPrompt
          appName={appName}
          asked={!!perm?.asked}
          onDone={() => {
            setShowPermission(false);
            void checkPermission();
            searchRef.current?.focus();
          }}
        />
      )}
      {items.length > 0 ? (
        <HistoryList
          items={items}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onActivate={(id) => void paste(id)}
          now={now}
        />
      ) : (
        !fatal && <div className="empty">Nothing copied yet.</div>
      )}
    </main>
  );
}

export default App;
