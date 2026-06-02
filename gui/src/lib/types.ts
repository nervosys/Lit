/* ─── Status ─── */

export interface StatusData {
  branch: string | null;
  head: string | null;
  staged: string[];
  modified: string[];
  untracked: string[];
  clean: boolean;
}

/* ─── Branch ─── */

export interface BranchEntry {
  name: string;
  is_current: boolean;
}

export interface BranchListResponse {
  action: "list";
  branches: BranchEntry[];
}

/* ─── Log ─── */

export interface CommitEntry {
  hash: string;
  short_hash: string;
  author: string;
  timestamp: number;
  message: string;
  is_head: boolean;
}

export interface LogResponse {
  branch: string | null;
  commits: CommitEntry[];
}

/* ─── Diff ─── */

export interface DiffLine {
  kind: "context" | "add" | "remove";
  content: string;
}

export interface DiffHunk {
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  status: "added" | "modified" | "deleted";
  hunks: DiffHunk[];
  old_hash: string | null;
  new_hash: string | null;
  is_binary: boolean;
  additions: number;
  deletions: number;
}

export interface DiffResponse {
  files: FileDiff[];
  files_changed: number;
  total_additions: number;
  total_deletions: number;
}

/* ─── Stack ─── */

export interface StackEntry {
  branch: string;
  base: string;
}

/* ─── Stash ─── */

export interface StashEntry {
  index: number;
  message: string;
  branch: string | null;
  timestamp: number;
}

export interface StashListResponse {
  action: "list";
  entries: StashEntry[];
}

/* ─── Search ─── */

export interface SearchMatch {
  file: string;
  line_number: number;
  content: string;
  commit: string | null;
  match_type: string;
}

export interface SearchResponse {
  query: string;
  match_type: string;
  matches: SearchMatch[];
  total: number;
}

/* ─── Undo ─── */

export interface OpLogEntry {
  id: string;
  timestamp: string;
  operation: string;
  description: string;
  undone: boolean;
}

/* ─── View state ─── */

export type SidebarView = "workspace" | "branches" | "stacks" | "stash" | "history";
export type Theme = "dark" | "light";
