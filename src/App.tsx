import { useEffect, useLayoutEffect, useState } from "react";
import { Logo } from "./components/Logo";
import { Architecture } from "./chapters/Architecture";
import { Closing } from "./chapters/Closing";
import { Dataflow } from "./chapters/Dataflow";
import { Domains } from "./chapters/Domains";
import { Hero } from "./chapters/Hero";
import { Ownership } from "./chapters/Ownership";
import { PlaygroundChapter } from "./chapters/PlaygroundChapter";
import { Protocol } from "./chapters/Protocol";
import { Roadmap } from "./chapters/Roadmap";
import { Vision } from "./chapters/Vision";
import { REPO_URL } from "./data";
import { api } from "./lib/api";
import { gsap, ScrollTrigger } from "./lib/motion";

const CHAPTERS = [
  ["hero", "Intro"], ["vision", "Vision"], ["domains", "Domains"], ["architecture", "Architecture"], ["ownership", "Ownership"],
  ["protocol", "Protocol"], ["dataflow", "Dataflow"], ["playground", "Playground"], ["roadmap", "Roadmap"], ["closing", "Closing"],
] as const;

export default function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [current, setCurrent] = useState("hero");

  useEffect(() => {
    api.version().then(setVersion);
  }, []);

  // Chapter rail + page progress. Created after the chapters (children's layout effects run first),
  // so the triggers are registered top-to-bottom after every pin.
  useLayoutEffect(() => {
    const triggers = CHAPTERS.map(([id]) =>
      ScrollTrigger.create({
        trigger: `#${id}`,
        start: "top 55%",
        end: "bottom 55%",
        onToggle: (self) => self.isActive && setCurrent(id),
      }),
    );
    const progress = gsap.to(".progress i", {
      scaleX: 1,
      ease: "none",
      scrollTrigger: { trigger: document.body, start: "top top", end: "bottom bottom", scrub: 0.3 },
    });
    const onLoad = () => ScrollTrigger.refresh();
    window.addEventListener("load", onLoad);
    return () => {
      window.removeEventListener("load", onLoad);
      triggers.forEach((t) => t.kill());
      progress.scrollTrigger?.kill();
      progress.kill();
    };
  }, []);

  // The Playground section changes height when the server status arrives.
  useEffect(() => {
    ScrollTrigger.refresh();
  }, [version]);

  return (
    <>
      <a className="skip" href="#main">Skip to content</a>
      <div className="progress" aria-hidden="true"><i /></div>
      <header className="site-header">
        <a className="logo-link" href="#hero" aria-label="YOMNOVA home"><Logo /></a>
        <nav className="header-nav" aria-label="Primary">
          <a className="hide-sm" href="#architecture">Architecture</a>
          <a className="hide-sm" href="#roadmap">Roadmap</a>
          {version && <a href="#playground">Playground</a>}
          <a href={REPO_URL} rel="noopener">GitHub</a>
        </nav>
      </header>
      <nav className="rail" aria-label="Chapters">
        {CHAPTERS.map(([id, label], i) => (
          <a key={id} href={`#${id}`} aria-current={current === id ? "true" : undefined}>
            <span>{String(i + 1).padStart(2, "0")} {label}</span>
          </a>
        ))}
      </nav>
      <main id="main">
        <Hero playgroundOnline={version !== null} />
        <Vision />
        <Domains />
        <Architecture />
        <Ownership />
        <Protocol />
        <Dataflow />
        <PlaygroundChapter version={version} />
        <Roadmap />
        <Closing />
      </main>
      <footer className="site-footer">
        <span>YOMNOVA — ONE LANGUAGE. EVERY SYSTEM.</span>
        <span>Created by Mouhamed Salim Bousmina · MIT OR Apache-2.0</span>
      </footer>
    </>
  );
}
