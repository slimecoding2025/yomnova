import type { ReactNode } from "react";
import { STATUS_LABEL, type Status } from "../data";
import { highlight } from "../lib/highlight";

export function StatusChip({ status }: { status: Status }) {
  return <span className={`chip ${status}`}>{STATUS_LABEL[status]}</span>;
}

export function ChapterTag({ n, title }: { n: number; title: string }) {
  return (
    <div className="chapter-tag">
      <b>{String(n).padStart(2, "0")}</b>
      <span>{title}</span>
    </div>
  );
}

/** Syntax-highlighted code, safe (text nodes only). */
export function Code({ src, className = "codeblock" }: { src: string; className?: string }) {
  return (
    <pre className={className}>
      {highlight(src).map((p, i) => (p.cls === "ws" || p.cls === "id" ? p.text : <span key={i} className={p.cls}>{p.text}</span>))}
    </pre>
  );
}

export function Section({
  id,
  className,
  label,
  children,
  sectionRef,
}: {
  id: string;
  className: string;
  label: string;
  children: ReactNode;
  sectionRef: React.RefObject<HTMLElement | null>;
}) {
  return (
    <section id={id} ref={sectionRef} className={`chapter ${className}`} aria-label={label}>
      {children}
    </section>
  );
}
