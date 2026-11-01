import { useRef } from "react";
import { ChapterTag, Code, StatusChip } from "../components/Common";
import { NET_TYPES } from "../data";
import { usePinnedSteps, useReveal } from "../lib/motion";

// IPv4 header, conceptually: 32 bit columns. Each field: [label, bits, group]
const FIELDS: Array<[string, number, number]> = [
  ["Ver", 4, 0], ["IHL", 4, 0], ["DSCP", 8, 0], ["Total length", 16, 0],
  ["Identification", 16, 0], ["Flags / offset", 16, 0],
  ["TTL", 8, 0], ["Protocol", 8, 0], ["Header checksum", 16, 0],
  ["Source address", 32, 1],
  ["Destination address", 32, 1],
  ["Source port", 16, 2], ["Destination port", 16, 2],
];

const STEPS = [
  { caption: "A protocol header is typed data with a fixed layout.", groups: [0, 1, 2] },
  { caption: "IPv4(\"…\") is a real value type in YOMNOVA: four octets, validated.", groups: [1] },
  { caption: "Port(n) is a real value type: 1–65535.", groups: [2] },
  { caption: "Endpoint(ip, port) pairs them. This is what runs today.", groups: [1, 2] },
];

const CODE = `let ip: IPv4 = IPv4("127.0.0.1")
let port: Port = Port(8080)
emit(Endpoint(ip, port))`;

export function Protocol() {
  const root = useRef<HTMLElement>(null);
  const { active, pinned } = usePinnedSteps(root, STEPS.length, { perStep: 50 });
  useReveal(root);
  const step = pinned ? active : STEPS.length - 1;
  const on = STEPS[step].groups;

  return (
    <section id="protocol" ref={root} className="chapter proto" aria-label="Protocol-aware programming">
      <div className="grid-bg" aria-hidden="true" />
      <ChapterTag n={6} title="Protocol-aware programming" />
      <h2 className="display rv">Network values, not strings.</h2>
      <div className="proto-layout">
        <div>
          <div className="hdr" role="img" aria-label="Conceptual diagram of an IPv4 header followed by transport ports">
            {FIELDS.map(([label, bits, g], i) => (
              <div key={i} className={on.includes(g) ? "on" : ""} style={{ gridColumn: `span ${bits}` }}>{label}</div>
            ))}
          </div>
          <p className="hdr-caption" aria-live="polite">{STEPS[step].caption}</p>
          <div className="proto-code">
            <Code src={CODE} />
            <p className="proto-out"><span>output</span>127.0.0.1:8080</p>
          </div>
          <p className="honest">
            The header diagram is conceptual. YOMNOVA does not parse or build packet headers yet; only the three value
            types on the right exist.
          </p>
        </div>
        <ul className="types-list rv">
          {NET_TYPES.map((t) => (
            <li key={t.name}>
              <span className="tn">{t.name}</span>
              <StatusChip status={t.status} />
              <span className="note">{t.note}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
