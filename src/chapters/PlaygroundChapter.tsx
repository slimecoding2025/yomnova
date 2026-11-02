import { useRef } from "react";
import { ChapterTag } from "../components/Common";
import { Playground } from "../playground/Playground";
import { useReveal } from "../lib/motion";

export function PlaygroundChapter({ version }: { version: string | null }) {
  const root = useRef<HTMLElement>(null);
  useReveal(root);
  return (
    <section id="playground" ref={root} className="chapter play" aria-label="Playground">
      <ChapterTag n={8} title="Playground" />
      {version ? (
        <>
          <div className="rv"><Playground version={version} /></div>
          <p className="pg-note">
            Every button calls the actual <code>yomnova</code> binary on the server: Run executes the program on the VM, and
            the Tokens, AST and Bytecode tabs are that binary&apos;s own output. The editor is a plain text area with a small
            syntax highlighter, not Monaco. Programs run with a 5-second limit and no file or network access, but the server
            has no OS-level sandbox.
          </p>
        </>
      ) : (
        <div className="pg-offline">
          <p>
            The Playground needs the YOMNOVA compiler server, which is not reachable right now. Nothing is simulated here.
            Start it locally with <code>npm run build:compiler</code> followed by <code>npm run dev</code>.
          </p>
        </div>
      )}
    </section>
  );
}
