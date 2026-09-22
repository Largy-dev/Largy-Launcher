import { useEffect, useRef } from "react";
import { animate, useInView } from "motion/react";

interface AnimatedNumberProps {
  value: number;
  format?: (value: number) => string;
  className?: string;
}

const defaultFormat = (v: number) => Math.round(v).toLocaleString("fr-FR");

/** Counts up to `value` the first time it scrolls into view, then follows changes. */
export function AnimatedNumber({ value, format = defaultFormat, className }: AnimatedNumberProps) {
  const ref = useRef<HTMLSpanElement>(null);
  const from = useRef(0);
  const inView = useInView(ref, { once: true });

  useEffect(() => {
    if (!inView || !ref.current) return;
    const node = ref.current;
    const controls = animate(from.current, value, {
      duration: 0.9,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (v) => {
        node.textContent = format(v);
      },
    });
    from.current = value;
    return () => controls.stop();
  }, [value, inView, format]);

  return (
    <span ref={ref} className={className}>
      {format(0)}
    </span>
  );
}
