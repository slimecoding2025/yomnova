import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { SplitText } from "gsap/SplitText";
import { useLayoutEffect, useRef, useState, type RefObject } from "react";

// Register plugins exactly once, at module scope.
gsap.registerPlugin(ScrollTrigger, SplitText);
ScrollTrigger.config({ ignoreMobileResize: true });

export { gsap, ScrollTrigger, SplitText };

/** Media queries used by every chapter. Reduced motion always wins. */
export const MQ = {
  desktop: "(min-width: 900px) and (prefers-reduced-motion: no-preference)",
  mobile: "(max-width: 899px) and (prefers-reduced-motion: no-preference)",
  motion: "(prefers-reduced-motion: no-preference)",
} as const;

interface PinnedStepsOptions {
  /** Scroll distance per step, as a percentage of viewport height. */
  perStep?: number;
  /** Called on every scroll update with the section's pinned progress (0..1). Not a React state. */
  onProgress?: (progress: number) => void;
  /** Called once when the pinned mode starts; may return a cleanup. */
  onPin?: (section: HTMLElement) => void | (() => void);
}

/**
 * Pins a chapter on desktop (when motion is allowed) and maps scroll progress to a discrete
 * `active` step plus a continuous `onProgress` callback. On mobile and under reduced motion the
 * chapter is left as a normal, fully visible, scrolling section (`pinned === false`).
 */
export function usePinnedSteps(ref: RefObject<HTMLElement | null>, count: number, opts: PinnedStepsOptions = {}) {
  const [active, setActive] = useState(0);
  const [pinned, setPinned] = useState(false);
  const cb = useRef(opts);
  cb.current = opts;

  useLayoutEffect(() => {
    const section = ref.current;
    if (!section) return;
    const mm = gsap.matchMedia();
    mm.add(MQ.desktop, () => {
      setPinned(true);
      const cleanupPin = cb.current.onPin?.(section);
      let last = -1;
      ScrollTrigger.create({
        trigger: section,
        start: "top top",
        end: `+=${count * (cb.current.perStep ?? 60)}%`,
        pin: true,
        anticipatePin: 1,
        invalidateOnRefresh: true,
        onUpdate: (self) => {
          const idx = Math.min(count - 1, Math.floor(self.progress * count));
          if (idx !== last) {
            last = idx;
            setActive(idx);
          }
          cb.current.onProgress?.(self.progress);
        },
      });
      return () => {
        setPinned(false);
        setActive(0);
        if (typeof cleanupPin === "function") cleanupPin();
      };
    });
    return () => mm.revert();
  }, [ref, count]);

  return { active, pinned };
}

/** Fade/slide-in for any element marked `.rv` inside the section (motion allowed only). */
export function useReveal(ref: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const mm = gsap.matchMedia();
    mm.add(MQ.motion, () => {
      const items = root.querySelectorAll<HTMLElement>(".rv");
      if (!items.length) return;
      gsap.set(items, { autoAlpha: 0, y: 28 });
      ScrollTrigger.batch(items, {
        start: "top 88%",
        once: true,
        onEnter: (els) =>
          gsap.to(els, { autoAlpha: 1, y: 0, duration: 0.9, ease: "power3.out", stagger: 0.09, overwrite: true }),
      });
    });
    return () => mm.revert();
  }, [ref]);
}
