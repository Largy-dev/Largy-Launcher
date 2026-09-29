import { createContext, useContext } from "react";

/** The element pages scroll in (the app shell's `<main>`), provided by the shell. */
export const ScrollContainerContext = createContext<HTMLElement | null>(null);

/** For virtualised lists that scroll with the page. */
export function useScrollContainer(): HTMLElement | null {
  return useContext(ScrollContainerContext);
}
