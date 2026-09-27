use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum WindowId {
    Table(String),
    Surface(String),
    Search,
    MapFinder,
    HexEditor,
    Compare,
    CompareSurface,
    XdfEditor,
    ProjectNotepad,
    ActionHistory(String),
    DebugReport,
    Settings,
    NookLink,
}

impl WindowId {
    pub fn to_stable_id(&self) -> String {
        match self {
            Self::Table(key) => format!("table:{key}"),
            Self::Surface(key) => format!("surface:{key}"),
            Self::Search => "search".into(),
            Self::MapFinder => "map_finder".into(),
            Self::HexEditor => "hex_editor".into(),
            Self::Compare => "compare".into(),
            Self::CompareSurface => "compare_surface".into(),
            Self::XdfEditor => "xdf_editor".into(),
            Self::ProjectNotepad => "project_notepad".into(),
            Self::ActionHistory(key) => format!("history:{key}"),
            Self::DebugReport => "debug_report".into(),
            Self::Settings => "settings".into(),
            Self::NookLink => "nooklink".into(),
        }
    }

    pub fn from_stable_id(id: &str) -> Option<Self> {
        Some(match id {
            "search" => Self::Search,
            "map_finder" => Self::MapFinder,
            "hex_editor" => Self::HexEditor,
            "compare" => Self::Compare,
            "compare_surface" => Self::CompareSurface,
            "xdf_editor" => Self::XdfEditor,
            "project_notepad" => Self::ProjectNotepad,
            "debug_report" => Self::DebugReport,
            "settings" => Self::Settings,
            "nooklink" => Self::NookLink,
            _ => {
                let (kind, key) = id.split_once(':')?;
                if key.is_empty() {
                    return None;
                }
                match kind {
                    "table" => Self::Table(key.into()),
                    "surface" => Self::Surface(key.into()),
                    "history" => Self::ActionHistory(key.into()),
                    _ => return None,
                }
            }
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct WindowDockState {
    pub focused: Option<WindowId>,
    pub minimized: BTreeSet<WindowId>,
    pub focus_order: Vec<WindowId>,
    pub open_tool_windows: BTreeSet<WindowId>,
}

impl WindowDockState {
    pub fn focus(&mut self, id: WindowId) {
        self.minimized.remove(&id);
        self.focus_order.retain(|known| known != &id);
        self.focus_order.insert(0, id.clone());
        self.focused = Some(id);
    }

    pub fn focused(&self) -> Option<&WindowId> {
        self.focused.as_ref()
    }

    pub fn open_tool(&mut self, id: WindowId) {
        self.open_tool_windows.insert(id);
    }

    pub fn minimize(&mut self, id: &WindowId) {
        self.minimized.insert(id.clone());
        self.focus_order.retain(|known| known != id);
        if self.focused.as_ref() == Some(id) {
            self.focused = self.focus_order.first().cloned();
        }
    }

    pub fn restore(&mut self, id: &WindowId) {
        self.minimized.remove(id);
        self.focus(id.clone());
    }

    pub fn close(&mut self, id: &WindowId) {
        self.open_tool_windows.remove(id);
        self.minimized.remove(id);
        self.focus_order.retain(|known| known != id);
        if self.focused.as_ref() == Some(id) {
            self.focused = self.focus_order.first().cloned();
        }
    }

    pub fn is_minimized(&self, id: &WindowId) -> bool {
        self.minimized.contains(id)
    }

    pub fn is_open_tool(&self, id: &WindowId) -> bool {
        self.open_tool_windows.contains(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focusing_minimizing_restoring_and_closing_tracks_recent_target() {
        let table = WindowId::Table("xdf|table:a".into());
        let nooklink = WindowId::NookLink;
        assert_eq!(
            WindowId::from_stable_id(&table.to_stable_id()),
            Some(table.clone())
        );
        let mut state = WindowDockState::default();
        state.focus(table.clone());
        state.open_tool(nooklink.clone());
        state.focus(nooklink.clone());
        state.minimize(&nooklink);
        assert!(state.is_minimized(&nooklink));
        assert_eq!(state.focused(), Some(&table));
        state.restore(&nooklink);
        assert_eq!(state.focused(), Some(&nooklink));
        state.close(&nooklink);
        assert_eq!(state.focused(), Some(&table));
        assert!(!state.is_open_tool(&nooklink));
    }

    #[test]
    fn stable_window_ids_preserve_colons_in_keys_and_state_round_trips() {
        let ids = [
            WindowId::Table("xdf|table:a:b".into()),
            WindowId::Surface("xdf|surface:c:d".into()),
            WindowId::ActionHistory("xdf|history:e:f".into()),
            WindowId::Search,
            WindowId::MapFinder,
            WindowId::HexEditor,
            WindowId::Compare,
            WindowId::CompareSurface,
            WindowId::XdfEditor,
            WindowId::ProjectNotepad,
            WindowId::DebugReport,
            WindowId::Settings,
            WindowId::NookLink,
        ];
        for id in ids {
            let stable = id.to_stable_id();
            assert_eq!(WindowId::from_stable_id(&stable), Some(id));
        }

        let state = WindowDockState {
            focused: Some(WindowId::NookLink),
            minimized: BTreeSet::from([WindowId::Search]),
            focus_order: vec![WindowId::NookLink, WindowId::Table("table:1".into())],
            open_tool_windows: BTreeSet::from([WindowId::NookLink]),
        };
        let encoded = serde_json::to_string(&state).unwrap();
        assert_eq!(
            serde_json::from_str::<WindowDockState>(&encoded).unwrap(),
            state
        );
        assert_eq!(WindowId::from_stable_id("unknown:x"), None);
    }
}
