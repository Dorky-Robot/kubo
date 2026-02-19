use std::fs;
use std::path::{Path, PathBuf};

use crate::chain::ActionChain;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("toml deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
}

type Result<T> = std::result::Result<T, CatalogError>;

/// Filesystem-backed catalog of action chains stored as TOML files.
pub struct Catalog {
    dir: PathBuf,
}

impl Catalog {
    /// Open the default catalog at `~/.kubo/chains/`, creating the directory if needed.
    pub fn open() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let dir = PathBuf::from(home).join(".kubo").join("chains");
        Self::open_at(&dir)
    }

    /// Open a catalog at a specific directory (useful for testing).
    pub fn open_at(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Save a chain to disk. Returns the path of the written file.
    pub fn save(&self, chain: &ActionChain) -> Result<PathBuf> {
        let path = self
            .dir
            .join(format!("{}.toml", slugify(&chain.chain.name)));
        let toml_str = toml::to_string_pretty(chain)?;
        fs::write(&path, toml_str)?;
        Ok(path)
    }

    /// Load a chain by name. Returns `None` if not found.
    pub fn load(&self, name: &str) -> Result<Option<ActionChain>> {
        let path = self.dir.join(format!("{}.toml", slugify(name)));
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path)?;
        let chain: ActionChain = toml::from_str(&contents)?;
        Ok(Some(chain))
    }

    /// List all chains in the catalog.
    pub fn list(&self) -> Result<Vec<ActionChain>> {
        let mut chains = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                let contents = fs::read_to_string(&path)?;
                if let Ok(chain) = toml::from_str::<ActionChain>(&contents) {
                    chains.push(chain);
                }
            }
        }
        chains.sort_by(|a, b| a.chain.name.cmp(&b.chain.name));
        Ok(chains)
    }

    /// Delete a chain by name. Returns `true` if the file existed and was removed.
    pub fn delete(&self, name: &str) -> Result<bool> {
        let path = self.dir.join(format!("{}.toml", slugify(name)));
        if path.exists() {
            fs::remove_file(&path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Search chains by keyword intersection on name + intent.
    /// Words <= 2 chars are skipped. Results scored by overlap count.
    pub fn search(&self, query: &str) -> Result<Vec<ActionChain>> {
        let query_words: Vec<String> = tokenize(query);
        if query_words.is_empty() {
            return self.list();
        }

        let all_chains = self.list()?;
        let mut scored: Vec<(usize, ActionChain)> = all_chains
            .into_iter()
            .filter_map(|chain| {
                let chain_words = {
                    let mut w = tokenize(&chain.chain.name);
                    w.extend(tokenize(&chain.chain.intent));
                    w.extend(chain.chain.tags.iter().flat_map(|t| tokenize(t)));
                    w
                };
                let score = query_words
                    .iter()
                    .filter(|qw| chain_words.iter().any(|cw| cw.contains(qw.as_str())))
                    .count();
                if score > 0 {
                    Some((score, chain))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(scored.into_iter().map(|(_, chain)| chain).collect())
    }
}

/// Convert a name to a filesystem-safe slug: lowercase, hyphens, collapse specials.
fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive hyphens and trim
    let mut result = String::new();
    for c in slug.chars() {
        if c == '-' && result.ends_with('-') {
            continue;
        }
        result.push(c);
    }
    result.trim_matches('-').to_string()
}

/// Tokenize text into lowercase words, skipping words <= 2 chars.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{ActionChain, ChainMeta};
    use crate::stage::Stage;
    use tempfile::TempDir;

    fn sample_chain(name: &str, intent: &str) -> ActionChain {
        ActionChain {
            chain: ChainMeta {
                name: name.into(),
                intent: intent.into(),
                created_at: "2026-02-19T10:30:00Z".parse().unwrap(),
                tags: vec!["test".into()],
            },
            stages: vec![Stage::Shell {
                command: "echo hello".into(),
            }],
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_at(tmp.path()).unwrap();
        let chain = sample_chain("plan-dinner", "what should we get for dinner?");

        let path = cat.save(&chain).unwrap();
        assert!(path.exists());

        let loaded = cat.load("plan-dinner").unwrap().unwrap();
        assert_eq!(loaded.chain.name, "plan-dinner");
        assert_eq!(loaded.stages.len(), 1);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_at(tmp.path()).unwrap();
        assert!(cat.load("nope").unwrap().is_none());
    }

    #[test]
    fn list_returns_all_chains() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_at(tmp.path()).unwrap();

        cat.save(&sample_chain("alpha", "first")).unwrap();
        cat.save(&sample_chain("beta", "second")).unwrap();

        let chains = cat.list().unwrap();
        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0].chain.name, "alpha");
        assert_eq!(chains[1].chain.name, "beta");
    }

    #[test]
    fn delete_chain() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_at(tmp.path()).unwrap();
        cat.save(&sample_chain("doomed", "to be deleted")).unwrap();

        assert!(cat.delete("doomed").unwrap());
        assert!(!cat.delete("doomed").unwrap());
        assert!(cat.load("doomed").unwrap().is_none());
    }

    #[test]
    fn search_matching() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_at(tmp.path()).unwrap();

        cat.save(&sample_chain(
            "plan-dinner",
            "what should we get for dinner?",
        ))
        .unwrap();
        cat.save(&sample_chain("plan-trip", "plan a vacation trip"))
            .unwrap();
        cat.save(&sample_chain("budget", "track monthly expenses"))
            .unwrap();

        let results = cat.search("dinner food").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chain.name, "plan-dinner");

        let results = cat.search("plan").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("Plan Dinner!"), "plan-dinner");
        assert_eq!(slugify("  hello---world  "), "hello-world");
        assert_eq!(slugify("UPPER_case"), "upper-case");
    }
}
