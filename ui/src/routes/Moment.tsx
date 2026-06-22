// Moment (/timeline/:id) — one frame: image, OCR text, vision tags, context, and
// the on-demand "queue vision" action. Foundation scaffold (M1+M2); the detail
// body lands in M4. Reads the route param so deep links resolve already.
import { useParams } from "react-router-dom";
import { ScreenScaffold } from "../components/ScreenScaffold";

export function Component() {
  const { id } = useParams();
  return (
    <ScreenScaffold
      title={`Moment #${id ?? ""}`}
      purpose="One captured frame: its image, recognized text, vision tags, and context."
    />
  );
}
