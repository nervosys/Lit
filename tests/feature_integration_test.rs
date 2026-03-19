/// Working Integration Tests for Lit Features
/// Tests core functionality with correct API usage
use std::fs;
use tempfile::TempDir;

// Helper function to create a test repository
fn create_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path();

    // Create .lit directory structure
    fs::create_dir_all(repo_path.join(".lit/objects")).unwrap();
    fs::create_dir_all(repo_path.join(".lit/refs/heads")).unwrap();
    fs::create_dir_all(repo_path.join(".lit/refs/remotes")).unwrap();

    // Create HEAD
    fs::write(repo_path.join(".lit/HEAD"), "ref: refs/heads/main\n").unwrap();

    // Create minimal encryption config
    fs::write(
        repo_path.join(".lit/encryption.toml"),
        "enabled = false\nkey_file = \"~/.lit/encryption.key\"\nfips_mode = true\n",
    )
    .unwrap();

    temp
}

#[cfg(test)]
mod core_objects_tests {

    use lit::core::{Blob, Commit, Object, ObjectHash, Tree};

    #[test]
    fn test_blob_creation() {
        let content = b"Hello, World!".to_vec();
        let blob = Blob::new(content.clone());
        assert_eq!(blob.content, content);
    }

    #[test]
    fn test_blob_in_object() {
        let content = b"Test content".to_vec();
        let blob = Blob::new(content.clone());
        let obj = Object::Blob(blob);

        match obj {
            Object::Blob(b) => assert_eq!(b.content, content),
            _ => panic!("Expected Blob"),
        }
    }

    #[test]
    fn test_object_hash() {
        let blob = Blob::new(b"test".to_vec());
        let obj = Object::Blob(blob);
        let hash = obj.hash();

        // Hash should be 192 hex chars (SHA3-512 + BLAKE3 = 96 bytes)
        assert_eq!(hash.len(), 192);
    }

    #[test]
    fn test_object_hash_from_bytes() {
        let bytes = vec![0xAB; 96]; // 96 bytes for combined hash

        let hash = ObjectHash::from_bytes(&bytes);
        let hash_str = hash.as_str();

        // Verify correct length
        assert_eq!(hash_str.len(), 192); // 96 bytes = 192 hex chars

        // Verify it's valid hex
        assert!(hash_str.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn test_object_hash_from_hex() {
        let hex = "a".repeat(192);
        let hash = ObjectHash::from_hex(hex.clone());
        assert_eq!(hash.as_str(), &hex);
    }

    #[test]
    fn test_object_hash_short() {
        let hex = "abcdef0123456789".to_string() + &"0".repeat(176);
        let hash = ObjectHash::from_hex(hex);
        let short = hash.short();

        assert_eq!(short.len(), 16);
        assert_eq!(short, "abcdef0123456789");
    }

    #[test]
    fn test_tree_creation() {
        let tree = Tree::new();
        assert_eq!(tree.entries.len(), 0);
    }

    #[test]
    fn test_commit_struct() {
        let tree_hash = ObjectHash::from_hex("a".repeat(192));
        let commit = Commit {
            tree: tree_hash.clone(),
            parents: vec![],
            author: "Test Author".to_string(),
            committer: "Test Author".to_string(),
            timestamp: 1234567890,
            message: "Initial commit".to_string(),
            pq_signature: None,
            metadata: None,
        };

        assert_eq!(commit.tree.as_str(), tree_hash.as_str());
        assert_eq!(commit.author, "Test Author");
        assert_eq!(commit.message, "Initial commit");
    }

    #[test]
    fn test_commit_with_parents() {
        let tree_hash = ObjectHash::from_hex("a".repeat(192));
        let parent1 = ObjectHash::from_hex("b".repeat(192));
        let parent2 = ObjectHash::from_hex("c".repeat(192));

        let commit = Commit {
            tree: tree_hash,
            parents: vec![parent1.clone(), parent2.clone()],
            author: "Merger".to_string(),
            committer: "Merger".to_string(),
            timestamp: 1234567890,
            message: "Merge commit".to_string(),
            pq_signature: None,
            metadata: None,
        };

        assert_eq!(commit.parents.len(), 2);
    }

    #[test]
    fn test_object_to_bytes() {
        let blob = Blob::new(b"test data".to_vec());
        let obj = Object::Blob(blob);
        let bytes = obj.to_bytes();

        assert!(!bytes.is_empty());
    }
}

#[cfg(test)]
mod core_refs_tests {
    use super::*;
    use lit::core::refs::*;

    #[test]
    fn test_write_and_read_ref() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash = "a".repeat(192);
        write_ref(repo_path, "heads/main", &hash).unwrap();

        let read_hash = read_ref(repo_path, "heads/main").unwrap();
        assert_eq!(read_hash, hash);
    }

    #[test]
    fn test_ref_update() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash1 = "a".repeat(192);
        let hash2 = "b".repeat(192);

        write_ref(repo_path, "heads/test", &hash1).unwrap();
        let read1 = read_ref(repo_path, "heads/test").unwrap();
        assert_eq!(read1, hash1);

        write_ref(repo_path, "heads/test", &hash2).unwrap();
        let read2 = read_ref(repo_path, "heads/test").unwrap();
        assert_eq!(read2, hash2);
    }

