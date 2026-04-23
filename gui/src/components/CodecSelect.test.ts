import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import CodecSelect from "./CodecSelect.svelte";

describe("CodecSelect", () => {
  it("hides chromakey when codec is prores4444 (default)", () => {
    const { queryByLabelText } = render(CodecSelect);
    expect(queryByLabelText(/Chromakey/)).toBeNull();
  });

  it("shows chromakey for h264_nvenc", async () => {
    const { getByLabelText, findByLabelText } = render(CodecSelect);
    const select = getByLabelText(/Codec/) as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: "h264_nvenc" } });
    const chroma = await findByLabelText(/Chromakey/);
    expect(chroma).not.toBeNull();
  });
});
