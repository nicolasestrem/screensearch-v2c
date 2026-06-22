// Shared route scaffold for the M1+M2 foundation: a titled Panel that establishes
// the screen's layout slot and states its job (UI_REFERENCE §3). The data-bearing
// screen bodies are implemented in later milestones (M3–M5); this PR delivers the
// shell, routing, IPC layer, and primitives they build on.
import type { ReactNode } from "react";
import { Panel } from "./primitives";

export interface ScreenScaffoldProps {
  title: string;
  /** One-line statement of what this screen is for. */
  purpose: ReactNode;
  children?: ReactNode;
}

export function ScreenScaffold({ title, purpose, children }: ScreenScaffoldProps) {
  return (
    <div className="p-6 max-w-5xl mx-auto flex flex-col gap-4">
      <Panel title={title}>
        <p className="text-body text-ink-muted font-body">{purpose}</p>
        {children}
      </Panel>
    </div>
  );
}
