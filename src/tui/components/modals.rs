//! Modal/Popup Components
//!
//! This module provides modal and popup dialog components for confirmations,
//! details display, and user input overlays.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

/// Modal types for different use cases
#[derive(Debug, Clone)]
pub enum ModalType {
    Confirmation {
        title: String,
        message: String,
        confirm_text: String,
        cancel_text: String,
    },
    Information {
        title: String,
        content: Vec<String>,
    },
    Error {
        title: String,
        error_message: String,
        details: Option<Vec<String>>,
    },
    TransactionDetails {
        tx_hash: String,
        status: String,
        details: Vec<(String, String)>,
    },
    Help {
        title: String,
        sections: Vec<HelpSection>,
    },
}

/// Help section for help modal
#[derive(Debug, Clone)]
pub struct HelpSection {
    pub title: String,
    pub items: Vec<(String, String)>, // (key, description) pairs
}

/// Modal state
#[derive(Debug, Clone)]
pub struct ModalState {
    pub modal_type: ModalType,
    pub is_visible: bool,
    pub selected_option: usize, // For confirmation dialogs
}

impl ModalState {
    /// Create a new confirmation modal
    pub fn confirmation(
        title: String,
        message: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    ) -> Self {
        Self {
            modal_type: ModalType::Confirmation {
                title,
                message,
                confirm_text: confirm_text.unwrap_or_else(|| "Yes".to_string()),
                cancel_text: cancel_text.unwrap_or_else(|| "No".to_string()),
            },
            is_visible: true,
            selected_option: 0,
        }
    }

    /// Create a new information modal
    pub fn information(title: String, content: Vec<String>) -> Self {
        Self {
            modal_type: ModalType::Information { title, content },
            is_visible: true,
            selected_option: 0,
        }
    }

    /// Create a new error modal
    pub fn error(title: String, error_message: String, details: Option<Vec<String>>) -> Self {
        Self {
            modal_type: ModalType::Error {
                title,
                error_message,
                details,
            },
            is_visible: true,
            selected_option: 0,
        }
    }

    /// Create a new transaction details modal
    pub fn transaction_details(
        tx_hash: String,
        status: String,
        details: Vec<(String, String)>,
    ) -> Self {
        Self {
            modal_type: ModalType::TransactionDetails {
                tx_hash,
                status,
                details,
            },
            is_visible: true,
            selected_option: 0,
        }
    }

    /// Create a new help modal
    pub fn help(title: String, sections: Vec<HelpSection>) -> Self {
        Self {
            modal_type: ModalType::Help { title, sections },
            is_visible: true,
            selected_option: 0,
        }
    }

    /// Hide the modal
    pub fn hide(&mut self) {
        self.is_visible = false;
    }

    /// Show the modal
    pub fn show(&mut self) {
        self.is_visible = true;
    }

    /// Move selection up (for confirmation dialogs)
    pub fn select_previous(&mut self) {
        if self.selected_option > 0 {
            self.selected_option -= 1;
        }
    }

    /// Move selection down (for confirmation dialogs)
    pub fn select_next(&mut self) {
        if let ModalType::Confirmation { .. } = self.modal_type {
            if self.selected_option < 1 {
                self.selected_option += 1;
            }
        }
    }

    /// Check if the modal is currently visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Get the currently selected option (true for confirm, false for cancel)
    pub fn is_confirmed(&self) -> bool {
        self.selected_option == 0
    }
}

/// Render modal overlay
pub fn render_modal(f: &mut Frame, modal_state: &ModalState, area: Rect) {
    if !modal_state.is_visible {
        return;
    }

    // Calculate modal size and position (centered)
    let modal_area = centered_rect(60, 40, area);

    // Clear the area behind the modal
    f.render_widget(Clear, modal_area);

    match &modal_state.modal_type {
        ModalType::Confirmation {
            title,
            message,
            confirm_text,
            cancel_text,
        } => render_confirmation_modal(
            f,
            title,
            message,
            confirm_text,
            cancel_text,
            modal_state.selected_option,
            modal_area,
        ),
        ModalType::Information { title, content } => {
            render_information_modal(f, title, content, modal_area)
        }
        ModalType::Error {
            title,
            error_message,
            details,
        } => render_error_modal(f, title, error_message, details, modal_area),
        ModalType::TransactionDetails {
            tx_hash,
            status,
            details,
        } => render_transaction_modal(f, tx_hash, status, details, modal_area),
        ModalType::Help { title, sections } => render_help_modal(f, title, sections, modal_area),
    }
}

/// Render confirmation modal
fn render_confirmation_modal(
    f: &mut Frame,
    title: &str,
    message: &str,
    confirm_text: &str,
    cancel_text: &str,
    selected_option: usize,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Message area
            Constraint::Length(3), // Button area
        ])
        .split(area);

    // Message
    let message_paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(title),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));
    f.render_widget(message_paragraph, chunks[0]);

    // Buttons
    let button_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Confirm button
    let confirm_style = if selected_option == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let confirm_button = Paragraph::new(confirm_text)
        .style(confirm_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(confirm_button, button_chunks[0]);

    // Cancel button
    let cancel_style = if selected_option == 1 {
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    };

    let cancel_button = Paragraph::new(cancel_text)
        .style(cancel_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(cancel_button, button_chunks[1]);
}

/// Render information modal
fn render_information_modal(f: &mut Frame, title: &str, content: &[String], area: Rect) {
    let items: Vec<ListItem> = content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Render error modal
fn render_error_modal(
    f: &mut Frame,
    title: &str,
    error_message: &str,
    details: &Option<Vec<String>>,
    area: Rect,
) {
    let mut content = vec![error_message.to_string()];

    if let Some(detail_lines) = details {
        content.push("".to_string()); // Empty line
        content.push("Details:".to_string());
        content.extend(detail_lines.clone());
    }

    let items: Vec<ListItem> = content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(title),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Render transaction details modal
fn render_transaction_modal(
    f: &mut Frame,
    tx_hash: &str,
    status: &str,
    details: &[(String, String)],
    area: Rect,
) {
    let mut content = vec![
        format!("Transaction: {}", tx_hash),
        format!("Status: {}", status),
        "".to_string(),
    ];

    for (key, value) in details {
        content.push(format!("{}: {}", key, value));
    }

    let items: Vec<ListItem> = content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title("Transaction Details"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Render help modal
fn render_help_modal(f: &mut Frame, title: &str, sections: &[HelpSection], area: Rect) {
    let mut content = Vec::new();

    for section in sections {
        content.push(format!("{}:", section.title));
        for (key, desc) in &section.items {
            content.push(format!("  {} - {}", key, desc));
        }
        content.push("".to_string()); // Empty line between sections
    }

    let items: Vec<ListItem> = content
        .iter()
        .map(|line| {
            if line.ends_with(':') {
                ListItem::new(line.as_str()).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(line.as_str())
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(title),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Helper function to calculate centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_state_creation() {
        let modal =
            ModalState::confirmation("Test".to_string(), "Are you sure?".to_string(), None, None);
        assert!(modal.is_visible());
        assert!(modal.is_confirmed()); // Default to first option
    }

    #[test]
    fn test_modal_navigation() {
        let mut modal =
            ModalState::confirmation("Test".to_string(), "Are you sure?".to_string(), None, None);

        assert_eq!(modal.selected_option, 0);
        modal.select_next();
        assert_eq!(modal.selected_option, 1);
        modal.select_previous();
        assert_eq!(modal.selected_option, 0);
    }
}
