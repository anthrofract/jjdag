use crate::{
    command_tree::{CommandTree, display_unbound_error_lines},
    log_tree::{DIFF_HUNK_LINE_IDX, JjLog, TreePosition, get_parent_tree_position},
    shell_out::{JjCommand, JjCommandError, get_input_from_editor},
    terminal::Term,
    update::Message,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    text::{Line, Text},
    widgets::ListState,
};

const LOG_LIST_SCROLL_PADDING: usize = 0;

#[derive(Default, Debug, PartialEq, Eq)]
pub enum State {
    #[default]
    Running,
    Quit,
}

#[derive(Debug, Clone)]
pub struct GlobalArgs {
    pub repository: String,
    pub ignore_immutable: bool,
}

#[derive(Debug)]
pub struct Model {
    pub global_args: GlobalArgs,
    pub display_repository: String,
    pub revset: String,
    pub state: State,
    pub command_tree: CommandTree,
    command_keys: Vec<KeyCode>,
    queued_jj_commands: Vec<JjCommand>,
    accumulated_command_output: Vec<Line<'static>>,
    saved_change_id: Option<String>,
    saved_file_path: Option<String>,
    pub saved_log_index: Option<usize>,
    jj_log: JjLog,
    pub log_list: Vec<Text<'static>>,
    pub log_list_state: ListState,
    log_list_tree_positions: Vec<TreePosition>,
    pub log_list_layout: Rect,
    pub log_list_scroll_padding: usize,
    pub info_list: Option<Text<'static>>,
}

#[derive(Debug)]
enum ScrollDirection {
    Up,
    Down,
}

impl Model {
    pub fn new(repository: String, revset: String) -> Result<Self> {
        let mut model = Self {
            state: State::default(),
            command_tree: CommandTree::new(),
            command_keys: Vec::new(),
            queued_jj_commands: Vec::new(),
            accumulated_command_output: Vec::new(),
            saved_log_index: None,
            saved_change_id: None,
            saved_file_path: None,
            jj_log: JjLog::new()?,
            log_list: Vec::new(),
            log_list_state: ListState::default(),
            log_list_tree_positions: Vec::new(),
            log_list_layout: Rect::ZERO,
            log_list_scroll_padding: LOG_LIST_SCROLL_PADDING,
            info_list: None,
            display_repository: format_repository_for_display(&repository),
            global_args: GlobalArgs {
                repository,
                ignore_immutable: false,
            },
            revset,
        };

        model.sync()?;
        Ok(model)
    }

    pub fn quit(&mut self) {
        self.state = State::Quit;
    }

    fn reset_log_list_selection(&mut self) -> Result<()> {
        // Start with @ selected and unfolded
        let list_idx = match self.jj_log.get_current_commit() {
            None => 0,
            Some(commit) => commit.flat_log_idx,
        };
        self.log_select(list_idx);
        self.toggle_current_fold()
    }

    pub fn sync(&mut self) -> Result<()> {
        self.jj_log.load_log_tree(&self.global_args, &self.revset)?;
        self.sync_log_list()?;
        self.reset_log_list_selection()?;
        Ok(())
    }

    fn sync_log_list(&mut self) -> Result<()> {
        (self.log_list, self.log_list_tree_positions) = self.jj_log.flatten_log()?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        // Add periods for visual feedback on repeated refreshes
        let periods = self
            .info_list
            .as_ref()
            .map(|t| t.to_string())
            .filter(|s| s.starts_with("Refreshed"))
            .map_or(0, |s| s.matches('.').count() + 3);
        self.clear();
        self.sync()?;
        self.info_list = Some(format!("Refreshed{}", ".".repeat(periods)).into());
        Ok(())
    }

    pub fn toggle_ignore_immutable(&mut self) {
        self.global_args.ignore_immutable = !self.global_args.ignore_immutable;
    }

    fn log_offset(&self) -> usize {
        self.log_list_state.offset()
    }

    fn log_selected(&self) -> usize {
        self.log_list_state.selected().unwrap()
    }

    fn log_select(&mut self, idx: usize) {
        self.log_list_state.select(Some(idx));
    }

    fn get_selected_tree_position(&self) -> TreePosition {
        self.log_list_tree_positions[self.log_selected()].clone()
    }

    fn get_selected_change_id(&self) -> Option<&str> {
        let tree_pos = self.get_selected_tree_position();
        self.get_change_id(tree_pos)
    }

