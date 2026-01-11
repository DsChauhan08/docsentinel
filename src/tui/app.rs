//! TUI application state and logic

use crate::drift::{DriftEvent, DriftSeverity};
use crate::repo::Repository;
use crate::storage::Database;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::{Path, PathBuf};

/// Current view in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Main dashboard
    Dashboard,
    /// List of drift issues
    Issues,
    /// Detailed issue view
    IssueDetail,
    /// Fix editor
    FixEditor,
    /// Documentation browser
    Docs,
    /// Help screen
    Help,
}

/// Application state
pub struct AppState {
    /// Current view
    pub view: View,
    /// Selected issue index
    pub selected_issue: usize,
    /// Selected doc/symbol index
    pub selected_doc: usize,
    /// Scroll offset for lists
    pub scroll_offset: usize,
    /// Input buffer for editing
    pub input_buffer: String,
    /// Whether in input mode
    pub input_mode: bool,
    /// Search query for docs
    pub search_query: String,
    /// Status message
    pub status_message: Option<String>,
    /// Confirmation dialog
    pub confirm_dialog: Option<ConfirmDialog>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: View::Dashboard,
            selected_issue: 0,
            selected_doc: 0,
            scroll_offset: 0,
            input_buffer: String::new(),
            input_mode: false,
            search_query: String::new(),
            status_message: None,
            confirm_dialog: None,
        }
    }
}

/// Confirmation dialog
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub on_confirm: Box<dyn FnOnce(&mut App) -> Result<()>>,
}

/// Main TUI application
pub struct App {
    /// Repository path
    pub repo_path: PathBuf,
    /// Repository handle
    pub repo: Repository,
    /// Database handle
    pub db: Database,
    /// Application state
    pub state: AppState,
    /// Drift events
    pub events: Vec<DriftEvent>,
    /// Code chunks for docs browser
    pub code_chunks: Vec<crate::extract::CodeChunk>,
    /// Database statistics
    pub stats: crate::storage::DatabaseStats,
}

impl App {
    /// Create a new app instance
    pub fn new(path: &Path) -> Result<Self> {
        let repo = Repository::open(path)?;
        let sentinel_dir = repo.sentinel_dir();

        if !sentinel_dir.exists() {
            anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
        }

        let db_path = sentinel_dir.join("docsentinel.db");
        let db = Database::open(&db_path)?;

        let events = db.get_unresolved_drift_events()?;
        let code_chunks = db.get_all_code_chunks().unwrap_or_default();
        let stats = db.get_stats()?;

        Ok(Self {
            repo_path: path.to_path_buf(),
            repo,
            db,
            state: AppState::default(),
            events,
            code_chunks,
            stats,
        })
    }

    /// Handle a key event
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle confirmation dialog first
        if self.state.confirm_dialog.is_some() {
            return self.handle_confirm_key(key);
        }

        // Handle input mode
        if self.state.input_mode {
            return self.handle_input_key(key);
        }

