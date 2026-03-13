pub mod diff;
pub mod merge;
pub mod objects;
pub mod refs;

pub use objects::{Blob, Commit, Object, ObjectHash, Tag, Tree, TreeEntry};
pub use refs::{
    delete_ref, find_repo_root, get_current_branch, list_refs, read_head, read_ref,
    set_head_detached, update_head, write_ref,
};