    fn get_saved_change_id(&self) -> Option<&str> {
        self.saved_change_id.as_deref()
    }

    fn get_change_id(&self, tree_pos: TreePosition) -> Option<&str> {
        match self.jj_log.get_tree_commit(&tree_pos) {
            None => None,
            Some(commit) => Some(&commit.change_id),
        }
    }

    fn get_selected_file_path(&self) -> Option<&str> {
        let tree_pos = self.get_selected_tree_position();
        self.get_file_path(tree_pos)
    }

    fn get_saved_file_path(&self) -> Option<&str> {
        self.saved_file_path.as_deref()
    }

    fn get_file_path(&self, tree_pos: TreePosition) -> Option<&str> {
        match self.jj_log.get_tree_file_diff(&tree_pos) {
            None => None,
            Some(file_diff) => Some(&file_diff.path),
        }
    }

    fn is_selected_working_copy(&self) -> bool {
        let tree_pos = self.get_selected_tree_position();
        match self.jj_log.get_tree_commit(&tree_pos) {
            None => false,
            Some(commit) => commit.current_working_copy,
        }
    }

    pub fn select_next_node(&mut self) {
        if self.log_list_state.selected().unwrap() < self.log_list.len() - 1 {
            self.log_list_state.select_next();
        }
    }

    pub fn select_prev_node(&mut self) {
        if self.log_list_state.selected().unwrap() > 0 {
            self.log_list_state.select_previous();
        }
    }

    pub fn select_current_working_copy(&mut self) {
        if let Some(commit) = self.jj_log.get_current_commit() {
            self.log_select(commit.flat_log_idx);
        }
    }

    pub fn select_parent_node(&mut self) -> Result<()> {
        let tree_pos = self.get_selected_tree_position();
        if let Some(parent_pos) = get_parent_tree_position(&tree_pos) {
            let parent_node_idx = self.jj_log.get_tree_node(&parent_pos)?.flat_log_idx();
            self.log_select(parent_node_idx);
        }
        Ok(())
    }

    pub fn select_current_next_sibling_node(&mut self) -> Result<()> {
        let tree_pos = self.get_selected_tree_position();
        self.select_next_sibling_node(tree_pos)
    }

    fn select_next_sibling_node(&mut self, tree_pos: TreePosition) -> Result<()> {
        let mut tree_pos = tree_pos;
        if tree_pos.len() == DIFF_HUNK_LINE_IDX + 1 {
            tree_pos = get_parent_tree_position(&tree_pos).unwrap();
        }
        let idx = tree_pos[tree_pos.len() - 1];

        match get_parent_tree_position(&tree_pos) {
            Some(parent_pos) => {
                let parent_node = self.jj_log.get_tree_node(&parent_pos)?;
                let children = parent_node.children();

                if idx == children.len() - 1 {
                    self.select_next_sibling_node(parent_pos)?;
                } else {
                    let sibling_idx = (idx + 1).min(children.len() - 1);
                    self.log_list_state
                        .select(Some(children[sibling_idx].flat_log_idx()));
                }
            }
            None => {
                let sibling_idx = (idx + 1).min(self.jj_log.log_tree.len() - 1);
                self.log_list_state
                    .select(Some(self.jj_log.log_tree[sibling_idx].flat_log_idx()));
            }
        };

        Ok(())
    }

    pub fn select_current_prev_sibling_node(&mut self) -> Result<()> {
        let tree_pos = self.get_selected_tree_position();
        self.select_prev_sibling_node(tree_pos)
    }

    fn select_prev_sibling_node(&mut self, tree_pos: TreePosition) -> Result<()> {
        if tree_pos.len() == DIFF_HUNK_LINE_IDX + 1 {
            let parent_pos = get_parent_tree_position(&tree_pos).unwrap();
            let parent_node_idx = self.jj_log.get_tree_node(&parent_pos)?.flat_log_idx();
            self.log_select(parent_node_idx);
            return Ok(());
        }
        let idx = tree_pos[tree_pos.len() - 1];

        match get_parent_tree_position(&tree_pos) {
            Some(parent_pos) => {
                let parent_node = self.jj_log.get_tree_node(&parent_pos)?;
                let children = parent_node.children();

                if idx == 0 {
                    let parent_node_idx = parent_node.flat_log_idx();
                    self.log_select(parent_node_idx);
                } else {
                    let sibling_idx = idx - 1;
                    self.log_list_state
                        .select(Some(children[sibling_idx].flat_log_idx()));
                }
            }
            None => {
                let sibling_idx = idx.saturating_sub(1);
                self.log_list_state
                    .select(Some(self.jj_log.log_tree[sibling_idx].flat_log_idx()));
            }
        };

        Ok(())
    }

