import { describe, it, expect, vi, beforeEach } from "vitest";

const { setImage, setBusy } = vi.hoisted(() => ({
  setImage: vi.fn(),
  setBusy: vi.fn(),
}));

vi.mock("./tauri", () => ({
  previewFrame: vi.fn(),
}));

vi.mock("./stores", () => ({
  previewImage: { set: setImage },
  previewBusy: { set: setBusy },
}));

import { requestPreview, __reset } from "./preview-dispatcher";
import { previewFrame } from "./tauri";

describe("preview-dispatcher latest-wins", () => {
  beforeEach(() => {
    __reset();
    setImage.mockClear();
    setBusy.mockClear();
    (previewFrame as any).mockReset();
  });

  it("applies responses in order when they arrive in order", async () => {
    (previewFrame as any)
      .mockResolvedValueOnce("A")
      .mockResolvedValueOnce("B");
    await requestPreview(1);
    await requestPreview(2);
    const calls = setImage.mock.calls.map((c) => c[0]);
    expect(calls).toEqual(["A", "B"]);
  });

  it("drops stale responses that resolve after newer ones", async () => {
    let resolveA!: (v: string) => void;
    (previewFrame as any)
      .mockImplementationOnce(() => new Promise<string>((r) => { resolveA = r; }))
      .mockResolvedValueOnce("B");
    const pa = requestPreview(1);
    const pb = requestPreview(2);
    await pb;                  // B resolves first -> image = "B"
    resolveA("A_STALE");        // A resolves after -> should be dropped
    await pa;
    const calls = setImage.mock.calls.map((c) => c[0]);
    expect(calls).toEqual(["B"]); // never applied A_STALE
  });

  it("toggles busy around every call", async () => {
    (previewFrame as any).mockResolvedValueOnce("X");
    await requestPreview(1);
    const calls = setBusy.mock.calls.map((c) => c[0]);
    expect(calls).toEqual([true, false]);
  });
});
