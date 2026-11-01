import { useLayoutEffect, useRef } from "react";
import { DOCS_URL, REPO_URL } from "../data";
import { gsap, MQ } from "../lib/motion";

export function Closing() {
  const root = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const mm = gsap.matchMedia();
    mm.add(MQ.motion, () => {
      const tl = gsap.timeline({ scrollTrigger: { trigger: el, start: "top 60%", once: true } });
      tl.from(".mark", { scale: 0.7, autoAlpha: 0, duration: 1, ease: "power3.out" })
        .from(".close h2 span", { yPercent: 40, autoAlpha: 0, duration: 0.9, stagger: 0.18, ease: "power3.out" }, 0.2)
        .from(".close .cta > *", { autoAlpha: 0, y: 16, duration: 0.6, stagger: 0.1 }, 0.8);
    });
    return () => mm.revert();
  }, []);

  return (
    <section id="closing" ref={root} className="chapter close" aria-label="Closing">
      <img className="mark" src="/brand/symbol.svg" alt="" width="160" height="160" />
      <h2>
        <span>Explore the architecture.</span>
        <span>Understand the language.</span>
        <span>Build beyond boundaries.</span>
      </h2>
      <div className="cta">
        <a className="btn primary" href={REPO_URL} rel="noopener">GitHub repository</a>
        <a className="btn" href={DOCS_URL} rel="noopener">Documentation</a>
      </div>
    </section>
  );
}