        // Handle view-specific keys
        match self.state.view {
            View::Dashboard => self.handle_dashboard_key(key),
            View::Issues => self.handle_issues_key(key),
            View::IssueDetail => self.handle_detail_key(key),
            View::FixEditor => self.handle_editor_key(key),
            View::Docs => self.handle_docs_key(key),
            View::Help => self.handle_help_key(key),
        }
    }

    /// Handle keys in dashboard view
    fn handle_dashboard_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('i') | KeyCode::Enter => {
                self.state.view = View::Issues;
            }
            KeyCode::Char('d') => {
                self.state.view = View::Docs;
                self.state.selected_doc = 0;
            }
            KeyCode::Char('s') => {
                self.run_scan()?;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.state.view = View::Help;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in issues view
    fn handle_issues_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.view = View::Dashboard;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.selected_issue > 0 {
                    self.state.selected_issue -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.selected_issue < self.events.len().saturating_sub(1) {
                    self.state.selected_issue += 1;
                }
            }
            KeyCode::Enter => {
                if !self.events.is_empty() {
                    self.state.view = View::IssueDetail;
                }
            }
            KeyCode::Char('f') => {
                if !self.events.is_empty() {
                    self.state.view = View::FixEditor;
                }
            }
            KeyCode::Char('x') => {
                self.ignore_selected()?;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.state.view = View::Help;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in detail view
    fn handle_detail_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.view = View::Issues;
            }
            KeyCode::Char('f') => {
                self.state.view = View::FixEditor;
            }
            KeyCode::Char('x') => {
                self.ignore_selected()?;
                self.state.view = View::Issues;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.scroll_offset > 0 {
                    self.state.scroll_offset -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.scroll_offset += 1;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in fix editor
    fn handle_editor_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.state.view = View::IssueDetail;
                self.state.input_mode = false;
            }
            KeyCode::Char('e') if !self.state.input_mode => {
                self.state.input_mode = true;
                if let Some(event) = self.events.get(self.state.selected_issue) {
                    self.state.input_buffer = event.suggested_fix.clone().unwrap_or_default();
                }
            }
            KeyCode::Char('a') if !self.state.input_mode => {
                self.apply_fix()?;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in help view
    fn handle_help_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                self.state.view = View::Dashboard;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in docs browser view
    fn handle_docs_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Filter chunks based on search query
        let filtered_chunks: Vec<_> = if self.state.search_query.is_empty() {
            self.code_chunks.iter().filter(|c| c.is_public).collect()
        } else {
            let query = self.state.search_query.to_lowercase();
            self.code_chunks
                .iter()
                .filter(|c| c.is_public)
                .filter(|c| {
                    c.symbol_name.to_lowercase().contains(&query)
                        || c.file_path.to_lowercase().contains(&query)
                })
                .collect()
        };

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.state.input_mode {
                    self.state.input_mode = false;
                } else {
                    self.state.view = View::Dashboard;
                    self.state.search_query.clear();
                }
            }
            KeyCode::Up | KeyCode::Char('k') if !self.state.input_mode => {
                if self.state.selected_doc > 0 {
                    self.state.selected_doc -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !self.state.input_mode => {
                if self.state.selected_doc < filtered_chunks.len().saturating_sub(1) {
                    self.state.selected_doc += 1;
                }
            }
            KeyCode::Char('/') if !self.state.input_mode => {
                self.state.input_mode = true;
            }
            KeyCode::Enter if self.state.input_mode => {
                self.state.input_mode = false;
                self.state.selected_doc = 0;
            }
            KeyCode::Backspace if self.state.input_mode => {
                self.state.search_query.pop();
            }
            KeyCode::Char(c) if self.state.input_mode => {
                self.state.search_query.push(c);
            }
            KeyCode::Char('g') if !self.state.input_mode => {
                self.state.selected_doc = 0;
            }
            KeyCode::Char('G') if !self.state.input_mode => {
                self.state.selected_doc = filtered_chunks.len().saturating_sub(1);
            }
            _ => {}
        }
        Ok(false)
    }


    /// Handle keys in input mode
    fn handle_input_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = false;
            }
            KeyCode::Enter => {
                self.state.input_mode = false;
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.state.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle keys in confirmation dialog
    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                if let Some(dialog) = self.state.confirm_dialog.take() {
                    (dialog.on_confirm)(self)?;
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.state.confirm_dialog = None;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Run a scan
    fn run_scan(&mut self) -> Result<()> {
        self.state.status_message = Some("Scanning...".to_string());

        // Run scan
        let events = crate::cli::scan(&self.repo_path, false, None, true)?;

        // Refresh data
        self.events = self.db.get_unresolved_drift_events()?;
        self.stats = self.db.get_stats()?;

        self.state.status_message = Some(format!("Scan complete. {} issues found.", events.len()));

        Ok(())
    }

    /// Ignore the selected issue
    fn ignore_selected(&mut self) -> Result<()> {
        if let Some(event) = self.events.get(self.state.selected_issue) {
            self.db.update_drift_event_status(&event.id, "Ignored")?;
            self.events = self.db.get_unresolved_drift_events()?;
            self.stats = self.db.get_stats()?;

            if self.state.selected_issue >= self.events.len() && self.state.selected_issue > 0 {
                self.state.selected_issue -= 1;
            }

            self.state.status_message = Some("Issue ignored".to_string());
        }
        Ok(())
    }

    /// Apply fix to selected issue
    fn apply_fix(&mut self) -> Result<()> {
        if let Some(event) = self.events.get(self.state.selected_issue) {
            let fix_content = if !self.state.input_buffer.is_empty() {
                Some(self.state.input_buffer.as_str())
            } else {
                event.suggested_fix.as_deref()
            };

            if let Some(content) = fix_content {
                crate::cli::fix(&self.repo_path, &event.id, Some(content), false)?;

                self.events = self.db.get_unresolved_drift_events()?;
                self.stats = self.db.get_stats()?;

                if self.state.selected_issue >= self.events.len() && self.state.selected_issue > 0 {
                    self.state.selected_issue -= 1;
                }

                self.state.status_message = Some("Fix applied".to_string());
                self.state.view = View::Issues;
            } else {
                self.state.status_message = Some("No fix content available".to_string());
            }
        }
        Ok(())
    }

    /// Get the currently selected event
    pub fn selected_event(&self) -> Option<&DriftEvent> {
        self.events.get(self.state.selected_issue)
    }

    /// Get severity color
    pub fn severity_color(severity: DriftSeverity) -> ratatui::style::Color {
        use ratatui::style::Color;
        match severity {
            DriftSeverity::Critical => Color::Red,
            DriftSeverity::High => Color::LightRed,
            DriftSeverity::Medium => Color::Yellow,
            DriftSeverity::Low => Color::Green,
        }
    }
}
