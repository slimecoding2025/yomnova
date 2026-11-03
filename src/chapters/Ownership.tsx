import { useRef } from "react";
import { ChapterTag, StatusChip } from "../components/Common";
import { OWNERSHIP_CONCEPTS, OWNERSHIP_DIAGNOSTIC, OWNERSHIP_SRC } from "../data";
import { highlight } from "../lib/highlight";
import { usePinnedSteps, useReveal } from "../lib/motion";

const STEPS = [
  { line: 7, text: "`msg` owns the text value. Only one owner exists at a time." },
  { line: 8, text: "Passing `msg` to consume() moves it. The caller no longer owns it." },
  { line: 9, text: "Using `msg` again is rejected at compile time: the program never runs." },
];

function Line({ text, cls }: { text: string; cls: string }) {
  return (
    <span className={`own-line ${cls}`}>
      {text === "" ? " " : highlight(text).map((p, i) => (p.cls === "ws" || p.cls === "id" ? p.text : <span key={i} className={p.cls}>{p.text}</span>))}
    </span>
  );
}

export function Ownership() {
  const root = useRef<HTMLElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const { active, pinned } = usePinnedSteps(root, STEPS.length, { perStep: 60, mobileTarget: stage });
  useReveal(root);
  // Not pinned (mobile / reduced motion): show the end state with the diagnostic.
  const step = pinned ? active : STEPS.length - 1;

  return (
    <section id="ownership" ref={root} className="chapter own" aria-label="Ownership and resource safety">
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={5} title="Ownership & resource safety" />
      <h2 className="display rv">One owner. Checked before it runs.</h2>
      <div className="own-layout">
        <div className="own-code" ref={stage}>
          <pre className="codeblock" style={{ padding: "16px 4px" }} aria-label="Example program with a use after move">
            {OWNERSHIP_SRC.map((l, i) => (
              <Line key={i} text={l} cls={i + 1 === STEPS[step].line ? (step === 2 ? "bad" : "hot") : ""} />
            ))}
          </pre>
          <div className="owners" aria-hidden="true">
            <div className={`owner ${step === 0 ? "has" : "gone"}`}><small>main · msg</small><span className="val">TEXT</span>{step > 0 && <small style={{ margin: "6px 0 0" }}>moved</small>}</div>
            <span className="own-arrow">→</span>
            <div className={`owner ${step === 1 ? "has" : "gone"}`}><small>consume · payload</small><span className="val">TEXT</span></div>
          </div>
          {step === 2 && <div className="diag" role="alert">{OWNERSHIP_DIAGNOSTIC}</div>}
          <p className="own-step" aria-live="polite" style={{ marginTop: 18 }}>{STEPS[step].text.replaceAll("`", "")}</p>
        </div>
        <div className="own-side rv">
          <h3>What exists today</h3>
          <ul className="concepts">
            {OWNERSHIP_CONCEPTS.map((c) => (
              <li key={c.name}>
                <strong>{c.name}</strong>
                <StatusChip status={c.status} />
                <span className="note">{c.note}</span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
