import type { Transition, Variants } from "motion/react";

import type { AnimationLevel } from "@/lib/theme";

export const spring: Transition = { type: "spring", stiffness: 380, damping: 32, mass: 0.8 };
export const easeOut: Transition = { duration: 0.28, ease: [0.22, 1, 0.36, 1] };

export const fadeUp: Variants = {
  hidden: { opacity: 0, y: 12 },
  show: { opacity: 1, y: 0, transition: easeOut },
};

export const scaleIn: Variants = {
  hidden: { opacity: 0, scale: 0.96 },
  show: { opacity: 1, scale: 1, transition: easeOut },
};

export const stagger: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.045, delayChildren: 0.05 } },
};

export const pageTransition: Variants = {
  initial: { opacity: 0, y: 10, filter: "blur(4px)" },
  enter: { opacity: 1, y: 0, filter: "blur(0px)", transition: { duration: 0.32, ease: [0.22, 1, 0.36, 1] } },
  exit: { opacity: 0, y: -6, transition: { duration: 0.15 } },
};

/** Maps the user's animation preference onto Motion's `reducedMotion` setting. */
export function reducedMotionFor(level: AnimationLevel): "never" | "always" | "user" {
  return level === "full" ? "user" : "always";
}
