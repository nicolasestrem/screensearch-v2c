// Recall (/recall) — hybrid search + grounded Ask (RAG answer). Foundation
// scaffold (M1+M2); the search list and AnswerStream land in M3.
import { ScreenScaffold } from "../components/ScreenScaffold";

export function Component() {
  return (
    <ScreenScaffold
      title="Recall"
      purpose="Search your screen history and ask grounded questions, with cited frames."
    />
  );
}
