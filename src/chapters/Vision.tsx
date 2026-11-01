import { useLayoutEffect, useRef } from "react";
import { ChapterTag } from "../components/Common";
import { gsap, MQ, SplitText } from "../lib/motion";

export function Vision() {
  const root = useRef<HTMLElement>(null);

  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const mm = gsap.matchMedia();
    mm.add(MQ.desktop, () => {
      const text = el.querySelector<HTMLElement>(".vision-text")!;
      const split = new SplitText(text, { type: "words" });
      gsap.set(split.words, { opacity: 0.14 });
      const rows = gsap.utils.toArray<HTMLElement>(".frag-row", el);
      const tl = gsap.timeline({
        defaults: { ease: "none" },
        scrollTrigger: { trigger: el, start: "top top", end: "+=140%", pin: true, scrub: 0.6, anticipatePin: 1 },
      });
      tl.to(split.words, { opacity: 1, stagger: 0.5, duration: 1 }, 0)
        .to(".vision .grid-bg", { yPercent: -12 }, 0)
        // the separate tools collapse into one row
        .to(rows.slice(0, -1), { opacity: 0.35, yPercent: (i) => (rows.length - 1 - i) * 18 }, 0.5 * split.words.length * 0.5)
        .to(rows[rows.length - 1], { scale: 1.04, transformOrigin: "50% 50%" }, "<");
      return () => split.revert();
    });
    mm.add(MQ.mobile, () => {
      gsap.from(el.querySelectorAll(".vision-text, .frag"), {
        autoAlpha: 0, y: 28, duration: 0.9, stagger: 0.15, ease: "power3.out",
        scrollTrigger: { trigger: el, start: "top 75%", once: true },
      });
    });
    return () => mm.revert();
  }, []);

  return (
    <section id="vision" ref={root} className="chapter vision" aria-label="Vision">
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={2} title="Vision" />
      <div className="vision-grid">
        <p className="vision-text">
          Software for networks is written in pieces: one language for applications, another for systems code, a toolkit for
          protocols, a framework for pipelines. YOMNOVA is an experiment in making that <em>one coherent language</em> where
          network types, ownership and dataflow belong to the language itself.
        </p>
        <div className="frag" role="img" aria-label="Four separate tools converging into one language">
          <div className="frag-row">Application language <small>apps</small></div>
          <div className="frag-row">Systems language <small>memory</small></div>
          <div className="frag-row">Protocol toolkit <small>wire</small></div>
          <div className="frag-row">Pipeline framework <small>flow</small></div>
          <div className="frag-row">YOMNOVA <small>one language · experimental</small></div>
        </div>
      </div>
    </section>
  );
}