    pub fn toggle_current_fold(&mut self) -> Result<()> {
        let tree_pos = self.get_selected_tree_position();
        let log_list_selected_idx = self.jj_log.toggle_fold(&self.global_args, &tree_pos)?;
        self.sync_log_list()?;
        self.log_select(log_list_selected_idx);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.info_list = None;
        self.saved_log_index = None;
        self.saved_change_id = None;
        self.saved_file_path = None;
        self.command_keys.clear();
        self.queued_jj_commands.clear();
        self.accumulated_command_output.clear();
    }

    /// User cancelled an action (e.g., closed editor without entering input).
    /// The command key sequence is automatically cleared by `handle_command_key`
    /// when the action is triggered, so we don't need to clear it here.
    fn cancelled(&mut self) -> Result<()> {
        self.info_list = Some(Text::from("Cancelled"));
        Ok(())
    }

    /// The selected or saved change is invalid for this operation (e.g., no
    /// change selected, or the saved selection from a two-step command is missing).
    /// The command key sequence is automatically cleared by `handle_command_key`
    /// when the action is triggered, so we don't need to clear it here.
    fn invalid_selection(&mut self) -> Result<()> {
        self.info_list = Some(Text::from("Invalid selection"));
        Ok(())
    }

    fn display_error_lines(&mut self, err: &anyhow::Error) {
        self.info_list = Some(err.to_string().into_text().unwrap());
    }

    pub fn set_revset(&mut self, term: Term) -> Result<()> {
        let old_revset = self.revset.clone();
        let Some(new_revset) =
            get_input_from_editor(term, Some(&self.revset), Some("Enter the new revset"))?
        else {
            return self.cancelled();
        };
        self.revset = new_revset.clone();
        match self.sync() {
            Err(err) => {
                self.display_error_lines(&err);
                self.revset = old_revset;
            }
            Ok(()) => {
                self.info_list = Some(Text::from(format!("Revset set to '{}'", self.revset)));
            }
        }
        Ok(())
    }

    pub fn show_help(&mut self) {
        self.info_list = Some(self.command_tree.get_help());
    }

    pub fn handle_command_key(&mut self, key_code: KeyCode) -> Option<Message> {
        self.command_keys.push(key_code);

        let node = match self.command_tree.get_node(&self.command_keys) {
            None => {
                self.command_keys.pop();
                display_unbound_error_lines(&mut self.info_list, &key_code);
                return None;
            }
            Some(node) => node,
        };
        if let Some(children) = &node.children {
            self.info_list = Some(children.get_help());
        }
        if let Some(message) = node.action {
            if node.children.is_none() {
                self.command_keys.clear();
            }
            return Some(message);
        }
        None
    }

    pub fn scroll_down_once(&mut self) {
        if self.log_selected() <= self.log_offset() + self.log_list_scroll_padding {
            self.select_next_node();
        }
        *self.log_list_state.offset_mut() = self.log_offset() + 1;
    }

    pub fn scroll_up_once(&mut self) {
        if self.log_offset() == 0 {
            return;
        }
        let last_node_visible = self.line_dist_to_dest_node(
            self.log_list_layout.height as usize - 1,
            self.log_offset(),
            &ScrollDirection::Down,
        );
        if self.log_selected() >= last_node_visible - 1 - self.log_list_scroll_padding {
            self.select_prev_node();
        }
        *self.log_list_state.offset_mut() = self.log_offset().saturating_sub(1);
    }

    pub fn scroll_down_page(&mut self) {
        self.scroll_lines(self.log_list_layout.height as usize, &ScrollDirection::Down);
    }

    pub fn scroll_up_page(&mut self) {
        self.scroll_lines(self.log_list_layout.height as usize, &ScrollDirection::Up);
    }

