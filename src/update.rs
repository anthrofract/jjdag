use crate::{model::Model, terminal::Term};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use std::time::Duration;

const EVENT_POLL_DURATION: Duration = Duration::from_millis(200);

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Message {
    Abandon,
    AbandonRestoreDescendants,
    AbandonRetainBookmarks,
    Absorb,
    AbsorbInto,
    BookmarkCreate,
    BookmarkDelete,
    BookmarkForget,
    BookmarkForgetIncludeRemotes,
    BookmarkMove,
    BookmarkMoveAllowBackwards,
    BookmarkMoveTug,
    BookmarkRename,
    BookmarkSet,
    BookmarkTrack,
    BookmarkUntrack,
    Clear,
    Commit,
    Describe,
    Duplicate,
    DuplicateInsertAfter,
    DuplicateInsertBefore,
    DuplicateOnto,
    Edit,
    Evolog,
    EvologPatch,
    FileTrack,
    FileUntrack,
    GitFetch,
    GitFetchAllRemotes,
    GitFetchBranch,
    GitFetchRemote,
    GitFetchTracked,
    GitPush,
    GitPushAll,
    GitPushBookmark,
    GitPushChange,
    GitPushDeleted,
    GitPushNamed,
    GitPushRevision,
    GitPushTracked,
    InterdiffFromSelection,
    InterdiffFromSelectionToDestination,
    InterdiffToSelection,
    LeftMouseClick { row: u16, column: u16 },
    MetaeditForceRewrite,
    MetaeditSetAuthor,
    MetaeditSetAuthorTimestamp,
    MetaeditUpdateAuthor,
    MetaeditUpdateAuthorTimestamp,
    MetaeditUpdateChangeId,
    New,
    NewAfterTrunk,
    NewAfterTrunkSync,
    NewBefore,
    NewInsertAfter,
    Next,
    NextConflict,
    NextEdit,
    NextEditOffset,
    NextNoEdit,
    NextNoEditOffset,
    NextOffset,
    Parallelize,
    ParallelizeRange,
    ParallelizeRevset,
    Prev,
    PrevConflict,
    PrevEdit,
    PrevEditOffset,
    PrevNoEdit,
    PrevNoEditOffset,
    PrevOffset,
    Quit,
    RebaseAfterDestination,
    RebaseAfterDestinationNoDescendants,
    RebaseBeforeDestination,
    RebaseBeforeDestinationNoDescendants,
    RebaseBranchOntoDestination,
    RebaseBranchOntoTrunk,
    RebaseOntoDestination,
    RebaseOntoDestinationNoDescendants,
    RebaseOntoTrunk,
    Redo,
    Refresh,
    Restore,
    RestoreFrom,
    RestoreFromInto,
    RestoreInto,
    RestoreRestoreDescendants,
    Revert,
    RevertInsertAfter,
    RevertInsertBefore,
    RevertOntoDestination,
    RightMouseClick { row: u16, column: u16 },
    SaveSelection,
    ScrollDown,
    ScrollDownPage,
    ScrollUp,
    ScrollUpPage,
    SelectCurrentWorkingCopy,
    SelectNextNode,
    SelectNextSiblingNode,
    SelectParentNode,
    SelectPrevNode,
    SelectPrevSiblingNode,
    SetRevset,
    ShowHelp,
    Sign,
    SignRange,
    SimplifyParents,
    SimplifyParentsSource,
    Squash,
    SquashInto,
    Status,
    ToggleIgnoreImmutable,
    ToggleLogListFold,
    Undo,
    Unsign,
    UnsignRange,
    View,
    ViewFromSelection,
    ViewFromSelectionToDestination,
    ViewToSelection,
}

pub fn update(terminal: Term, model: &mut Model) -> Result<()> {
    model.process_jj_command_queue()?;

    let mut current_msg = handle_event(model)?;
    while let Some(msg) = current_msg {
        current_msg = handle_msg(terminal.clone(), model, msg)?;
    }

    Ok(())
}

fn handle_event(model: &mut Model) -> Result<Option<Message>> {
    if event::poll(EVENT_POLL_DURATION)? {
        match event::read()? {
            Event::Key(key) => {
                if key.kind == event::KeyEventKind::Press {
                    return Ok(handle_key(model, key));
                }
            }
            Event::Mouse(mouse) => {
                return Ok(handle_mouse(mouse));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn handle_key(model: &mut Model, key: event::KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::SelectNextNode),
        KeyCode::Up | KeyCode::Char('k') => Some(Message::SelectPrevNode),
        KeyCode::PageDown => Some(Message::ScrollDownPage),
        KeyCode::PageUp => Some(Message::ScrollUpPage),
        KeyCode::Left | KeyCode::Char('h') => Some(Message::SelectPrevSiblingNode),
        KeyCode::Right | KeyCode::Char('l') => Some(Message::SelectNextSiblingNode),
        KeyCode::Char('K') => Some(Message::SelectParentNode),
        KeyCode::Char(' ') => Some(Message::Refresh),
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::Refresh)
        }
        KeyCode::Tab => Some(Message::ToggleLogListFold),
        KeyCode::Esc => Some(Message::Clear),
        KeyCode::Char('@') => Some(Message::SelectCurrentWorkingCopy),
        KeyCode::Char('L') => Some(Message::SetRevset),
        KeyCode::Char('I') => Some(Message::ToggleIgnoreImmutable),
        KeyCode::Char('?') => Some(Message::ShowHelp),
        _ => model.handle_command_key(key.code),
    }
}

