import { useCallback, useEffect, useRef, useState } from "react";
import {
  appName as fetchAppName,
  hideWindow,
  listItems,
  onHistoryChanged,
  onWindowShown,
  startupError,
  type ItemSummary,
} from "./api";
import { HistoryList, LISTBOX_ID, optionId } from "./components/HistoryList";
import "./App.css";

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

    const unlisteners = [
      onHistoryChanged(() => void refresh(false)),
      onWindowShown(() => {
        setQuery("");
        searchRef.current?.focus();
        void refresh(true);
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((off) => off()).catch(() => {}));
    };
  }, [refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        hideWindow().catch((err) => setError(String(err)));
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
  }, [items, selectedId]);

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
      {items.length > 0 ? (
        <HistoryList
          items={items}
          selectedId={selectedId}
          onSelect={setSelectedId}
          now={now}
        />
      ) : (
        !fatal && <div className="empty">Nothing copied yet.</div>
      )}
    </main>
  );
}

export default App;