    fn scroll_lines(&mut self, num_lines: usize, direction: &ScrollDirection) {
        let selected_node_dist_from_offset = self.log_selected() - self.log_offset();
        let mut target_offset =
            self.line_dist_to_dest_node(num_lines, self.log_offset(), direction);
        let mut target_node = target_offset + selected_node_dist_from_offset;
        match direction {
            ScrollDirection::Down => {
                if target_offset == self.log_list.len() - 1 {
                    target_node = target_offset;
                    target_offset = self.log_offset();
                }
            }
            ScrollDirection::Up => {
                // If we're already at the top of the page, then move selection to the top as well
                if target_offset == 0 && target_offset == self.log_offset() {
                    target_node = 0;
                }
            }
        }
        self.log_select(target_node);
        *self.log_list_state.offset_mut() = target_offset;
    }

    pub fn handle_mouse_click(&mut self, row: u16, column: u16) {
        let Rect {
            x,
            y,
            width,
            height,
        } = self.log_list_layout;

        // Check if inside log list
        if row < y || row >= y + height || column < x || column >= x + width {
            return;
        }

        let target_node = self.line_dist_to_dest_node(
            row as usize - y as usize,
            self.log_offset(),
            &ScrollDirection::Down,
        );
        self.log_select(target_node);
    }

    // Since some nodes contain multiple lines, we need a way to determine the destination node
    // which is n lines away from the starting node.
    fn line_dist_to_dest_node(
        &self,
        line_dist: usize,
        starting_node: usize,
        direction: &ScrollDirection,
    ) -> usize {
        let mut current_node = starting_node;
        let mut lines_traversed = 0;
        loop {
            let lines_in_node = self.log_list[current_node].lines.len();
            lines_traversed += lines_in_node;

            // Stop if we've found the dest node or have no further to traverse
            if match direction {
                ScrollDirection::Down => current_node == self.log_list.len() - 1,
                ScrollDirection::Up => current_node == 0,
            } || lines_traversed > line_dist
            {
                break;
            }

            match direction {
                ScrollDirection::Down => current_node += 1,
                ScrollDirection::Up => current_node -= 1,
            }
        }

        current_node
    }

