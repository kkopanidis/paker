import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  deleteConnection as deleteConnectionApi,
  listConnections,
  parseStructuredError,
  saveConnection,
  testConnection,
  type PakerIpcError,
} from "@/lib/tauri";
import type { S3Connection, S3ConnectionInput } from "@/types/connection";

function formatIpcError(error: unknown): string {
  const parsed: PakerIpcError = parseStructuredError(error);
  return parsed.userAction
    ? `${parsed.message} ${parsed.userAction}`
    : parsed.message;
}

export function useConnections() {
  const [connections, setConnections] = useState<S3Connection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [testingId, setTestingId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = await listConnections();
      setConnections(list);
      setSelectedId((current) => {
        if (current && list.some((c) => c.id === current)) return current;
        return list[0]?.id ?? null;
      });
    } catch (error) {
      toast.error("Failed to load connections", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = useCallback(
    async (input: S3ConnectionInput, id?: string, options?: { quiet?: boolean }) => {
      try {
        const saved = await saveConnection({ ...input, id });
        await refresh();
        setSelectedId(saved.id);
        if (!options?.quiet) {
          toast.success(id ? "Connection updated" : "Connection created");
        }
        return saved;
      } catch (error) {
        toast.error(id ? "Failed to update connection" : "Failed to create connection", {
          description: formatIpcError(error),
        });
        throw error;
      }
    },
    [refresh]
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteConnectionApi(id);
      await refresh();
      toast.success("Connection deleted");
    },
    [refresh]
  );

  const test = useCallback(async (id: string) => {
    setTestingId(id);
    try {
      await testConnection(id);
      toast.success("Connection successful", {
        description: "Credentials and endpoint are valid.",
      });
    } catch (error) {
      toast.error("Connection test failed", {
        description: formatIpcError(error),
      });
      throw error;
    } finally {
      setTestingId(null);
    }
  }, []);

  const selected = connections.find((c) => c.id === selectedId) ?? null;

  return {
    connections,
    selected,
    selectedId,
    setSelectedId,
    loading,
    testingId,
    refresh,
    save,
    remove,
    test,
  };
}
