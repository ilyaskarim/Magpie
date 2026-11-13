import { useState } from "react";
import {
  dismissAccessibilityPrompt,
  openAccessibilitySettings,
  requestAccessibility,
} from "../api";

interface Props {
  appName: string;
  /** The system prompt was already shown once; open the pane instead. */
  asked: boolean;
  /** Called once the user has chosen; the prompt then goes away. */
  onDone: () => void;
}

/**
 * First-run explainer for the macOS Accessibility permission, which Magpie
 * needs to paste into other apps. Without it, items are still copied.
 */
export function PermissionPrompt({ appName, asked, onDone }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const name = appName || "This app";

  const open = async () => {
    setBusy(true);
    setError(null);
    try {
      // The system prompt (which has its own "Open System Settings" button)
      // the first time; macOS shows it only once, so afterwards go straight
      // to the pane.
      if (asked) {
        await openAccessibilitySettings();
      } else {
        await requestAccessibility();
      }
      onDone();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const later = async () => {
    setError(null);
    try {
      await dismissAccessibilityPrompt();
    } catch (err) {
      // Not being able to remember the choice is not worth blocking on.
      console.error(err);
    }
    onDone();
  };

  return (
    <section className="permission" aria-labelledby="permission-title">
      <h2 id="permission-title">Paste with one key</h2>
      <p>
        To paste an item straight into the app you were using, {name} needs
        the Accessibility permission. Without it, Enter copies the item and you
        press ⌘V yourself.
      </p>
      {error && (
        <p className="permission-error" role="alert">
          {error}
        </p>
      )}
      <div className="permission-actions">
        <button type="button" className="primary" onClick={open} disabled={busy}>
          Open Accessibility settings
        </button>
        <button type="button" onClick={later} disabled={busy}>
          Not now
        </button>
      </div>
    </section>
  );
}
