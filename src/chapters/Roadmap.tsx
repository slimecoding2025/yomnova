import { useLayoutEffect, useRef } from "react";
import { ChapterTag, StatusChip } from "../components/Common";
import { DOCS_URL, ROADMAP, type Status } from "../data";
import { gsap, MQ, useReveal } from "../lib/motion";

const ORDER: Status[] = ["implemented", "partial", "planned", "research"];

export function Roadmap() {
  const root = useRef<HTMLElement>(null);
  useReveal(root);

  // Columns sweep in left to right while scrolling (scrubbed, not pinned).
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const mm = gsap.matchMedia();
    mm.add(MQ.desktop, () => {
      gsap.fromTo(
        el.querySelectorAll(".road-col li"),
        { xPercent: -6, opacity: 0.25 },
        { xPercent: 0, opacity: 1, stagger: 0.04, ease: "none", scrollTrigger: { trigger: ".road-cols", start: "top 85%", end: "bottom 60%", scrub: 0.6 } },
      );
    });
    return () => mm.revert();
  }, []);

  return (
    <section id="roadmap" ref={root} className="chapter road" aria-label="Roadmap">
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={9} title="Roadmap" />
      <h2 className="display rv">What exists. What doesn&apos;t.</h2>
      <div className="road-cols">
        {ORDER.map((s) => (
          <div key={s} className="road-col">
            <h3><StatusChip status={s} /></h3>
            <ul>
              {ROADMAP[s].map((it) => (
                <li key={it.title}>
                  {it.title}
                  {it.note && <small>{it.note}</small>}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      <p className="road-link">
        Detailed, dated status lives in <a href={`${DOCS_URL}/PROGRESS.md`}>docs/PROGRESS.md</a> and{" "}
        <a href={`${DOCS_URL}/ROADMAP.md`}>docs/ROADMAP.md</a>.
      </p>
    </section>
  );
}
