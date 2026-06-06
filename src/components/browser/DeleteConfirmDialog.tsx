import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface DeleteConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  itemCount: number;
  itemNames: string[];
  onConfirm: () => void;
  busy?: boolean;
}

export function DeleteConfirmDialog({
  open,
  onOpenChange,
  itemCount,
  itemNames,
  onConfirm,
  busy,
}: DeleteConfirmDialogProps) {
  const previewNames = itemNames.slice(0, 5);
  const remaining = itemNames.length - previewNames.length;

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            Delete {itemCount} {itemCount === 1 ? "item" : "items"}?
          </AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-2">
              <p>This action cannot be undone.</p>
              {previewNames.length > 0 && (
                <ul className="max-h-32 overflow-y-auto rounded-md border bg-muted/40 px-3 py-2 text-left text-sm text-foreground">
                  {previewNames.map((name) => (
                    <li key={name} className="truncate font-mono text-xs">
                      {name}
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
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            disabled={busy}
            className={cn(buttonVariants({ variant: "destructive" }))}
            onClick={(event) => {
              event.preventDefault();
              onConfirm();
            }}
          >
            {busy ? "Deleting…" : "Delete"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
