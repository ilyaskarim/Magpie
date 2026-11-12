import { useEffect, useRef } from "react";
import type { ItemSummary } from "../api";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(ms: number, now: number): string {
  const d = Math.max(0, now - ms);
  if (d < MINUTE) return "just now";
  if (d < HOUR) return `${Math.floor(d / MINUTE)} min ago`;
  if (d < DAY) return `${Math.floor(d / HOUR)} h ago`;
  if (d < 7 * DAY) return `${Math.floor(d / DAY)} d ago`;
  return new Date(ms).toLocaleDateString();
}

export const LISTBOX_ID = "history-list";

/** The DOM id of an item's option, for `aria-activedescendant`. */
export function optionId(itemId: string): string {
  return `item-${itemId}`;
}

interface Props {
  items: ItemSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** Click: paste the item (same as Enter). */
  onActivate: (id: string) => void;
  now: number;
}

export function HistoryList({ items, selectedId, onSelect, onActivate, now }: Props) {
  const selectedRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: "nearest" });
    // `items` too: on reopen the selection may reset to the same first id
    // while the list is scrolled down.
  }, [selectedId, items]);

  return (
    <ul
      id={LISTBOX_ID}
      className="history"
      role="listbox"
      aria-label="Clipboard history"
    >
      {items.map((item) => {
        const selected = item.id === selectedId;
        return (
          <li
            key={item.id}
            id={optionId(item.id)}
            ref={selected ? selectedRef : undefined}
            className={selected ? "item selected" : "item"}
            role="option"
            aria-selected={selected}
            onMouseDown={(e) => {
              e.preventDefault(); // keep focus in the search box
              onSelect(item.id);
            }}
            onClick={() => onActivate(item.id)}
          >
            <span className="preview">{item.preview}</span>
            <span className="meta">
              {item.sourceApp && <span className="source">{item.sourceApp}</span>}
              <time dateTime={new Date(item.lastCopiedAt).toISOString()}>
                {relativeTime(item.lastCopiedAt, now)}
              </time>
            </span>
          </li>
        );
      })}
    </ul>
  );
}
