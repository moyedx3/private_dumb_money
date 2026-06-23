import { describe, expect, it } from "vitest";
import { MockDropApi } from "./mockApi";
import { DispatchPoller } from "./poller";
import { createPurchase } from "./purchase";
import { sodiumReady } from "./seal";

describe("poll → unlock (mock end-to-end)", () => {
  it("unlocks only the buyer's own dispatch and skips strangers'", async () => {
    await sodiumReady();
    const api = await MockDropApi.demo();
    const [drop] = await api.fetchCatalog();

    const purchase = await createPurchase(drop);
    const poller = new DispatchPoller(api);

    // before payment: nothing to open
    expect(await poller.poll([purchase])).toHaveLength(0);

    // a stranger's dispatch blob appears — must be trial-opened and skipped
    await api.seedForeignDispatch();
    expect(await poller.poll([purchase])).toHaveLength(0);

    // our payment lands → A1 publishes a dispatch blob sealed to our e_pub
    await api.simulateDispatch(drop.drop_id, purchase.ePub);
    const unlocked = await poller.poll([purchase]);

    expect(unlocked).toHaveLength(1);
    expect(unlocked[0].purchase.id).toBe(purchase.id);
    expect(unlocked[0].kDrop.length).toBe(32);
    expect(new TextDecoder().decode(unlocked[0].content)).toContain("cat photo");
  });
});
