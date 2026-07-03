import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { S3Connection, S3ConnectionInput } from "@/types/connection";
import { PROVIDER_PRESETS } from "@/types/connection";

interface ConnectionFormProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connection?: S3Connection | null;
  onSubmit: (input: S3ConnectionInput, id?: string) => Promise<unknown>;
}

const emptyForm: S3ConnectionInput = {
  name: "",
  endpoint: "",
  region: "us-east-1",
  accessKeyId: "",
  secretAccessKey: "",
  sessionToken: "",
  forcePathStyle: false,
  skipTlsVerify: false,
  defaultBucket: "",
};

function endpointUsesHttps(endpoint: string | undefined): boolean {
  const trimmed = endpoint?.trim();
  return Boolean(trimmed && trimmed.startsWith("https://"));
}

export function ConnectionForm({ open, onOpenChange, connection, onSubmit }: ConnectionFormProps) {
  const [form, setForm] = useState<S3ConnectionInput>(emptyForm);
  const [presetId, setPresetId] = useState("aws");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;

    if (connection) {
      setForm({
        name: connection.name,
        endpoint: connection.endpoint ?? "",
        region: connection.region,
        accessKeyId: connection.accessKeyId,
        secretAccessKey: "",
        sessionToken: "",
        forcePathStyle: connection.forcePathStyle,
        skipTlsVerify: connection.skipTlsVerify,
        defaultBucket: connection.defaultBucket ?? "",
      });
      const match = PROVIDER_PRESETS.find(
        (p) =>
          p.region === connection.region &&
          p.forcePathStyle === connection.forcePathStyle &&
          (p.endpoint ?? "") === (connection.endpoint ?? "")
      );
      setPresetId(match?.id ?? "custom");
      return;
    }

    const aws = PROVIDER_PRESETS.find((p) => p.id === "aws")!;
    setForm({
      ...emptyForm,
      region: aws.region,
      forcePathStyle: aws.forcePathStyle,
    });
    setPresetId("aws");
  }, [open, connection]);

  const skipTlsVerifyEnabled = endpointUsesHttps(form.endpoint);

  const applyPreset = (id: string) => {
    const preset = PROVIDER_PRESETS.find((p) => p.id === id);
    if (!preset) return;
    setPresetId(id);
    setForm((current) => ({
      ...current,
      endpoint: preset.endpoint ?? "",
      region: preset.region,
      forcePathStyle: preset.forcePathStyle,
    }));
  };

  const update = <K extends keyof S3ConnectionInput>(key: K, value: S3ConnectionInput[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
    if (["endpoint", "region", "forcePathStyle"].includes(key)) {
      setPresetId("custom");
    }
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const payload: S3ConnectionInput = {
        ...form,
        endpoint: form.endpoint?.trim() || undefined,
        defaultBucket: form.defaultBucket?.trim() || undefined,
        sessionToken: form.sessionToken?.trim() || undefined,
      };
      await onSubmit(payload, connection?.id);
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{connection ? "Edit connection" : "New connection"}</DialogTitle>
          <DialogDescription>
            Configure your S3-compatible storage credentials.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="provider">Provider</Label>
            <Select value={presetId} onValueChange={applyPreset}>
              <SelectTrigger id="provider">
                <SelectValue placeholder="Select provider" />
              </SelectTrigger>
              <SelectContent>
                {PROVIDER_PRESETS.map((preset) => (
                  <SelectItem key={preset.id} value={preset.id}>
                    {preset.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="name">Name</Label>
            <Input
              id="name"
              value={form.name}
              onChange={(e) => update("name", e.target.value)}
              placeholder="My S3"
              required
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="endpoint">Endpoint</Label>
            <Input
              id="endpoint"
              value={form.endpoint ?? ""}
              onChange={(e) => update("endpoint", e.target.value)}
              placeholder="https://s3.amazonaws.com"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <Label htmlFor="region">Region</Label>
              <Input
                id="region"
                value={form.region}
                onChange={(e) => update("region", e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="defaultBucket">Default bucket</Label>
              <Input
                id="defaultBucket"
                value={form.defaultBucket ?? ""}
                onChange={(e) => update("defaultBucket", e.target.value)}
                placeholder="required if key cannot list buckets"
              />
              <p className="text-xs text-muted-foreground">
                If your credentials only allow access to one bucket, enter it here to skip listing
                buckets.
              </p>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="accessKeyId">Access key ID</Label>
            <Input
              id="accessKeyId"
              value={form.accessKeyId}
              onChange={(e) => update("accessKeyId", e.target.value)}
              required
              autoComplete="off"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="secretAccessKey">
              Secret access key{connection ? " (leave blank to keep)" : ""}
            </Label>
            <Input
              id="secretAccessKey"
              type="password"
              value={form.secretAccessKey}
              onChange={(e) => update("secretAccessKey", e.target.value)}
              required={!connection}
              autoComplete="new-password"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="sessionToken">
              Session token (STS){connection ? " (leave blank to keep)" : ""}
            </Label>
            <Input
              id="sessionToken"
              type="password"
              value={form.sessionToken ?? ""}
              onChange={(e) => update("sessionToken", e.target.value)}
              placeholder="Optional — for temporary credentials"
              autoComplete="off"
            />
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="forcePathStyle"
              checked={form.forcePathStyle}
              onCheckedChange={(checked) => update("forcePathStyle", checked === true)}
            />
            <Label htmlFor="forcePathStyle">Force path-style addressing</Label>
          </div>

          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <Checkbox
                id="skipTlsVerify"
                checked={form.skipTlsVerify}
                disabled={!skipTlsVerifyEnabled}
                onCheckedChange={(checked) => update("skipTlsVerify", checked === true)}
              />
              <Label
                htmlFor="skipTlsVerify"
                className={!skipTlsVerifyEnabled ? "text-muted-foreground" : undefined}
              >
                Skip TLS certificate verification
              </Label>
            </div>
            <p className="text-xs text-muted-foreground">
              Only applies to HTTPS endpoints. Use for self-signed certificates in development — not
              recommended for production.
            </p>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={saving}>
              {saving ? "Saving…" : connection ? "Update" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
