import assert from "node:assert/strict";
import test from "node:test";

import { packFixedSessionBands } from "../src/components/domain/sessionBandLayout.ts";

test("five simultaneous sessions use four fixed rows and aggregate the fifth", () => {
  const sessions = Array.from({ length: 5 }, (_, index) => ({
    id: index + 1,
    start: 10,
    end: 20,
  }));

  const packed = packFixedSessionBands(
    sessions,
    0,
    100,
    { width: 320, hitTarget: 32, gap: 4 },
    (session) => session.start,
    (session) => session.end,
  );

  assert.equal(packed.rowCount, 4);
  assert.equal(packed.bands.length, 4);
  assert.deepEqual(
    packed.bands.map(({ item, lane }) => [item.id, lane]),
    [
      [1, 0],
      [2, 1],
      [3, 2],
      [4, 3],
    ],
  );
  assert.equal(packed.overflowCount, 1);
});
