import { useRef } from "react";
import { ChapterTag, Code, StatusChip } from "../components/Common";
import { STAGES } from "../data";
import { gsap, usePinnedSteps, useReveal } from "../lib/motion";

export function Architecture() {
  const root = useRef<HTMLElement>(null);
  const packet = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLOListElement>(null);

  const { active, pinned } = usePinnedSteps(root, STAGES.length, {
    perStep: 40,
    onProgress: (p) => {
      const ol = list.current;
      const pk = packet.current;
      if (!ol || !pk) return;
      const items = ol.children;
      const last = items[items.length - 1] as HTMLElement;
      const step = last.offsetLeft / (STAGES.length - 1);
      const x = Math.min(STAGES.length - 1, Math.max(0, p * STAGES.length - 0.5)) * step;
      gsap.to(pk, { x, duration: 0.35, ease: "power2.out", overwrite: true });
    },
  });
  useReveal(root);
  const s = STAGES[active];

  return (
    <section id="architecture" ref={root} className="chapter arch" aria-label="Language architecture" data-mode={pinned ? "pinned" : undefined}>
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={4} title="Language architecture" />
      <h2 className="display rv">From source to a running program.</h2>

      <div className="arch-track" aria-hidden={!pinned}>
        <div className="packet" ref={packet} aria-hidden="true" />
        <ol className="stage-list" ref={list}>
          {STAGES.map((st, i) => (
            <li key={st.name} className={`stage-item${st.status === "planned" ? " planned" : ""}${pinned && i < active ? " done" : ""}${pinned && i === active ? " on" : ""}`}>
              {st.name}
            </li>
          ))}
        </ol>
      </div>

      <div className="stage-detail stage-current" aria-live="polite">
        <div>
          <StatusChip status={s.status} />
          <h3>{s.name}</h3>
          {s.crate !== "—" && <span className="crate">{s.crate}</span>}
          <p>{s.note}</p>
        </div>
        <div>
          {s.artifact ? (
            <>
              <div className="artifact-label">{s.artifact.label} · real output</div>
              <Code src={s.artifact.text} />
            </>
          ) : (
            <p className="mono" style={{ color: "var(--ink-faint)", fontSize: 13 }}>
              {s.status === "planned" ? "Not implemented: nothing to show." : "No separate artifact; see the Playground for live output."}
            </p>
          )}
        </div>
      </div>

      <div className="stage-all">
        {STAGES.map((st, i) => (
          <article key={st.name} className="rv">
            <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
              <span className="mono" style={{ color: "var(--ink-faint)" }}>{String(i + 1).padStart(2, "0")}</span>
              <h3>{st.name}</h3>
              <StatusChip status={st.status} />
            </div>
            <span className="crate">{st.crate}</span>
            <p>{st.note}</p>
            {st.artifact && <Code src={st.artifact.text} />}
          </article>
        ))}
      </div>
    </section>
  );
}
