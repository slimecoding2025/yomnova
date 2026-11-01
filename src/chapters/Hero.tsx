import { useLayoutEffect, useRef } from "react";
import { gsap, MQ, SplitText } from "../lib/motion";
import { DOMAINS } from "../data";

const CX = 1200;
const CY = 450;
const SHORT = ["APPS", "SYSTEMS", "NETWORK", "PROTOCOL", "DISTRIBUTED", "DATAFLOW", "CONCURRENCY", "SECURITY", "INFRA", "EMBEDDED"];

const NODES = DOMAINS.map((_, i) => {
  const a = (i / DOMAINS.length) * Math.PI * 2 - Math.PI / 2;
  const r = 250 + (i % 3) * 38;
  return { x: CX + Math.cos(a) * r * 0.86, y: CY + Math.sin(a) * r * 1.05, label: SHORT[i] };
});

export function Hero({ playgroundOnline }: { playgroundOnline: boolean }) {
  const root = useRef<HTMLElement>(null);

  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const mm = gsap.matchMedia();

    mm.add(MQ.motion, () => {
      const h1 = el.querySelector<HTMLElement>("h1")!;
      const split = new SplitText(h1, { type: "chars" });
      const lines = el.querySelectorAll<SVGLineElement>(".hn-line");
      lines.forEach((l) => {
        const len = l.getTotalLength();
        gsap.set(l, { strokeDasharray: len, strokeDashoffset: len });
      });

      // Entrance (time-based).
      const intro = gsap.timeline({ defaults: { ease: "power3.out" } });
      intro
        .from(split.chars, { yPercent: 110, autoAlpha: 0, duration: 0.9, stagger: 0.025 })
        .to(lines, { strokeDashoffset: 0, duration: 1.4, stagger: 0.07, ease: "power2.inOut" }, 0.1)
        .from(".hn-dot", { scale: 0, duration: 0.5, stagger: 0.06 }, 0.5)
        .from(".hn-label", { autoAlpha: 0, duration: 0.6, stagger: 0.05 }, 0.8)
        .from(".hero-copy > :not(h1)", { autoAlpha: 0, y: 24, duration: 0.8, stagger: 0.12 }, 0.7)
        .from(".hero-symbol", { scale: 0.6, autoAlpha: 0, svgOrigin: `${CX} ${CY}`, duration: 1, ease: "back.out(1.4)" }, 0.3);

      return () => split.revert();
    });

    // Scroll story: the domains are drawn in toward one language. Desktop only, pinned.
    mm.add(MQ.desktop, () => {
      const tl = gsap.timeline({
        defaults: { ease: "none" },
        scrollTrigger: { trigger: el, start: "top top", end: "+=90%", pin: true, scrub: 0.6, anticipatePin: 1 },
      });
      tl.to(".hn-spoke", { scale: 0.52, svgOrigin: `${CX} ${CY}` }, 0)
        .to(".hn-label", { autoAlpha: 0 }, 0)
        .to(".hero-symbol", { scale: 1.5, svgOrigin: `${CX} ${CY}` }, 0)
        .to(".hn-halo", { autoAlpha: 1, scale: 1.4, svgOrigin: `${CX} ${CY}` }, 0.1)
        .to(".hero-copy", { yPercent: -18, autoAlpha: 0.0 }, 0.35)
        .to(".hero-meta", { autoAlpha: 0 }, 0);
    });

    return () => mm.revert();
  }, []);

  return (
    <section id="hero" ref={root} className="chapter hero" aria-label="Introduction">
      <svg className="hero-viz" viewBox="0 0 1600 900" preserveAspectRatio="xMaxYMid slice" aria-hidden="true" focusable="false">
        <circle className="hn-halo" cx={CX} cy={CY} r="120" fill="none" stroke="#19e3ff" strokeOpacity="0.35" opacity="0" />
        {NODES.map((n, i) => (
          <g className="hn-spoke" key={i}>
            <line className="hn-line" x1={n.x} y1={n.y} x2={CX} y2={CY} stroke="#19e3ff" strokeOpacity="0.38" strokeWidth="1" />
            <circle className="hn-dot" cx={n.x} cy={n.y} r="5" fill="#07080b" stroke="#f2eee6" strokeWidth="1.5" />
            <text className="hn-label" x={n.x + (n.x > CX ? 14 : -14)} y={n.y + 4} textAnchor={n.x > CX ? "start" : "end"}>
              {n.label}
            </text>
          </g>
        ))}
        <g className="hero-symbol">
          <image href="/brand/symbol.svg" x={CX - 56} y={CY - 56} width="112" height="112" />
        </g>
      </svg>

      <div className="hero-copy">
        <p className="eyebrow">Experimental programming language</p>
        <h1 aria-label="One language. Every system.">
          <span className="l1">ONE LANGUAGE.</span>
          <span className="l2">EVERY SYSTEM.</span>
        </h1>
        <p className="lede">
          An experimental hybrid programming language exploring the intersection of general-purpose programming, systems
          engineering, networking, and distributed computing.
        </p>
        <div className="cta">
          <a className="btn primary" href="#vision">Explore the Language</a>
          {playgroundOnline && <a className="btn" href="#playground">Open Playground</a>}
        </div>
      </div>

      <div className="hero-meta">
        <span>Early-stage · v0.1</span>
        <span className="scroll-hint">Scroll</span>
      </div>
    </section>
  );
}