    pub fn save_selection(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            self.clear();
            return self.invalid_selection();
        };
        self.saved_change_id = Some(change_id.to_string());
        self.saved_file_path = self.get_selected_file_path().map(String::from);
        self.saved_log_index = Some(self.log_selected());
        Ok(())
    }

    pub fn jj_describe(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::describe(change_id, self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_duplicate(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::duplicate(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_duplicate_onto(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd =
            JjCommand::duplicate_onto(source_change_id, dest_change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_duplicate_insert_after(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::duplicate_insert_after(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_duplicate_insert_before(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::duplicate_insert_before(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_new(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::new(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_parallelize(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let revset = format!("{}-::{}", change_id, change_id);
        let cmd = JjCommand::parallelize(&revset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_parallelize_range(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let revset = format!("{}::{}", from_change_id, to_change_id);
        let cmd = JjCommand::parallelize(&revset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_parallelize_revset(&mut self, term: Term) -> Result<()> {
        let Some(revset) =
            get_input_from_editor(term, None, Some("Enter the revset to parallelize"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::parallelize(&revset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_new_before(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::new_before(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_new_insert_after(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::new_insert_after(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_new_after_trunk(&mut self) -> Result<()> {
        let cmd = JjCommand::new_after_trunk(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_new_after_trunk_sync(&mut self) -> Result<()> {
        let fetch_cmd = JjCommand::fetch(self.global_args.clone());
        let new_cmd = JjCommand::new_after_trunk(self.global_args.clone());
        self.queue_jj_commands(vec![fetch_cmd, new_cmd])
    }

    pub fn jj_next(&mut self) -> Result<()> {
        let cmd = JjCommand::next(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::next_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_edit(&mut self) -> Result<()> {
        let cmd = JjCommand::next_edit(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_edit_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::next_edit_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_no_edit(&mut self) -> Result<()> {
        let cmd = JjCommand::next_no_edit(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_no_edit_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::next_no_edit_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_next_conflict(&mut self) -> Result<()> {
        let cmd = JjCommand::next_conflict(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev(&mut self) -> Result<()> {
        let cmd = JjCommand::prev(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::prev_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_edit(&mut self) -> Result<()> {
        let cmd = JjCommand::prev_edit(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_edit_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::prev_edit_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_no_edit(&mut self) -> Result<()> {
        let cmd = JjCommand::prev_no_edit(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_no_edit_offset(&mut self, term: Term) -> Result<()> {
        let Some(offset) = get_input_from_editor(term, None, Some("Enter the offset"))? else {
            return self.cancelled();
        };
        let cmd = JjCommand::prev_no_edit_offset(&offset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_prev_conflict(&mut self) -> Result<()> {
        let cmd = JjCommand::prev_conflict(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_abandon(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::abandon(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_abandon_retain_bookmarks(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::abandon_retain_bookmarks(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_abandon_restore_descendants(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::abandon_restore_descendants(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_absorb(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();
        let cmd = JjCommand::absorb(change_id, maybe_file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_absorb_into(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_saved_file_path();
        let Some(into_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::absorb_into(
            from_change_id,
            into_change_id,
            maybe_file_path,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_undo(&mut self) -> Result<()> {
        let cmd = JjCommand::undo(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_redo(&mut self) -> Result<()> {
        let cmd = JjCommand::redo(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_commit(&mut self, term: Term) -> Result<()> {
        let maybe_file_path = self.get_selected_file_path();
        let cmd = JjCommand::commit(maybe_file_path, self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_onto_trunk(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_onto_trunk(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_branch_onto_trunk(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_branch_onto_trunk(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_onto_destination(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_onto_destination(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_branch_onto_destination(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_branch_onto_destination(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_onto_destination_no_descendants(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_onto_destination_no_descendants(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_after_destination(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_after_destination(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_after_destination_no_descendants(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_after_destination_no_descendants(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_before_destination(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_before_destination(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_rebase_before_destination_no_descendants(&mut self) -> Result<()> {
        let Some(source_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(dest_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::rebase_before_destination_no_descendants(
            source_change_id,
            dest_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_restore(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();

        let cmd = JjCommand::restore(change_id, maybe_file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_restore_from(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();

        let cmd =
            JjCommand::restore_from(from_change_id, maybe_file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_restore_into(&mut self) -> Result<()> {
        let Some(into_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();

        let cmd =
            JjCommand::restore_into(into_change_id, maybe_file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_restore_restore_descendants(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();

        let cmd = JjCommand::restore_restore_descendants(
            change_id,
            maybe_file_path,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_restore_from_into(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_saved_file_path();
        let Some(into_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };

        let cmd = JjCommand::restore_from_into(
            from_change_id,
            into_change_id,
            maybe_file_path,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_revert(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::revert_onto(change_id, "@", self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_revert_onto_destination(&mut self) -> Result<()> {
        let Some(revision) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(destination) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::revert_onto(revision, destination, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_revert_insert_after(&mut self) -> Result<()> {
        let Some(revision) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(destination) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::revert_insert_after(revision, destination, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_revert_insert_before(&mut self) -> Result<()> {
        let Some(revision) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(destination) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::revert_insert_before(revision, destination, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_squash(&mut self, term: Term) -> Result<()> {
        let tree_pos = self.get_selected_tree_position();
        let Some(commit) = self.jj_log.get_tree_commit(&tree_pos) else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();

        let cmd = if commit.description_first_line.is_none() {
            JjCommand::squash_noninteractive(
                &commit.change_id,
                maybe_file_path,
                self.global_args.clone(),
            )
        } else {
            JjCommand::squash_interactive(
                &commit.change_id,
                maybe_file_path,
                self.global_args.clone(),
                term,
            )
        };
        self.queue_jj_command(cmd)
    }

    pub fn jj_squash_into(&mut self, term: Term) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_saved_file_path();
        let Some(into_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::squash_into_interactive(
            from_change_id,
            into_change_id,
            maybe_file_path,
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_status(&mut self, term: Term) -> Result<()> {
        let cmd = JjCommand::status(self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_sign(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::sign(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_sign_range(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let revset = format!("{}::{}", from_change_id, to_change_id);
        let cmd = JjCommand::sign(&revset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_view(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = match self.get_selected_file_path() {
            Some(file_path) => JjCommand::diff_file_interactive(
                change_id,
                file_path,
                self.global_args.clone(),
                term,
            ),
            None => JjCommand::show(change_id, self.global_args.clone(), term),
        };
        self.queue_jj_command(cmd)
    }

    pub fn jj_view_from_selection(&mut self, term: Term) -> Result<()> {
        let Some(from_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::diff_from_to_interactive(
            from_change_id,
            "@",
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_view_to_selection(&mut self, term: Term) -> Result<()> {
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd =
            JjCommand::diff_from_to_interactive("@", to_change_id, self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_view_from_selection_to_destination(&mut self, term: Term) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::diff_from_to_interactive(
            from_change_id,
            to_change_id,
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_unsign(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::unsign(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_unsign_range(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let revset = format!("{}::{}", from_change_id, to_change_id);
        let cmd = JjCommand::unsign(&revset, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_simplify_parents(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::simplify_parents(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_simplify_parents_source(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::simplify_parents_source(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_edit(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::edit(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_evolog(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::evolog(change_id, self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_evolog_patch(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::evolog_patch(change_id, self.global_args.clone(), term);
        self.queue_jj_command(cmd)
    }

    pub fn jj_interdiff_from_selection(&mut self, term: Term) -> Result<()> {
        let Some(from_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();
        let cmd = JjCommand::interdiff(
            from_change_id,
            "@",
            maybe_file_path,
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_interdiff_to_selection(&mut self, term: Term) -> Result<()> {
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_selected_file_path();
        let cmd = JjCommand::interdiff(
            "@",
            to_change_id,
            maybe_file_path,
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_interdiff_from_selection_to_destination(&mut self, term: Term) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let maybe_file_path = self.get_saved_file_path();
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::interdiff(
            from_change_id,
            to_change_id,
            maybe_file_path,
            self.global_args.clone(),
            term,
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_file_track(&mut self, term: Term) -> Result<()> {
        let Some(file_path) =
            get_input_from_editor(term, None, Some("Enter the file path(s) to track"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::file_track(&file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_file_untrack(&mut self) -> Result<()> {
        let Some(file_path) = self.get_selected_file_path() else {
            return self.invalid_selection();
        };
        if !self.is_selected_working_copy() {
            return self.invalid_selection();
        }
        let cmd = JjCommand::file_untrack(file_path, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_update_change_id(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::metaedit_update_change_id(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_update_author_timestamp(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::metaedit_update_author_timestamp(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_update_author(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::metaedit_update_author(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_set_author(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let Some(author) = get_input_from_editor(
            term,
            None,
            Some("Enter the author (e.g. 'Name <email@example.com>')"),
        )?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::metaedit_set_author(change_id, &author, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_set_author_timestamp(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let Some(timestamp) = get_input_from_editor(
            term,
            None,
            Some("Enter the author timestamp (e.g. '2000-01-23T01:23:45-08:00')"),
        )?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::metaedit_set_author_timestamp(
            change_id,
            &timestamp,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_metaedit_force_rewrite(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::metaedit_force_rewrite(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_fetch(&mut self) -> Result<()> {
        let cmd = JjCommand::fetch(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_fetch_all_remotes(&mut self) -> Result<()> {
        let cmd = JjCommand::fetch_all_remotes(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_fetch_tracked(&mut self) -> Result<()> {
        let cmd = JjCommand::fetch_tracked(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_fetch_branch(&mut self, term: Term) -> Result<()> {
        let Some(branch) = get_input_from_editor(term, None, Some("Enter the branch to fetch"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::fetch_branch(&branch, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_fetch_remote(&mut self, term: Term) -> Result<()> {
        let Some(remote) =
            get_input_from_editor(term, None, Some("Enter the remote to fetch from"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::fetch_remote(&remote, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push(&mut self) -> Result<()> {
        let cmd = JjCommand::push(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_all(&mut self) -> Result<()> {
        let cmd = JjCommand::push_all(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_revision(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::push_revision(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_tracked(&mut self) -> Result<()> {
        let cmd = JjCommand::push_tracked(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_deleted(&mut self) -> Result<()> {
        let cmd = JjCommand::push_deleted(self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_change(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::push_change(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_named(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let Some(bookmark_name) = get_input_from_editor(
            term,
            None,
            Some("Enter the bookmark name for this revision"),
        )?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::push_named(&bookmark_name, change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_push_bookmark(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_name) =
            get_input_from_editor(term, None, Some("Enter the bookmark to push"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::push_bookmark(&bookmark_name, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_create(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let Some(bookmark_names) =
            get_input_from_editor(term, None, Some("Enter the new bookmark(s)"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_create(&bookmark_names, change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_delete(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_names) =
            get_input_from_editor(term, None, Some("Enter the bookmark(s) to delete"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_delete(&bookmark_names, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_forget(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_names) =
            get_input_from_editor(term, None, Some("Enter the bookmark(s) to forget"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_forget(&bookmark_names, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_forget_include_remotes(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_names) = get_input_from_editor(
            term,
            None,
            Some("Enter the bookmark(s) to forget, including remotes"),
        )?
        else {
            return self.cancelled();
        };
        let cmd =
            JjCommand::bookmark_forget_include_remotes(&bookmark_names, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_move(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::bookmark_move(from_change_id, to_change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_move_allow_backwards(&mut self) -> Result<()> {
        let Some(from_change_id) = self.get_saved_change_id() else {
            return self.invalid_selection();
        };
        let Some(to_change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::bookmark_move_allow_backwards(
            from_change_id,
            to_change_id,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_move_tug(&mut self) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let cmd = JjCommand::bookmark_move_tug(change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_rename(&mut self, term: Term) -> Result<()> {
        let Some(old_bookmark_name) =
            get_input_from_editor(term.clone(), None, Some("Enter the bookmark to rename"))?
        else {
            return self.cancelled();
        };
        let Some(new_bookmark_name) =
            get_input_from_editor(term, None, Some("Enter the bookmark to rename to"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_rename(
            &old_bookmark_name,
            &new_bookmark_name,
            self.global_args.clone(),
        );
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_set(&mut self, term: Term) -> Result<()> {
        let Some(change_id) = self.get_selected_change_id() else {
            return self.invalid_selection();
        };
        let Some(bookmark_names) =
            get_input_from_editor(term, None, Some("Enter the bookmark(s) to set"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_set(&bookmark_names, change_id, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_track(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_at_remote) =
            get_input_from_editor(term, None, Some("Enter the bookmark@remote to track"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_track(&bookmark_at_remote, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    pub fn jj_bookmark_untrack(&mut self, term: Term) -> Result<()> {
        let Some(bookmark_at_remote) =
            get_input_from_editor(term, None, Some("Enter the bookmark@remote to untrack"))?
        else {
            return self.cancelled();
        };
        let cmd = JjCommand::bookmark_untrack(&bookmark_at_remote, self.global_args.clone());
        self.queue_jj_command(cmd)
    }

    fn queue_jj_command(&mut self, cmd: JjCommand) -> Result<()> {
        self.queue_jj_commands(vec![cmd])
    }

    fn queue_jj_commands(&mut self, cmds: Vec<JjCommand>) -> Result<()> {
        self.accumulated_command_output.clear();
        self.queued_jj_commands = cmds;
        self.update_info_list_for_queue();
        Ok(())
    }

    fn update_info_list_for_queue(&mut self) {
        let mut lines = self.accumulated_command_output.clone();
        if let Some(cmd) = self.queued_jj_commands.first() {
            lines.extend(cmd.to_lines());
            lines.push(Line::raw("Running..."));
        }
        self.info_list = Some(Text::from(lines));
    }

    pub fn process_jj_command_queue(&mut self) -> Result<()> {
        if self.queued_jj_commands.is_empty() {
            return Ok(());
        }

        let cmd = self.queued_jj_commands.remove(0);
        let result = cmd.run();

        // Accumulate output from this command (with blank line separator)
        if !self.accumulated_command_output.is_empty() {
            self.accumulated_command_output.push(Line::raw(""));
        }
        self.accumulated_command_output.extend(cmd.to_lines());

        match result {
            Ok(output) => {
                self.accumulated_command_output
                    .extend(output.into_text()?.lines);

                if self.queued_jj_commands.is_empty() {
                    // All commands done, show final output and sync
                    let final_output = self.accumulated_command_output.clone();
                    self.clear();
                    self.info_list = Some(Text::from(final_output));
                    if cmd.sync() {
                        self.sync()?;
                    }
                } else {
                    // More commands to run, update info_list to show next command
                    self.update_info_list_for_queue();
                }
            }
            Err(err) => match err {
                JjCommandError::Other { err } => return Err(err),
                JjCommandError::Failed { stderr } => {
                    // Command failed, show error with accumulated output
                    self.accumulated_command_output
                        .extend(stderr.into_text()?.lines);
                    let final_output = self.accumulated_command_output.clone();
                    self.clear();
                    self.info_list = Some(Text::from(final_output));
                }
            },
        }

        Ok(())
    }
}

fn format_repository_for_display(repository: &str) -> String {
    let Ok(home_dir) = std::env::var("HOME") else {
        return repository.to_string();
    };

    if repository == home_dir {
        return "~".to_string();
    }

    let home_prefix = format!("{home_dir}/");
    match repository.strip_prefix(&home_prefix) {
        Some(relative_path) => format!("~/{relative_path}"),
        None => repository.to_string(),
    }
}
