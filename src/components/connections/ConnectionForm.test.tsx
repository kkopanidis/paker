import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ConnectionForm } from "./ConnectionForm";
import { PROVIDER_PRESETS } from "@/types/connection";

function renderForm(overrides: Partial<Parameters<typeof ConnectionForm>[0]> = {}) {
  const onOpenChange = vi.fn();
  const onSubmit = vi.fn().mockResolvedValue(undefined);

  render(
    <ConnectionForm
      open
      onOpenChange={onOpenChange}
      onSubmit={onSubmit}
      {...overrides}
    />
  );

  return { onOpenChange, onSubmit };
}

async function selectProvider(label: string) {
  fireEvent.click(screen.getByLabelText("Provider"));
  fireEvent.click(await screen.findByRole("option", { name: label }));
}

describe("ConnectionForm", () => {
  it("applies provider preset endpoint, region, and forcePathStyle", async () => {
    const minio = PROVIDER_PRESETS.find((p) => p.id === "minio")!;

    renderForm();
    await selectProvider(minio.label);

    expect(screen.getByLabelText("Endpoint")).toHaveProperty(
      "value",
      minio.endpoint ?? ""
    );
    expect(screen.getByLabelText("Region")).toHaveProperty("value", minio.region);
    expect(screen.getByLabelText("Force path-style addressing").getAttribute("data-state")).toBe(
      minio.forcePathStyle ? "checked" : "unchecked"
    );
  });

  it("does not submit when required fields are empty", async () => {
    const { onSubmit } = renderForm();

    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(onSubmit).not.toHaveBeenCalled();
    });
  });

  it("submits when required fields are filled", async () => {
    const { onSubmit } = renderForm();

    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Test S3" } });
    fireEvent.change(screen.getByLabelText("Endpoint"), {
      target: { value: "https://minio.local:9000" },
    });
    fireEvent.change(screen.getByLabelText("Access key ID"), {
      target: { value: "AKIATEST" },
    });
    fireEvent.change(screen.getByLabelText("Secret access key"), {
      target: { value: "secret-key" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Test S3",
          endpoint: "https://minio.local:9000",
          accessKeyId: "AKIATEST",
          secretAccessKey: "secret-key",
          skipTlsVerify: false,
        }),
        undefined
      );
    });
  });

  it("disables skip TLS verification for HTTP endpoints", async () => {
    renderForm();

    fireEvent.change(screen.getByLabelText("Endpoint"), {
      target: { value: "http://127.0.0.1:9000" },
    });

    const checkbox = screen.getByLabelText("Skip TLS certificate verification");
    expect(checkbox).toHaveProperty("disabled", true);
  });

  it("enables skip TLS verification for HTTPS endpoints", async () => {
    renderForm();

    fireEvent.change(screen.getByLabelText("Endpoint"), {
      target: { value: "https://minio.local:9000" },
    });

    const checkbox = screen.getByLabelText("Skip TLS certificate verification");
    expect(checkbox).toHaveProperty("disabled", false);
    fireEvent.click(checkbox);
    expect(checkbox.getAttribute("data-state")).toBe("checked");
  });

  it("includes skipTlsVerify in submit payload when enabled", async () => {
    const { onSubmit } = renderForm();

    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Dev MinIO" } });
    fireEvent.change(screen.getByLabelText("Endpoint"), {
      target: { value: "https://minio.local:9000" },
    });
    fireEvent.change(screen.getByLabelText("Access key ID"), {
      target: { value: "AKIATEST" },
    });
    fireEvent.change(screen.getByLabelText("Secret access key"), {
      target: { value: "secret-key" },
    });
    fireEvent.click(screen.getByLabelText("Skip TLS certificate verification"));
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          skipTlsVerify: true,
        }),
        undefined
      );
    });
  });
});
