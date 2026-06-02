import { invoke } from "@tauri-apps/api/core";
import type {
  StatusData,
  LogResponse,
  DiffResponse,
  BranchListResponse,
  StackEntry,
  StashListResponse,
  SearchResponse,
  OpLogEntry,
} from "./types";

/* ─── Error helpers ─── */

/** Normalize any thrown value (Tauri returns string errors) into a message. */
export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return "Unknown error";
}

/* ─── Status ─── */

export async function getStatus(): Promise<StatusData> {
  return invoke<StatusData>("get_status");
}

/* ─── Branches ─── */

export async function listBranches(): Promise<BranchListResponse> {
  return invoke<BranchListResponse>("list_branches");
}

export async function createBranch(name: string): Promise<unknown> {
  return invoke("create_branch", { name });
}

export async function deleteBranch(name: string): Promise<unknown> {
  return invoke("delete_branch", { name });
}

export async function checkoutBranch(target: string): Promise<unknown> {
  return invoke("checkout_branch", { target });
}

/* ─── Log ─── */

export async function getLog(count?: number): Promise<LogResponse> {
  return invoke<LogResponse>("get_log", { count: count ?? null });
}

/* ─── Diff ─── */

export async function getDiff(staged: boolean = false): Promise<DiffResponse> {
  return invoke<DiffResponse>("get_diff", { staged });
}

/* ─── Commit ─── */

export async function createCommit(message: string): Promise<unknown> {
  return invoke("create_commit", { message });
}

/* ─── Stage ─── */

export async function stageFiles(paths: string[]): Promise<unknown> {
  return invoke("stage_files", { paths });
}

/* ─── Stash ─── */

export async function stashSave(message?: string): Promise<unknown> {
  return invoke("stash_save", { message: message ?? null });
}

export async function stashList(): Promise<StashListResponse> {
  return invoke<StashListResponse>("stash_list");
}

export async function stashPop(): Promise<unknown> {
  return invoke("stash_pop");
}

export async function stashApply(index?: number): Promise<unknown> {
  return invoke("stash_apply", { index: index ?? null });
}

export async function stashDrop(index?: number): Promise<unknown> {
  return invoke("stash_drop", { index: index ?? null });
}

/* ─── Undo ─── */

export async function undoList(): Promise<{ entries: OpLogEntry[] }> {
  return invoke("undo_list");
}

export async function undoUndo(): Promise<unknown> {
  return invoke("undo_undo");
}

export async function undoRedo(): Promise<unknown> {
  return invoke("undo_redo");
}

/* ─── Stack ─── */

export async function stackList(): Promise<{ stacks: StackEntry[] }> {
  return invoke("stack_list");
}

/* ─── Search ─── */

export async function searchCommits(query: string): Promise<SearchResponse> {
  return invoke<SearchResponse>("search_commits", { query });
}

/* ─── Amend / Reword ─── */

export async function amendCommit(message?: string): Promise<unknown> {
  return invoke("amend_commit", { message: message ?? null });
}

export async function rewordCommit(message: string): Promise<unknown> {
  return invoke("reword_commit", { message });
}
