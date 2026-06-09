import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { OverwriteDialog } from "./OverwriteDialog";

const conflicts = [
  { name: "readme.txt", key: "docs/readme.txt" },
  { name: "notes.md", key: "docs/notes.md" },
];

function renderDialog(overrides: Partial<Parameters<typeof OverwriteDialog>[0]> = {}) {
  const onOpenChange = vi.fn();
  const onResolve = vi.fn();

  render(
    <OverwriteDialog
      open
      onOpenChange={onOpenChange}
      conflicts={conflicts}
      onResolve={onResolve}
      {...overrides}
    />
  );

  return { onOpenChange, onResolve };
}

describe("OverwriteDialog", () => {
  it("calls onResolve with skip and closes when Skip all is clicked", () => {
    const { onOpenChange, onResolve } = renderDialog();

    fireEvent.click(screen.getByRole("button", { name: "Skip all" }));

    expect(onResolve).toHaveBeenCalledWith("skip");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("calls onResolve with overwrite and closes when Overwrite all is clicked", () => {
    const { onOpenChange, onResolve } = renderDialog();

    fireEvent.click(screen.getByRole("button", { name: "Overwrite all" }));

    expect(onResolve).toHaveBeenCalledWith("overwrite");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("closes without resolving when Cancel is clicked", () => {
    const { onOpenChange, onResolve } = renderDialog();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onResolve).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("lists conflict names in the preview", () => {
    renderDialog();

    expect(screen.getByText("readme.txt")).toBeTruthy();
    expect(screen.getByText("notes.md")).toBeTruthy();
  });
});
