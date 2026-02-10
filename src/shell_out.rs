use crate::ansi::strip_non_style_ansi;
use crate::model::GlobalArgs;
use crate::terminal::{self, Term};
use anyhow::{Result, anyhow};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::{
    env,
    io::{Read, Write},
    process::Command,
};

#[derive(Debug)]
pub struct JjCommand {
    args: Vec<String>,
    global_args: GlobalArgs,
    interactive_term: Option<Term>,
    return_output: ReturnOutput,
    sync: bool,
}

impl JjCommand {
    fn _new(
        args: &[&str],
        global_args: GlobalArgs,
        interactive_term: Option<Term>,
        return_output: ReturnOutput,
    ) -> Self {
        Self {
            args: args.iter().map(|a| a.to_string()).collect(),
            global_args,
            interactive_term,
            return_output,
            sync: true,
        }
    }

    fn _new_skip_sync(
        args: &[&str],
        global_args: GlobalArgs,
        interactive_term: Option<Term>,
        return_output: ReturnOutput,
    ) -> Self {
        Self {
            args: args.iter().map(|a| a.to_string()).collect(),
            global_args,
            interactive_term,
            return_output,
            sync: false,
        }
    }

    pub fn sync(&self) -> bool {
        self.sync
    }

    pub fn to_lines(&self) -> Vec<Line<'static>> {
        let line = Line::from(vec![
            Span::styled("❯", Style::default().fg(Color::Yellow)),
            Span::raw(" jj "),
            Span::raw(self.args.join(" ")),
        ]);
        let blank_line = Line::raw("");
        vec![line, blank_line]
    }

    pub fn run(&self) -> Result<String, JjCommandError> {
        let output = match &self.interactive_term {
            None => self.run_noninteractive(),
            Some(term) => self.run_interactive(term),
        }?;
        match self.return_output {
            ReturnOutput::Stdout => Ok(output.stdout),
            ReturnOutput::Stderr => Ok(output.stderr),
        }
    }

    fn run_noninteractive(&self) -> Result<JjCommandOutput, JjCommandError> {
        let mut command = self.base_command();
        command.args(self.args.clone());
        let output = command.output().map_err(JjCommandError::new_other)?;

        let stderr = String::from_utf8_lossy(&output.stderr).into();
        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).map_err(JjCommandError::new_other)?;
            Ok(JjCommandOutput { stdout, stderr })
        } else {
            Err(JjCommandError::new_failed(stderr))
        }
    }

    fn run_interactive(&self, term: &Term) -> Result<JjCommandOutput, JjCommandError> {
        let mut command = self.base_command();
        command.args(self.args.clone());
        command.stderr(std::process::Stdio::piped());

        terminal::relinquish_terminal().map_err(JjCommandError::new_other)?;

        let mut child = command.spawn().map_err(JjCommandError::new_other)?;
        let status = child.wait().map_err(JjCommandError::new_other)?;

        let mut stderr = String::new();
        child
            .stderr
            .take()
            .ok_or_else(|| JjCommandError::new_other(anyhow!("No stderr")))?
            .read_to_string(&mut stderr)
            .map_err(JjCommandError::new_other)?;
        stderr = strip_non_style_ansi(&stderr);

        terminal::takeover_terminal(term).map_err(JjCommandError::new_other)?;

        if status.success() {
            Ok(JjCommandOutput {
                stdout: "".to_string(),
                stderr,
            })
        } else {
            Err(JjCommandError::new_failed(stderr))
        }
    }

    fn base_command(&self) -> Command {
        let mut command = Command::new("jj");
        let args = [
            "--color",
            "always",
            "--config",
            "ui.pager=:builtin",
            "--config",
            "ui.streampager.interface=full-screen-clear-output",
            "--config",
            r#"templates.log_node=
            coalesce(
              if(!self, label("elided", "~")),
              label(
                separate(" ",
                  if(current_working_copy, "working_copy"),
                  if(immutable, "immutable"),
                  if(conflict, "conflict"),
                ),
                coalesce(
                  if(current_working_copy, "@"),
                  if(root, "┴"),
                  if(immutable, "●"),
                  if(conflict, "⊗"),
                  "○",
                )
              )
            )
        "#,
            "--repository",
            &self.global_args.repository,
        ];
        command.args(args);

        if self.global_args.ignore_immutable {
            command.arg("--ignore-immutable");
        }

        command
    }

    pub fn log(revset: &str, global_args: GlobalArgs) -> Self {
        let args = ["log", "--revisions", revset];
        Self::_new(&args, global_args, None, ReturnOutput::Stdout)
    }

    pub fn diff_summary(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["diff", "--revisions", change_id, "--summary"];
        Self::_new(&args, global_args, None, ReturnOutput::Stdout)
    }

    pub fn diff_file(change_id: &str, file: &str, global_args: GlobalArgs) -> Self {
        let args = ["diff", "--revisions", change_id, file];
        Self::_new(&args, global_args, None, ReturnOutput::Stdout)
    }

    pub fn diff_file_interactive(
        change_id: &str,
        file: &str,
        global_args: GlobalArgs,
        term: Term,
    ) -> Self {
        let args = ["diff", "--revisions", change_id, file];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn diff_from_to_interactive(
        from: &str,
        to: &str,
        global_args: GlobalArgs,
        term: Term,
    ) -> Self {
        let args = ["diff", "--from", from, "--to", to];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn describe(change_id: &str, global_args: GlobalArgs, term: Term) -> Self {
        let args = ["describe", change_id];
        Self::_new(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn duplicate(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["duplicate", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn duplicate_onto(change_id: &str, dest_change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["duplicate", change_id, "--onto", dest_change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn duplicate_insert_after(
        change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = ["duplicate", change_id, "--insert-after", dest_change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn duplicate_insert_before(
        change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = ["duplicate", change_id, "--insert-before", dest_change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn new(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["new", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn parallelize(revset: &str, global_args: GlobalArgs) -> Self {
        let args = ["parallelize", revset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev(global_args: GlobalArgs) -> Self {
        let args = ["prev"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["prev", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_edit(global_args: GlobalArgs) -> Self {
        let args = ["prev", "--edit"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_edit_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["prev", "--edit", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_no_edit(global_args: GlobalArgs) -> Self {
        let args = ["prev", "--no-edit"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_no_edit_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["prev", "--no-edit", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn prev_conflict(global_args: GlobalArgs) -> Self {
        let args = ["prev", "--conflict"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn new_insert_after(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["new", "--insert-after", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn new_before(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["new", "--no-edit", "--insert-before", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn new_after_trunk(global_args: GlobalArgs) -> Self {
        let args = ["new", "trunk()"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next(global_args: GlobalArgs) -> Self {
        let args = ["next"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["next", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_edit(global_args: GlobalArgs) -> Self {
        let args = ["next", "--edit"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_edit_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["next", "--edit", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_no_edit(global_args: GlobalArgs) -> Self {
        let args = ["next", "--no-edit"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_no_edit_offset(offset: &str, global_args: GlobalArgs) -> Self {
        let args = ["next", "--no-edit", offset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn next_conflict(global_args: GlobalArgs) -> Self {
        let args = ["next", "--conflict"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn abandon(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["abandon", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn abandon_retain_bookmarks(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["abandon", "--retain-bookmarks", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn abandon_restore_descendants(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["abandon", "--restore-descendants", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn absorb(change_id: &str, maybe_file_path: Option<&str>, global_args: GlobalArgs) -> Self {
        let mut args = vec!["absorb", "--from", change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn absorb_into(
        from_change_id: &str,
        into_change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec!["absorb", "--from", from_change_id, "--into", into_change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn revert_onto(revision: &str, destination: &str, global_args: GlobalArgs) -> Self {
        let args = ["revert", "-r", revision, "--onto", destination];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn revert_insert_after(revision: &str, destination: &str, global_args: GlobalArgs) -> Self {
        let args = ["revert", "-r", revision, "--insert-after", destination];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn revert_insert_before(
        revision: &str,
        destination: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = ["revert", "-r", revision, "--insert-before", destination];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn sign(revset: &str, global_args: GlobalArgs) -> Self {
        let args = ["sign", "-r", revset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn show(change_id: &str, global_args: GlobalArgs, term: Term) -> Self {
        let args = ["show", change_id];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn status(global_args: GlobalArgs, term: Term) -> Self {
        let args = ["status"];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn unsign(revset: &str, global_args: GlobalArgs) -> Self {
        let args = ["unsign", "-r", revset];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn simplify_parents(revision: &str, global_args: GlobalArgs) -> Self {
        let args = ["simplify-parents", "-r", revision];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn simplify_parents_source(revision: &str, global_args: GlobalArgs) -> Self {
        let args = ["simplify-parents", "-s", revision];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn undo(global_args: GlobalArgs) -> Self {
        let args = ["undo"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn redo(global_args: GlobalArgs) -> Self {
        let args = ["redo"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn commit(maybe_file_path: Option<&str>, global_args: GlobalArgs, term: Term) -> Self {
        let mut args = vec!["commit"];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn rebase_onto_trunk(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = vec!["rebase", "--source", change_id, "--onto", "trunk()"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_branch_onto_trunk(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = vec!["rebase", "--branch", change_id, "--onto", "trunk()"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_onto_destination(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--source",
            source_change_id,
            "--onto",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_branch_onto_destination(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--branch",
            source_change_id,
            "--onto",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_onto_destination_no_descendants(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--revisions",
            source_change_id,
            "--onto",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_after_destination(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--source",
            source_change_id,
            "--insert-after",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_after_destination_no_descendants(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--revisions",
            source_change_id,
            "--insert-after",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_before_destination(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--source",
            source_change_id,
            "--insert-before",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn rebase_before_destination_no_descendants(
        source_change_id: &str,
        dest_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = vec![
            "rebase",
            "--revisions",
            source_change_id,
            "--insert-before",
            dest_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn restore(
        change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec!["restore", "--changes-in", change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn restore_from(
        from_change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec!["restore", "--from", from_change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn restore_into(
        into_change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec!["restore", "--into", into_change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn restore_restore_descendants(
        change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec![
            "restore",
            "--changes-in",
            change_id,
            "--restore-descendants",
        ];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn restore_from_into(
        from_change_id: &str,
        into_change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec![
            "restore",
            "--from",
            from_change_id,
            "--into",
            into_change_id,
        ];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn squash_noninteractive(
        change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
    ) -> Self {
        let mut args = vec!["squash", "--revision", change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn squash_interactive(
        change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
        term: Term,
    ) -> Self {
        let mut args = vec!["squash", "--revision", change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn squash_into_interactive(
        from_change_id: &str,
        into_change_id: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
        term: Term,
    ) -> Self {
        let mut args = vec!["squash", "--from", from_change_id, "--into", into_change_id];
        if let Some(file_path) = maybe_file_path {
            args.push(file_path);
        }
        Self::_new(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn edit(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["edit", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn evolog(change_id: &str, global_args: GlobalArgs, term: Term) -> Self {
        let args = ["evolog", "-r", change_id];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn evolog_patch(change_id: &str, global_args: GlobalArgs, term: Term) -> Self {
        let args = ["evolog", "-r", change_id, "--patch"];
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn interdiff(
        from: &str,
        to: &str,
        maybe_file_path: Option<&str>,
        global_args: GlobalArgs,
        term: Term,
    ) -> Self {
        let mut args = vec!["interdiff", "--from", from, "--to", to];
        if let Some(path) = maybe_file_path {
            args.push(path);
        }
        Self::_new_skip_sync(&args, global_args, Some(term), ReturnOutput::Stderr)
    }

    pub fn file_track(file_path: &str, global_args: GlobalArgs) -> Self {
        let args = ["file", "track", file_path];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn file_untrack(file_path: &str, global_args: GlobalArgs) -> Self {
        let args = ["file", "untrack", file_path];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_update_change_id(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["metaedit", "--update-change-id", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_update_author_timestamp(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["metaedit", "--update-author-timestamp", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_update_author(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["metaedit", "--update-author", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_set_author(change_id: &str, author: &str, global_args: GlobalArgs) -> Self {
        let args = ["metaedit", "--author", author, change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_set_author_timestamp(
        change_id: &str,
        timestamp: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = ["metaedit", "--author-timestamp", timestamp, change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn metaedit_force_rewrite(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["metaedit", "--force-rewrite", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn fetch(global_args: GlobalArgs) -> Self {
        let args = ["git", "fetch"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn fetch_all_remotes(global_args: GlobalArgs) -> Self {
        let args = ["git", "fetch", "--all-remotes"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn fetch_tracked(global_args: GlobalArgs) -> Self {
        let args = ["git", "fetch", "--tracked"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn fetch_branch(branch: &str, global_args: GlobalArgs) -> Self {
        let args = ["git", "fetch", "-b", branch];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn fetch_remote(remote: &str, global_args: GlobalArgs) -> Self {
        let args = ["git", "fetch", "--remote", remote];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push(global_args: GlobalArgs) -> Self {
        let args = ["git", "push"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_all(global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "--all"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_revision(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "-r", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_tracked(global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "--tracked"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_deleted(global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "--deleted"];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_change(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "-c", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_named(name: &str, change_id: &str, global_args: GlobalArgs) -> Self {
        let named_arg = format!("{}={}", name, change_id);
        let args = ["git", "push", "--named", &named_arg];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn push_bookmark(bookmark_name: &str, global_args: GlobalArgs) -> Self {
        let args = ["git", "push", "-b", bookmark_name];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_create(bookmark_names: &str, change_id: &str, global_args: GlobalArgs) -> Self {
        let args = [
            "bookmark",
            "create",
            "--revision",
            change_id,
            bookmark_names,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_delete(bookmark_names: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "delete", bookmark_names];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_forget(bookmark_names: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "forget", bookmark_names];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_forget_include_remotes(bookmark_names: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "forget", "--include-remotes", bookmark_names];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_move(
        from_change_id: &str,
        to_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = [
            "bookmark",
            "move",
            "--from",
            from_change_id,
            "--to",
            to_change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_move_allow_backwards(
        from_change_id: &str,
        to_change_id: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = [
            "bookmark",
            "move",
            "--from",
            from_change_id,
            "--to",
            to_change_id,
            "--allow-backwards",
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_move_tug(change_id: &str, global_args: GlobalArgs) -> Self {
        let args = [
            "bookmark",
            "move",
            "--from",
            "heads(::@- & bookmarks())",
            "--to",
            change_id,
        ];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_rename(
        old_bookmark_name: &str,
        new_bookmark_name: &str,
        global_args: GlobalArgs,
    ) -> Self {
        let args = ["bookmark", "rename", old_bookmark_name, new_bookmark_name];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_set(bookmark_names: &str, change_id: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "set", bookmark_names, "--revision", change_id];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_track(bookmark_at_remote: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "track", bookmark_at_remote];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn bookmark_untrack(bookmark_at_remote: &str, global_args: GlobalArgs) -> Self {
        let args = ["bookmark", "untrack", bookmark_at_remote];
        Self::_new(&args, global_args, None, ReturnOutput::Stderr)
    }

    pub fn ensure_valid_repo(repository: &str) -> Result<String, JjCommandError> {
        let args = [
            "--repository",
            repository,
            "workspace",
            "root",
            "--color",
            "always",
        ];
        let output = Command::new("jj")
            .args(args)
            .output()
            .map_err(JjCommandError::new_other)?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .to_string()
                .trim()
                .to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into();
            Err(JjCommandError::new_failed(stderr))
        }
    }
}

#[derive(Debug)]
enum ReturnOutput {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum JjCommandError {
    Failed { stderr: String },
    Other { err: anyhow::Error },
}

impl JjCommandError {
    fn new_failed(stderr: String) -> Self {
        Self::Failed {
            stderr: stderr.trim().to_string(),
        }
    }

    fn new_other(err: impl Into<anyhow::Error>) -> Self {
        Self::Other { err: err.into() }
    }
}

impl std::fmt::Display for JjCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed { stderr } => {
                write!(f, "{stderr}")
            }
            Self::Other { err } => err.fmt(f),
        }
    }
}

impl std::error::Error for JjCommandError {}

#[derive(Debug)]
pub struct JjCommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub fn get_input_from_editor(
    interactive_term: Term,
    starting_text: Option<&str>,
    help_text: Option<&str>,
) -> Result<Option<String>> {
    // Create temp file
    let mut temp_file = tempfile::Builder::new()
        .suffix(".jjdescription")
        .tempfile()?;
    if let Some(text) = starting_text {
        writeln!(temp_file, "{text}")?;
        temp_file.flush()?;
    }
    if let Some(text) = help_text {
        writeln!(temp_file, "\n\nJJ: {text}")?;
        writeln!(
            temp_file,
            "JJ: Lines starting with \"JJ:\" (like this one) will be removed."
        )?;

        temp_file.flush()?;
    }
    let temp_path = temp_file.path().to_path_buf();

    // Open editor in temp file
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    terminal::relinquish_terminal()?;
    let status = Command::new(&editor).arg(&temp_path).status()?;
    terminal::takeover_terminal(&interactive_term)?;
    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // Remove all lines starting with "JJ: "
    let contents = std::fs::read_to_string(&temp_path)?;
    let contents: String = contents
        .lines()
        .filter(|line| !line.starts_with("JJ:"))
        .collect::<Vec<&str>>()
        .join("\n")
        .trim()
        .to_string();
    if contents.is_empty() {
        Ok(None)
    } else {
        Ok(Some(contents))
    }
}
