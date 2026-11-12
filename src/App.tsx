import { useEffect, useRef, useState } from "react";
import { getName } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [appName, setAppName] = useState("");
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    getName().then(setAppName).catch(() => setAppName(""));
    // Re-focus the search box whenever the window is shown again.
    const onFocus = () => searchRef.current?.focus();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        invoke("hide_window").catch((err) => setError(String(err)));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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
      />
      {error && <div className="error">{error}</div>}
      <div className="empty">Nothing copied yet.</div>
    </main>
  );
}

export default App;
