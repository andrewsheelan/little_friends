/** Last folder name, matching the Rust `backup_label`. Empty means off. */
export function backupName(dir: string): string {
  const trimmed = dir.trim();
  if (!trimmed) {
    return "Off";
  }
  const parts = trimmed.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? trimmed;
}