fn handle_mouse(mouse: event::MouseEvent) -> Option<Message> {
    match mouse.kind {
        MouseEventKind::ScrollDown => Some(Message::ScrollDown),
        MouseEventKind::ScrollUp => Some(Message::ScrollUp),
        MouseEventKind::Down(event::MouseButton::Left) => Some(Message::LeftMouseClick {
            row: mouse.row,
            column: mouse.column,
        }),
        MouseEventKind::Down(event::MouseButton::Right) => Some(Message::RightMouseClick {
            row: mouse.row,
            column: mouse.column,
        }),
        _ => None,
    }
}

fn handle_msg(term: Term, model: &mut Model, msg: Message) -> Result<Option<Message>> {
    match msg {
        // General
        Message::Refresh => model.refresh()?,
        Message::Clear => model.clear(),
        Message::ToggleIgnoreImmutable => model.toggle_ignore_immutable(),
        Message::SetRevset => model.set_revset(term)?,
        Message::ShowHelp => model.show_help(),
        Message::Quit => model.quit(),

        // Navigation
        Message::ScrollDownPage => model.scroll_down_page(),
        Message::ScrollUpPage => model.scroll_up_page(),
        Message::SelectNextNode => model.select_next_node(),
        Message::SelectPrevNode => model.select_prev_node(),
        Message::SelectNextSiblingNode => model.select_current_next_sibling_node()?,
        Message::SelectPrevSiblingNode => model.select_current_prev_sibling_node()?,
        Message::SelectParentNode => model.select_parent_node()?,
        Message::SelectCurrentWorkingCopy => model.select_current_working_copy(),
        Message::ToggleLogListFold => model.toggle_current_fold()?,

        // Mouse
        Message::ScrollDown => model.scroll_down_once(),
        Message::ScrollUp => model.scroll_up_once(),
        Message::LeftMouseClick { row, column } => model.handle_mouse_click(row, column),
        Message::RightMouseClick { row, column } => {
            model.handle_mouse_click(row, column);
            model.toggle_current_fold()?;
        }

        // Commands
        Message::Abandon => model.jj_abandon()?,
        Message::AbandonRestoreDescendants => model.jj_abandon_restore_descendants()?,
        Message::AbandonRetainBookmarks => model.jj_abandon_retain_bookmarks()?,
        Message::Absorb => model.jj_absorb()?,
        Message::AbsorbInto => model.jj_absorb_into()?,
        Message::BookmarkCreate => model.jj_bookmark_create(term)?,
        Message::BookmarkDelete => model.jj_bookmark_delete(term)?,
        Message::BookmarkForget => model.jj_bookmark_forget(term)?,
        Message::BookmarkForgetIncludeRemotes => model.jj_bookmark_forget_include_remotes(term)?,
        Message::BookmarkMove => model.jj_bookmark_move()?,
        Message::BookmarkMoveAllowBackwards => model.jj_bookmark_move_allow_backwards()?,
        Message::BookmarkMoveTug => model.jj_bookmark_move_tug()?,
        Message::BookmarkRename => model.jj_bookmark_rename(term)?,
        Message::BookmarkSet => model.jj_bookmark_set(term)?,
        Message::BookmarkTrack => model.jj_bookmark_track(term)?,
        Message::BookmarkUntrack => model.jj_bookmark_untrack(term)?,
        Message::Commit => model.jj_commit(term)?,
        Message::Describe => model.jj_describe(term)?,
        Message::Duplicate => model.jj_duplicate()?,
        Message::DuplicateInsertAfter => model.jj_duplicate_insert_after()?,
        Message::DuplicateInsertBefore => model.jj_duplicate_insert_before()?,
        Message::DuplicateOnto => model.jj_duplicate_onto()?,
        Message::Edit => model.jj_edit()?,
        Message::Evolog => model.jj_evolog(term)?,
        Message::EvologPatch => model.jj_evolog_patch(term)?,
        Message::FileTrack => model.jj_file_track(term)?,
        Message::FileUntrack => model.jj_file_untrack()?,
        Message::GitFetch => model.jj_fetch()?,
        Message::GitFetchAllRemotes => model.jj_fetch_all_remotes()?,
        Message::GitFetchBranch => model.jj_fetch_branch(term)?,
        Message::GitFetchRemote => model.jj_fetch_remote(term)?,
        Message::GitFetchTracked => model.jj_fetch_tracked()?,
        Message::GitPush => model.jj_push()?,
        Message::GitPushAll => model.jj_push_all()?,
        Message::GitPushBookmark => model.jj_push_bookmark(term)?,
        Message::GitPushChange => model.jj_push_change()?,
        Message::GitPushDeleted => model.jj_push_deleted()?,
        Message::GitPushNamed => model.jj_push_named(term)?,
        Message::GitPushRevision => model.jj_push_revision()?,
        Message::GitPushTracked => model.jj_push_tracked()?,
        Message::InterdiffFromSelection => model.jj_interdiff_from_selection(term)?,
        Message::InterdiffFromSelectionToDestination => {
            model.jj_interdiff_from_selection_to_destination(term)?
        }
        Message::InterdiffToSelection => model.jj_interdiff_to_selection(term)?,
        Message::MetaeditForceRewrite => model.jj_metaedit_force_rewrite()?,
        Message::MetaeditSetAuthor => model.jj_metaedit_set_author(term)?,
        Message::MetaeditSetAuthorTimestamp => model.jj_metaedit_set_author_timestamp(term)?,
        Message::MetaeditUpdateAuthor => model.jj_metaedit_update_author()?,
        Message::MetaeditUpdateAuthorTimestamp => model.jj_metaedit_update_author_timestamp()?,
        Message::MetaeditUpdateChangeId => model.jj_metaedit_update_change_id()?,
        Message::Next => model.jj_next()?,
        Message::NextConflict => model.jj_next_conflict()?,
        Message::NextEdit => model.jj_next_edit()?,
        Message::NextEditOffset => model.jj_next_edit_offset(term)?,
        Message::NextNoEdit => model.jj_next_no_edit()?,
        Message::NextNoEditOffset => model.jj_next_no_edit_offset(term)?,
        Message::NextOffset => model.jj_next_offset(term)?,
        Message::New => model.jj_new()?,
        Message::NewAfterTrunk => model.jj_new_after_trunk()?,
        Message::NewAfterTrunkSync => model.jj_new_after_trunk_sync()?,
        Message::NewBefore => model.jj_new_before()?,
        Message::NewInsertAfter => model.jj_new_insert_after()?,
        Message::Parallelize => model.jj_parallelize()?,
        Message::ParallelizeRange => model.jj_parallelize_range()?,
        Message::ParallelizeRevset => model.jj_parallelize_revset(term)?,
        Message::Prev => model.jj_prev()?,
        Message::PrevConflict => model.jj_prev_conflict()?,
        Message::PrevEdit => model.jj_prev_edit()?,
        Message::PrevEditOffset => model.jj_prev_edit_offset(term)?,
        Message::PrevNoEdit => model.jj_prev_no_edit()?,
        Message::PrevNoEditOffset => model.jj_prev_no_edit_offset(term)?,
        Message::PrevOffset => model.jj_prev_offset(term)?,
        Message::RebaseAfterDestination => model.jj_rebase_after_destination()?,
        Message::RebaseAfterDestinationNoDescendants => {
            model.jj_rebase_after_destination_no_descendants()?
        }
        Message::RebaseBeforeDestination => model.jj_rebase_before_destination()?,
        Message::RebaseBeforeDestinationNoDescendants => {
            model.jj_rebase_before_destination_no_descendants()?
        }
        Message::RebaseBranchOntoDestination => model.jj_rebase_branch_onto_destination()?,
        Message::RebaseBranchOntoTrunk => model.jj_rebase_branch_onto_trunk()?,
        Message::RebaseOntoDestination => model.jj_rebase_onto_destination()?,
        Message::RebaseOntoDestinationNoDescendants => {
            model.jj_rebase_onto_destination_no_descendants()?
        }
        Message::RebaseOntoTrunk => model.jj_rebase_onto_trunk()?,
        Message::Redo => model.jj_redo()?,
        Message::Restore => model.jj_restore()?,
        Message::Revert => model.jj_revert()?,
        Message::RevertInsertAfter => model.jj_revert_insert_after()?,
        Message::RevertInsertBefore => model.jj_revert_insert_before()?,
        Message::RevertOntoDestination => model.jj_revert_onto_destination()?,
        Message::RestoreFrom => model.jj_restore_from()?,
        Message::RestoreFromInto => model.jj_restore_from_into()?,
        Message::RestoreInto => model.jj_restore_into()?,
        Message::RestoreRestoreDescendants => model.jj_restore_restore_descendants()?,
        Message::SaveSelection => model.save_selection()?,
        Message::View => model.jj_view(term)?,
        Message::Sign => model.jj_sign()?,
        Message::SignRange => model.jj_sign_range()?,
        Message::SimplifyParents => model.jj_simplify_parents()?,
        Message::SimplifyParentsSource => model.jj_simplify_parents_source()?,
        Message::Squash => model.jj_squash(term)?,
        Message::SquashInto => model.jj_squash_into(term)?,
        Message::Status => model.jj_status(term)?,
        Message::Undo => model.jj_undo()?,
        Message::Unsign => model.jj_unsign()?,
        Message::UnsignRange => model.jj_unsign_range()?,
        Message::ViewFromSelection => model.jj_view_from_selection(term)?,
        Message::ViewFromSelectionToDestination => {
            model.jj_view_from_selection_to_destination(term)?
        }
        Message::ViewToSelection => model.jj_view_to_selection(term)?,
    };

    Ok(None)
}