    #[test]
    fn test_delete_ref() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash = "c".repeat(192);
        write_ref(repo_path, "heads/temp", &hash).unwrap();
        assert!(repo_path.join(".lit/refs/heads/temp").exists());

        delete_ref(repo_path, "heads/temp").unwrap();
        assert!(!repo_path.join(".lit/refs/heads/temp").exists());
    }

    #[test]
    fn test_update_head() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        update_head(repo_path, "main").unwrap();

        let head_content = fs::read_to_string(repo_path.join(".lit/HEAD")).unwrap();
        assert!(head_content.contains("ref: refs/heads/main"));
    }

    #[test]
    fn test_get_current_branch() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        update_head(repo_path, "feature").unwrap();
        let branch = get_current_branch(repo_path).unwrap();
        assert_eq!(branch, "feature");
    }

    #[test]
    fn test_detached_head() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash = "d".repeat(192);
        set_head_detached(repo_path, &hash).unwrap();

        // Should error when trying to get current branch
        assert!(get_current_branch(repo_path).is_err());
    }

    #[test]
    fn test_list_refs() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash = "e".repeat(192);
        write_ref(repo_path, "heads/main", &hash).unwrap();
        write_ref(repo_path, "heads/feature", &hash).unwrap();
        write_ref(repo_path, "heads/develop", &hash).unwrap();

        let refs = list_refs(repo_path, "heads").unwrap();
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_read_head_symbolic() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        // Write a branch ref
        let hash = "abc123".to_string() + &"0".repeat(122);
        write_ref(repo_path, "heads/main", &hash).unwrap();

        // Point HEAD to it
        update_head(repo_path, "main").unwrap();

        // Read HEAD should resolve to the commit hash
        let head_hash = read_head(repo_path).unwrap();
        assert_eq!(head_hash, hash);
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use lit::core::{Blob, Object};
    use lit::storage::index::Index;
    use lit::storage::objects::ObjectStore;

    #[test]
    fn test_index_creation() {
        let index = Index::new();
        assert_eq!(index.entries.len(), 0);
    }

    #[test]
    fn test_index_add_entry() {
        let mut index = Index::new();

        let hash = "1".repeat(192);
        index.add("test.txt".to_string(), hash.clone(), "100644".to_string());

        assert_eq!(index.entries.len(), 1);
        assert!(index.entries.contains_key("test.txt"));
    }

    #[test]
    fn test_index_remove_entry() {
        let mut index = Index::new();

        index.add(
            "file.txt".to_string(),
            "abc".to_string(),
            "100644".to_string(),
        );
        assert_eq!(index.entries.len(), 1);

        let removed = index.remove("file.txt");
        assert!(removed.is_some());
        assert_eq!(index.entries.len(), 0);
    }

    #[test]
    fn test_index_clear() {
        let mut index = Index::new();

        index.add(
            "file1.txt".to_string(),
            "a".to_string(),
            "100644".to_string(),
        );
        index.add(
            "file2.txt".to_string(),
            "b".to_string(),
            "100644".to_string(),
        );
        assert_eq!(index.entries.len(), 2);

        index.clear();
        assert_eq!(index.entries.len(), 0);
    }

    #[test]
    fn test_index_save_and_load() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let mut index = Index::new();
        index.add(
            "file1.txt".to_string(),
            "a".repeat(192),
            "100644".to_string(),
        );
        index.add(
            "file2.txt".to_string(),
            "b".repeat(192),
            "100755".to_string(),
        );

        index.save(repo_path).unwrap();

        let loaded = Index::load(repo_path).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert!(loaded.entries.contains_key("file1.txt"));
        assert!(loaded.entries.contains_key("file2.txt"));
    }

    #[test]
    fn test_object_store_write_and_read() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let store = ObjectStore::new(repo_path);

        let content = b"Hello, world!".to_vec();
        let blob = Blob::new(content.clone());
        let object = Object::Blob(blob);

        let hash = store.write(&object).unwrap();

        let read_object = store.read(&hash).unwrap();

        match read_object {
            Object::Blob(blob) => assert_eq!(blob.content, content),
            _ => panic!("Expected blob"),
        }
    }

    #[test]
    fn test_object_store_exists() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let store = ObjectStore::new(repo_path);

        let blob = Blob::new(b"test".to_vec());
        let object = Object::Blob(blob);

        let hash = store.write(&object).unwrap();

        assert!(store.exists(&hash));
    }
}

