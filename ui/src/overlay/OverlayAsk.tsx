import { Suspense, lazy } from "react";

import { Button, EmptyState, Skeleton } from "../components/primitives";
import { IconSparkle } from "../components/icons";
import type { UseAsk } from "../lib/ipc/useAsk";

const LazyAnswerStream = lazy(() =>
  import("../components/domain/AnswerStream").then((m) => ({
    default: m.AnswerStream,
  })),
);

export interface OverlayAskProps {
  question: string;
  ask: UseAsk;
  onSubmit: () => void;
  onOpenFrame: (frameId: number) => void;
}

export function OverlayAsk({
  question,
  ask,
  onSubmit,
  onOpenFrame,
}: OverlayAskProps) {
  if (ask.phase === "idle") {
    return (
      <EmptyState
        icon={<IconSparkle size={24} />}
        title="Ask about what you've seen"
        description="Questions stream from captured screens and keep their source frames attached."
        action={
          <Button
            variant="primary"
            size="sm"
            onClick={onSubmit}
            disabled={!question.trim()}
          >
            Ask
          </Button>
        }
      />
    );
  }

  return (
    <Suspense
      fallback={
        <div className="flex flex-col gap-3 p-3">
          <Skeleton className="h-20 w-full" />
          <Skeleton className="h-16 w-full" />
        </div>
      }
    >
      <div className="px-3 pb-3">
        <LazyAnswerStream
          phase={ask.phase}
          thinking={ask.thinking}
          answer={ask.answer}
          citations={ask.citations}
          error={ask.error}
          onRetry={onSubmit}
          onOpenFrame={onOpenFrame}
        />
      </div>
    </Suspense>
  );
}
