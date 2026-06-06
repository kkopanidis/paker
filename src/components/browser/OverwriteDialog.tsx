import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";

export interface OverwriteConflict {
  name: string;
  key: string;
}

interface OverwriteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  conflicts: OverwriteConflict[];
  onResolve: (action: "skip" | "overwrite" | "rename", renames?: Record<string, string>) => void;
}

export function OverwriteDialog({ open, onOpenChange, conflicts, onResolve }: OverwriteDialogProps) {
  const previewConflicts = conflicts.slice(0, 5);
  const remaining = conflicts.length - previewConflicts.length;

  const handleResolve = (action: "skip" | "overwrite") => {
    onResolve(action);
    onOpenChange(false);
  };

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {conflicts.length === 1 ? "File already exists" : `${conflicts.length} files already exist`}
          </AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-2">
              <p>
                {conflicts.length === 1
                  ? "A file with the same name already exists at the destination."
                  : "Some files already exist at the destination. Choose how to handle them."}
              </p>
              {previewConflicts.length > 0 && (
                <ul className="max-h-32 overflow-y-auto rounded-md border bg-muted/40 px-3 py-2 text-left text-sm text-foreground">
                  {previewConflicts.map((conflict) => (
                    <li key={conflict.key} className="truncate font-mono text-xs">
                      {conflict.name}
                    </li>
                  ))}
                  {remaining > 0 && (
                    <li className="text-xs text-muted-foreground">…and {remaining} more</li>
                  )}
                </ul>
              )}
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter className="sm:justify-between">
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <div className="flex flex-col gap-2 sm:flex-row">
            <Button variant="outline" onClick={() => handleResolve("skip")}>
              Skip all
            </Button>
            <Button variant="destructive" onClick={() => handleResolve("overwrite")}>
              Overwrite all
            </Button>
          </div>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
