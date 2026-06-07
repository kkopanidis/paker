import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { S3Connection } from "@/types/connection";
import { useConnections } from "./useConnections";

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

afterEach(() => {
  clearMocks();
});

describe("useConnections", () => {
  it("loads connections via list_connections IPC and selects the first", async () => {
    const mockConnections: S3Connection[] = [
      {
        id: "conn-1",
        name: "MinIO Local",
        endpoint: "http://127.0.0.1:9000",
        region: "us-east-1",
        accessKeyId: "minioadmin",
        forcePathStyle: true,
      },
      {
        id: "conn-2",
        name: "AWS Prod",
        region: "eu-west-1",
        accessKeyId: "AKIAEXAMPLE",
        forcePathStyle: false,
      },
    ];

    mockIPC((cmd) => {
      if (cmd === "list_connections") {
        return mockConnections;
      }
    });

    const { result } = renderHook(() => useConnections());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.connections).toEqual(mockConnections);
    expect(result.current.selectedId).toBe("conn-1");
    expect(result.current.selected).toEqual(mockConnections[0]);
  });
});
