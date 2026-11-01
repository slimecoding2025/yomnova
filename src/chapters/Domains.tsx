import { useLayoutEffect, useRef } from "react";
import { ChapterTag, StatusChip } from "../components/Common";
import { DOMAINS } from "../data";
import { gsap, usePinnedSteps, useReveal } from "../lib/motion";

export function Domains() {
  const root = useRef<HTMLElement>(null);
  const { active, pinned } = usePinnedSteps(root, DOMAINS.length, { perStep: 45 });
  useReveal(root);
  const d = DOMAINS[active];

  // Animate the detail panel whenever the active domain changes.
  useLayoutEffect(() => {
    if (!pinned) return;
    const ctx = gsap.context(() => {
      gsap.fromTo(".dom-panel > *", { autoAlpha: 0, y: 18 }, { autoAlpha: 1, y: 0, duration: 0.5, stagger: 0.06, ease: "power3.out" });
    }, root);
    return () => ctx.revert();
  }, [active, pinned]);

  return (
    <section id="domains" ref={root} className="chapter domains" aria-label="One language, multiple domains" data-mode={pinned ? "pinned" : undefined}>
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={3} title="One language · multiple domains" />
      <h2 className="display rv">Ten domains. One direction.</h2>
      <div className="domains-layout">
        <ol className="dom-list">
          {DOMAINS.map((dom, i) => (
            <li key={dom.name} className={`dom-row rv${pinned && i === active ? " on" : ""}`} aria-current={pinned && i === active ? "true" : undefined}>
              <span className="n">{String(i + 1).padStart(2, "0")}</span>
              <span className="t">{dom.name}</span>
              <StatusChip status={dom.status} />
              <div className="detail">
                <span><b>Today</b>{dom.today}</span>
                <span><b>Next</b>{dom.next}</span>
              </div>
            </li>
          ))}
        </ol>
        <div className="dom-panel" aria-live="polite">
          <div className="bignum" aria-hidden="true">{String(active + 1).padStart(2, "0")}</div>
          <h3>{d.name}</h3>
          <StatusChip status={d.status} />
          <dl>
            <dt>Today</dt>
            <dd>{d.today}</dd>
            <dt>Next</dt>
            <dd>{d.next}</dd>
          </dl>
        </div>
      </div>
    </section>
  );
}
