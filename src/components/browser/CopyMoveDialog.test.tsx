import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { CopyMoveDialog } from "./CopyMoveDialog";
import { sampleBuckets } from "@/test/fixtures";

function confirmButton(label: "Copy" | "Move"): HTMLButtonElement {
  const dialog = screen.getByRole("dialog");
  const buttons = within(dialog).getAllByRole("button", { name: label });
  return buttons[buttons.length - 1] as HTMLButtonElement;
}

function renderDialog(overrides: Partial<Parameters<typeof CopyMoveDialog>[0]> = {}) {
  const onOpenChange = vi.fn();
  const onConfirm = vi.fn();

  render(
    <CopyMoveDialog
      open
      onOpenChange={onOpenChange}
      buckets={sampleBuckets}
      currentBucket="bucket-a"
      itemCount={2}
      onConfirm={onConfirm}
      {...overrides}
    />
  );

  return { onOpenChange, onConfirm };
}

async function selectBucket(name: string) {
  fireEvent.click(screen.getByLabelText("Destination bucket"));
  fireEvent.click(await screen.findByRole("option", { name }));
}

describe("CopyMoveDialog", () => {
  it("defaults to the current bucket for same-bucket copy", () => {
    renderDialog();

    expect(screen.getByLabelText("Destination bucket").textContent).toContain("bucket-a");
    expect(confirmButton("Copy").disabled).toBe(false);
  });

  it("passes destination prefix on confirm within the same bucket", () => {
    const { onConfirm } = renderDialog();

    fireEvent.change(screen.getByLabelText("Destination folder (optional)"), {
      target: { value: "backups/2024/" },
    });
    fireEvent.click(confirmButton("Copy"));

    expect(onConfirm).toHaveBeenCalledWith("bucket-a", "backups/2024/", "copy");
  });

  it("confirms to a different bucket for cross-bucket move", async () => {
    const { onConfirm } = renderDialog({ initialMode: "move" });

    await selectBucket("bucket-b");
    fireEvent.change(screen.getByLabelText("Destination folder (optional)"), {
      target: { value: "imports/" },
    });
    fireEvent.click(confirmButton("Move"));

    expect(onConfirm).toHaveBeenCalledWith("bucket-b", "imports/", "move");
  });

  it("disables confirm when no bucket is available", () => {
    renderDialog({ currentBucket: null, buckets: [] });

    expect(confirmButton("Copy").disabled).toBe(true);
  });
});
