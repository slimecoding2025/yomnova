import { useRef } from "react";
import { ChapterTag } from "../components/Common";
import { gsap, usePinnedSteps, useReveal } from "../lib/motion";

const NODES = [
  { name: "5", sub: "source" },
  { name: "inc", sub: "5 + 1" },
  { name: "scale(10)", sub: "× 10" },
  { name: "result", sub: "flow binding" },
];
const VALUES = ["5", "6", "60", "60"];

export function Dataflow() {
  const root = useRef<HTMLElement>(null);
  const val = useRef<HTMLDivElement>(null);
  const lane = useRef<HTMLDivElement>(null);

  const { active, pinned } = usePinnedSteps(root, NODES.length, {
    perStep: 45,
    onProgress: (p) => {
      const l = lane.current;
      const v = val.current;
      if (!l || !v) return;
      const cell = l.clientWidth / NODES.length;
      const idx = Math.min(NODES.length - 1, Math.floor(p * NODES.length));
      gsap.to(v, { x: cell * idx + cell / 2 - v.offsetWidth / 2, duration: 0.5, ease: "power3.out", overwrite: true });
    },
  });
  useReveal(root);
  const step = pinned ? active : NODES.length - 1;

  return (
    <section id="dataflow" ref={root} className="chapter flow" aria-label="Dataflow and concurrency" data-mode={pinned ? "pinned" : undefined}>
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={7} title="Dataflow & concurrency" />
      <h2 className="display rv">Data moves left to right.</h2>
      <div className="flow-layout">
        <p className="flow-src">
          <span className="active">flow result = 5 |&gt; inc |&gt; scale(10)</span>
        </p>
        <div className="lane" ref={lane} role="img" aria-label="A value 5 flows through inc to 6, then scale(10) to 60">
          {pinned && <div className="value" ref={val}>{VALUES[step]}</div>}
          {NODES.map((n, i) => (
            <div key={n.name} className={`node${i <= step ? " on" : ""}`}>
              {n.name}
              <small>{n.sub}</small>
            </div>
          ))}
        </div>
        <div className="flow-cols">
          <div className="rv">
            <h3><span className="chip implemented">Implemented</span></h3>
            <ul>
              <li><code>|&gt;</code> applies a function: <code>x |&gt; f</code> is <code>f(x)</code>, <code>x |&gt; f(a)</code> is <code>f(x, a)</code></li>
              <li><code>flow name = …</code> binds the result of a pipeline</li>
              <li><code>|&gt; emit</code> prints the value</li>
              <li>Every stage is checked: it must exist, take the right arguments and types</li>
            </ul>
          </div>
          <div className="rv">
            <h3><span className="chip planned">Planned</span> not built</h3>
            <div className="planned-lanes" aria-label="Planned parallel lanes, not implemented">
              <span>parallel stages</span><i /><span>channels · tasks · async</span><i /><span>distributed nodes</span>
            </div>
            <p style={{ marginTop: 14, color: "var(--ink-faint)", fontSize: 14 }}>
              Execution today is sequential on a single-threaded VM. There is no concurrency yet.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
