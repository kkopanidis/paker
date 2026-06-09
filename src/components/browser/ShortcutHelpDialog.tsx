import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: "Alt+↑", action: "Go up" },
  { keys: "F5", action: "Refresh" },
  { keys: "Del", action: "Delete" },
  { keys: "⌘U", action: "Upload" },
  { keys: "⌘D", action: "Download" },
  { keys: "⌘A", action: "Select all" },
  { keys: "Enter", action: "Open" },
  { keys: "⌘F", action: "Filter" },
  { keys: "F2", action: "Rename" },
  { keys: "?", action: "Help" },
  { keys: "⌘/", action: "Help" },
];

interface ShortcutHelpDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ShortcutHelpDialog({ open, onOpenChange }: ShortcutHelpDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Keyboard shortcuts</DialogTitle>
          <DialogDescription>
            Shortcuts apply to the remote browser when it is focused. On Windows and Linux, use
            Ctrl instead of ⌘.
          </DialogDescription>
        </DialogHeader>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[40%]">Shortcut</TableHead>
              <TableHead>Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {SHORTCUTS.map((shortcut) => (
              <TableRow key={`${shortcut.keys}-${shortcut.action}`}>
                <TableCell>
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">
                    {shortcut.keys}
                  </kbd>
                </TableCell>
                <TableCell className="text-sm">{shortcut.action}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </DialogContent>
    </Dialog>
  );
}
