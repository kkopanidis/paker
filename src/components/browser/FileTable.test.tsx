import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { FileTable } from "./FileTable";
import { sampleObjects } from "@/test/fixtures";

function renderTable(overrides: Partial<Parameters<typeof FileTable>[0]> = {}) {
  const onToggleKey = vi.fn();
  const onToggleAll = vi.fn();
  const onOpenFolder = vi.fn();

  render(
    <FileTable
      objects={sampleObjects}
      selectedKeys={new Set()}
      onToggleKey={onToggleKey}
      onToggleAll={onToggleAll}
      onOpenFolder={onOpenFolder}
      {...overrides}
    />
  );

  return { onToggleKey, onToggleAll, onOpenFolder };
}

describe("FileTable", () => {
  it("calls onToggleKey when a row checkbox is toggled", () => {
    const { onToggleKey } = renderTable();

    fireEvent.click(screen.getByRole("checkbox", { name: "Select readme.txt" }));

    expect(onToggleKey).toHaveBeenCalledWith("docs/readme.txt", true);
  });

  it("calls onToggleAll when the header checkbox is toggled", () => {
    const { onToggleAll } = renderTable();

    fireEvent.click(screen.getByRole("checkbox", { name: "Select all" }));

    expect(onToggleAll).toHaveBeenCalledWith(true);
  });

  it("renders only rows matching filterObjects text filter", () => {
    renderTable({ filterText: "readme" });

    expect(screen.getByText("readme.txt")).toBeTruthy();
    expect(screen.queryByText("old.zip")).toBeNull();
    expect(screen.queryByText("cat.jpg")).toBeNull();
    expect(screen.queryByText("docs")).toBeNull();
  });

  it("renders only rows matching filterObjects type filter", () => {
    renderTable({ typeFilter: "folders" });

    expect(screen.getByText("docs")).toBeTruthy();
    expect(screen.queryByText("readme.txt")).toBeNull();
    expect(screen.queryByText("old.zip")).toBeNull();
  });

  it("combines text and type filters via filterObjects", () => {
    renderTable({ filterText: "old", typeFilter: "glacier" });

    expect(screen.getByText("old.zip")).toBeTruthy();
    expect(screen.queryByText("readme.txt")).toBeNull();
    expect(screen.queryByText("cat.jpg")).toBeNull();
  });

  it("shows empty state when filters exclude all objects", () => {
    renderTable({ filterText: "missing" });

    expect(screen.getByText("No matching objects.")).toBeTruthy();
  });
});