#[cfg(test)]
mod command_integration_tests {
    use super::*;

    #[test]
    fn test_is_lit_repo() {
        let temp = create_test_repo();
        assert!(lit::core::refs::is_lit_repo(temp.path()));

        let non_repo = TempDir::new().unwrap();
        assert!(!lit::core::refs::is_lit_repo(non_repo.path()));
    }

    #[test]
    fn test_get_lit_dir() {
        let temp = create_test_repo();
        let lit_dir = lit::core::refs::get_lit_dir(temp.path());

        assert_eq!(lit_dir, temp.path().join(".lit"));
        assert!(lit_dir.exists());
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use lit::core::refs::*;

    #[test]
    fn test_read_nonexistent_ref() {
        let temp = create_test_repo();
        let result = read_ref(temp.path(), "heads/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_nonexistent_ref() {
        let temp = create_test_repo();
        let result = delete_ref(temp.path(), "heads/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_branch_when_detached() {
        let temp = create_test_repo();
        let repo_path = temp.path();

        let hash = "x".repeat(192);
        set_head_detached(repo_path, &hash).unwrap();

        let result = get_current_branch(repo_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("detached"));
    }
}

// ============================================================================
// Schema Generation Tests
// ============================================================================

#[cfg(test)]
mod schema_tests {
    use lit::ontology;

    #[test]
    fn test_generate_schemas_has_required_fields() {
        let schema = ontology::generate_schemas();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["$id"], "https://lit-vcs.dev/schema/v1");
        assert_eq!(schema["title"], "Lit VCS Schema");
        assert!(schema["$defs"].is_object(), "$defs must be an object");
        assert!(schema["commands"].is_object(), "commands must be an object");
    }

    #[test]
    fn test_generate_schemas_has_types() {
        let schema = ontology::generate_schemas();
        let defs = schema["$defs"].as_object().unwrap();
        // Ontology defines core types like ObjectHash, Commit, etc.
        assert!(!defs.is_empty(), "must have at least one type def");
        // Each type def should have "type": "object"
        for (_name, def) in defs {
            assert_eq!(def["type"], "object");
            assert!(def["properties"].is_object());
        }
    }

    #[test]
    fn test_generate_schemas_has_commands() {
        let schema = ontology::generate_schemas();
        let commands = schema["commands"].as_object().unwrap();
        // Must include core commands
        assert!(commands.contains_key("commit"), "missing commit command");
        assert!(commands.contains_key("status"), "missing status command");
        assert!(commands.contains_key("add"), "missing add command");
        assert!(commands.contains_key("branch"), "missing branch command");
        // Each command must have description and input schema
        for (_id, cmd) in commands {
            assert!(cmd["description"].is_string());
            assert!(cmd["input"].is_object());
            assert_eq!(cmd["input"]["type"], "object");
        }
    }

    #[test]
    fn test_generate_command_schema_known_command() {
        let schema = ontology::generate_command_schema("commit");
        assert!(schema.is_some(), "commit schema should exist");
        let schema = schema.unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(
            schema["$id"],
            "https://lit-vcs.dev/schema/v1/commands/commit"
        );
        assert!(schema["input"]["properties"].is_object());
        let props = schema["input"]["properties"].as_object().unwrap();
        assert!(
            props.contains_key("message"),
            "commit must have message param"
        );
    }

    #[test]
    fn test_generate_command_schema_unknown_returns_none() {
        let schema = ontology::generate_command_schema("nonexistent_command_xyz");
        assert!(schema.is_none());
    }

    #[test]
    fn test_command_schema_required_fields() {
        let schema = ontology::generate_command_schema("commit").unwrap();
        let required = schema["input"]["required"].as_array().unwrap();
        let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            required_names.contains(&"message"),
            "commit message should be required"
        );
    }

    #[test]
    fn test_schema_param_types_are_valid() {
        let schema = ontology::generate_schemas();
        let commands = schema["commands"].as_object().unwrap();
        let valid_types = ["string", "boolean", "integer", "number", "array", "object"];
        for (_id, cmd) in commands {
            if let Some(props) = cmd["input"]["properties"].as_object() {
                for (_name, prop) in props {
                    if let Some(ty) = prop["type"].as_str() {
                        assert!(
                            valid_types.contains(&ty),
                            "invalid type '{}' in command '{}'",
                            ty,
                            _id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_all_ontology_commands_in_schema() {
        let ont = ontology::get_ontology();
        let schema = ontology::generate_schemas();
        let commands = schema["commands"].as_object().unwrap();
        for cmd in &ont.commands {
            assert!(
                commands.contains_key(&cmd.id),
                "ontology command '{}' missing from schema",
                cmd.id
            );
        }
    }
}
