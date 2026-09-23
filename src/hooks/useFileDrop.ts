import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * Files dropped from Explorer onto the window. Returns whether files are
 * currently dragged over it, to show a drop overlay.
 */
export function useFileDrop(onDrop: (paths: string[]) => void, enabled = true): boolean {
  const [hovering, setHovering] = useState(false);
  const handler = useRef(onDrop);

  useEffect(() => {
    handler.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    if (!enabled) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const { type } = event.payload;
        if (type === "enter" || type === "over") setHovering(true);
        else if (type === "drop") {
          setHovering(false);
          handler.current(event.payload.paths);
        } else setHovering(false);
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {
        // Not running inside Tauri (tests, plain browser preview).
      });
    return () => {
      disposed = true;
      unlisten?.();
      setHovering(false);
    };
  }, [enabled]);

  return hovering;
}
