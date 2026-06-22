// NotFound (*) — catch-all. An invitation back to the Deck, not a dead end.
import { useNavigate } from "react-router-dom";
import { Button, EmptyState } from "../components/primitives";

export function Component() {
  const navigate = useNavigate();
  return (
    <div className="p-6">
      <EmptyState
        title="Not found"
        description="That screen doesn't exist."
        action={
          <Button variant="primary" onClick={() => navigate("/")}>
            Back to Deck
          </Button>
        }
      />
    </div>
  );
}
