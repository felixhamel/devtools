//! What the app remembers between launches (see [`Saved`]), and the mapping
//! between tool names (saved) and tool indices (used while running).
use crate::tools::Tool;

/// What the app remembers between launches, by tool name so that it
/// survives tools being added, renamed or reordered (unknown names are
/// dropped). The tools' inputs are never saved: they may hold tokens or
/// secrets. eframe also restores the window's size and position and the
/// sidebar's width (egui memory).
///
/// Stored as RON in `~/Library/Application Support/DevUtils/app.ron`.
/// `#[serde(default)]` lets state saved by an older version load too.
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub(crate) struct Saved {
    /// Name of the tool that was open, if any (otherwise the home screen).
    pub last_tool: Option<String>,
    /// Recently opened tools, most recent first.
    pub recent: Vec<String>,
    /// Starred tools, in the order they were starred.
    pub favorites: Vec<String>,
    /// Stored inverted so that a missing value means "shown".
    pub sidebar_hidden: bool,
}

impl Saved {
    /// The saved state, or the default one when there is none or it cannot be read.
    pub(crate) fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default()
    }

    /// Writes the state to `storage`; eframe flushes it to disk.
    pub(crate) fn store(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

/// Positions in `tools` of the tools named in `names`; unknown names are skipped.
pub(crate) fn indices(tools: &[Box<dyn Tool>], names: &[String]) -> Vec<usize> {
    names.iter().filter_map(|name| tools.iter().position(|t| t.name() == name)).collect()
}

/// Names of the tools at `positions`, to save them.
pub(crate) fn names(tools: &[Box<dyn Tool>], positions: &[usize]) -> Vec<String> {
    positions.iter().map(|&i| tools[i].name().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An in-memory stand-in for eframe's file storage.
    #[derive(Default)]
    struct MemoryStorage(std::collections::HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> { self.0.get(key).cloned() }
        fn set_string(&mut self, key: &str, value: String) { self.0.insert(key.to_owned(), value); }
        fn flush(&mut self) {}
    }

    #[test]
    fn saved_state_round_trips() {
        let mut storage = MemoryStorage::default();
        let saved = Saved { last_tool: Some("JWT Debugger".into()), recent: vec!["JWT Debugger".into()], favorites: vec!["IP Subnet Calculator".into()], sidebar_hidden: true };
        saved.store(&mut storage);
        let back = Saved::load(Some(&storage));
        assert_eq!((back.last_tool, back.favorites, back.sidebar_hidden), (saved.last_tool, saved.favorites, true));
        // State saved by an older version (fewer fields) still loads.
        eframe::Storage::set_string(&mut storage, eframe::APP_KEY, "(recent: [\"Hash Generator\"])".into());
        let old = Saved::load(Some(&storage));
        assert_eq!((old.recent, old.favorites.len(), old.sidebar_hidden), (vec!["Hash Generator".to_owned()], 0, false));
        assert_eq!(Saved::load(None).recent, Vec::<String>::new());
    }

    #[test]
    fn maps_names_to_tools_and_back() {
        let tools = crate::tools::registry();
        let positions = indices(&tools, &["Hash Generator".into(), "No Such Tool".into(), "JWT Debugger".into()]);
        assert_eq!(positions.len(), 2);
        assert_eq!(names(&tools, &positions), ["Hash Generator", "JWT Debugger"]);
    }
}
