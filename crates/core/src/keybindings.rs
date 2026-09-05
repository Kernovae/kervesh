use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyAction {
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    FocusNextPane,
    FocusPrevPane,
    ProcessViewer,
    SnippetsLibrary,
    MultiExec,
    FindInTerminal,
    NewSession,
    NextTab,
    PrevTab,
}

impl KeyAction {
    pub fn description(&self) -> &'static str {
        match self {
            Self::SplitVertical => "Split Pane Vertically",
            Self::SplitHorizontal => "Split Pane Horizontally",
            Self::ClosePane => "Close Active Split Pane",
            Self::FocusNextPane => "Focus Next Pane",
            Self::FocusPrevPane => "Focus Previous Pane",
            Self::ProcessViewer => "Open Process Viewer",
            Self::SnippetsLibrary => "Open Snippets Library",
            Self::MultiExec => "Toggle Multi-Exec Broadcast",
            Self::FindInTerminal => "Find in Terminal Scrollback",
            Self::NewSession => "Open New Host Connection",
            Self::NextTab => "Switch to Next Tab",
            Self::PrevTab => "Switch to Previous Tab",
        }
    }

    pub fn default_shortcut(&self) -> &'static str {
        match self {
            Self::SplitVertical => "Ctrl+Shift+D",
            Self::SplitHorizontal => "Ctrl+Shift+E",
            Self::ClosePane => "Ctrl+Shift+W",
            Self::FocusNextPane => "Alt+Right",
            Self::FocusPrevPane => "Alt+Left",
            Self::ProcessViewer => "Ctrl+Shift+P",
            Self::SnippetsLibrary => "Ctrl+Shift+K",
            Self::MultiExec => "Ctrl+Shift+B",
            Self::FindInTerminal => "Ctrl+Shift+F",
            Self::NewSession => "Ctrl+T",
            Self::NextTab => "Ctrl+PageDown",
            Self::PrevTab => "Ctrl+PageUp",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub action: KeyAction,
    pub shortcut: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyBindingsConfig {
    pub bindings: Vec<KeyBinding>,
}

impl Default for KeyBindingsConfig {
    fn default() -> Self {
        Self {
            bindings: vec![
                KeyBinding {
                    action: KeyAction::SplitVertical,
                    shortcut: KeyAction::SplitVertical.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::SplitHorizontal,
                    shortcut: KeyAction::SplitHorizontal.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::ClosePane,
                    shortcut: KeyAction::ClosePane.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::FocusNextPane,
                    shortcut: KeyAction::FocusNextPane.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::FocusPrevPane,
                    shortcut: KeyAction::FocusPrevPane.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::ProcessViewer,
                    shortcut: KeyAction::ProcessViewer.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::SnippetsLibrary,
                    shortcut: KeyAction::SnippetsLibrary.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::MultiExec,
                    shortcut: KeyAction::MultiExec.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::FindInTerminal,
                    shortcut: KeyAction::FindInTerminal.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::NewSession,
                    shortcut: KeyAction::NewSession.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::NextTab,
                    shortcut: KeyAction::NextTab.default_shortcut().into(),
                },
                KeyBinding {
                    action: KeyAction::PrevTab,
                    shortcut: KeyAction::PrevTab.default_shortcut().into(),
                },
            ],
        }
    }
}

impl KeyBindingsConfig {
    pub fn get_shortcut(&self, action: KeyAction) -> &str {
        self.bindings
            .iter()
            .find(|b| b.action == action)
            .map(|b| b.shortcut.as_str())
            .unwrap_or_else(|| action.default_shortcut())
    }

    pub fn set_shortcut(&mut self, action: KeyAction, shortcut: String) {
        if let Some(b) = self.bindings.iter_mut().find(|b| b.action == action) {
            b.shortcut = shortcut;
        } else {
            self.bindings.push(KeyBinding { action, shortcut });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keybindings() {
        let config = KeyBindingsConfig::default();
        assert_eq!(
            config.get_shortcut(KeyAction::SplitVertical),
            "Ctrl+Shift+D"
        );
        assert_eq!(
            config.get_shortcut(KeyAction::ProcessViewer),
            "Ctrl+Shift+P"
        );
    }
}
