use crate::update::Message;
use crossterm::event::KeyCode;
use indexmap::IndexMap;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};
use std::collections::HashMap;

type HelpEntries = IndexMap<String, Vec<(String, String)>>;

#[derive(Debug, Clone)]
pub struct CommandTreeNodeChildren {
    nodes: HashMap<KeyCode, CommandTreeNode>,
    help: HelpEntries,
}

impl CommandTreeNodeChildren {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            help: IndexMap::new(),
        }
    }

    pub fn get_node(&self, key_code: &KeyCode) -> Option<&CommandTreeNode> {
        self.nodes.get(key_code)
    }

    pub fn get_node_mut(&mut self, key_code: &KeyCode) -> Option<&mut CommandTreeNode> {
        self.nodes.get_mut(key_code)
    }

    pub fn get_help_entries(&self) -> HelpEntries {
        let mut help = self.help.clone();

        for (_, entries) in help.iter_mut() {
            entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        }

        help
    }

    pub fn get_help(&self) -> Text<'static> {
        let entries = self.get_help_entries();
        render_help_text(entries)
    }

    pub fn add_child(
        &mut self,
        help_group_text: &str,
        help_text: &str,
        key_code: KeyCode,
        node: CommandTreeNode,
    ) {
        self.nodes.insert(key_code, node);
        let help_group = self.help.entry(help_group_text.to_string()).or_default();
        help_group.push((key_code.to_string(), help_text.to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct CommandTreeNode {
    pub children: Option<CommandTreeNodeChildren>,
    pub action: Option<Message>,
}

impl CommandTreeNode {
    pub fn new_children() -> Self {
        Self {
            children: Some(CommandTreeNodeChildren::new()),
            action: None,
        }
    }

    pub fn new_action(action: Message) -> Self {
        Self {
            children: None,
            action: Some(action),
        }
    }

    pub fn new_action_with_children(action: Message) -> Self {
        let mut node = Self::new_children();
        node.action = Some(action);
        node
    }
}

#[derive(Debug)]
pub struct CommandTree(CommandTreeNode);

impl CommandTree {
    fn add_children(&mut self, entries: Vec<(&str, &str, Vec<KeyCode>, CommandTreeNode)>) {
        for (help_group_text, help_text, key_codes, node) in entries {
            let (last_key, rest_keys) = key_codes.split_last().unwrap();
            let dest_node = self.get_node_mut(rest_keys).unwrap();
            let children = dest_node.children.as_mut().unwrap();
            children.add_child(help_group_text, help_text, *last_key, node)
        }
    }

    pub fn get_node(&self, key_codes: &[KeyCode]) -> Option<&CommandTreeNode> {
        let mut node = &self.0;

        for key_code in key_codes {
            let children = match &node.children {
                None => return None,
                Some(children) => children,
            };
            node = children.get_node(key_code)?;
        }

        Some(node)
    }

    fn get_node_mut(&mut self, key_codes: &[KeyCode]) -> Option<&mut CommandTreeNode> {
        let mut node = &mut self.0;

        for key_code in key_codes {
            let children = match &mut node.children {
                None => return None,
                Some(children) => children,
            };
            node = children.get_node_mut(key_code)?;
        }

        Some(node)
    }

    pub fn get_help(&self) -> Text<'static> {
        let nav_help = [
            ("Tab ", "Toggle folding"),
            ("PgDn", "Move down page"),
            ("PgUp", "Move up page"),
            ("j/↓ ", "Move down"),
            ("k/↑ ", "Move up"),
            ("l/→ ", "Next sibling"),
            ("h/← ", "Prev sibling"),
            ("K", "Select parent"),
            ("@", "Select @ change"),
        ]
        .iter()
        .map(|(key, help)| (key.to_string(), help.to_string()))
        .collect();

        let general_help = [
            ("Spc/Ctrl-r", "Refresh log tree"),
            ("Esc", "Clear app state"),
            ("L", "Set log revset"),
            ("I", "Toggle --ignore-immutable"),
            ("?", "Show help"),
            ("q", "Quit"),
        ]
        .iter()
        .map(|(key, help)| (key.to_string(), help.to_string()))
        .collect();

        let mut entries = self.0.children.as_ref().unwrap().get_help_entries();
        entries.insert("Navigation".to_string(), nav_help);
        entries.insert("General".to_string(), general_help);
        render_help_text(entries)
    }

    pub fn new() -> Self {
        let items = vec![
            (
                "Commands",
                "Abandon",
                vec![KeyCode::Char('a')],
                CommandTreeNode::new_children(),
            ),
            (
                "Abandon",
                "Selection",
                vec![KeyCode::Char('a'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::Abandon),
            ),
            (
                "Abandon",
                "Selection (retain bookmarks)",
                vec![KeyCode::Char('a'), KeyCode::Char('b')],
                CommandTreeNode::new_action(Message::AbandonRetainBookmarks),
            ),
            (
                "Abandon",
                "Selection (restore descendants)",
                vec![KeyCode::Char('a'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::AbandonRestoreDescendants),
            ),
            (
                "Commands",
                "Absorb",
                vec![KeyCode::Char('A')],
                CommandTreeNode::new_children(),
            ),
            (
                "Absorb",
                "From selection",
                vec![KeyCode::Char('A'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::Absorb),
            ),
            (
                "Absorb",
                "From selection into destination",
                vec![KeyCode::Char('A'), KeyCode::Char('i')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Absorb into",
                "Select destination",
                vec![KeyCode::Char('A'), KeyCode::Char('i'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::AbsorbInto),
            ),
            (
                "Commands",
                "Bookmark",
                vec![KeyCode::Char('b')],
                CommandTreeNode::new_children(),
            ),
            (
                "Bookmark",
                "Create at selection",
                vec![KeyCode::Char('b'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::BookmarkCreate),
            ),
            (
                "Bookmark",
                "Move",
                vec![KeyCode::Char('b'), KeyCode::Char('m')],
                CommandTreeNode::new_children(),
            ),
            (
                "Bookmark move",
                "Selected bookmark to destination",
                vec![KeyCode::Char('b'), KeyCode::Char('m'), KeyCode::Char('m')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Move bookmark to",
                "Select destination",
                vec![
                    KeyCode::Char('b'),
                    KeyCode::Char('m'),
                    KeyCode::Char('m'),
                    KeyCode::Enter,
                ],
                CommandTreeNode::new_action(Message::BookmarkMove),
            ),
            (
                "Bookmark move",
                "Selected bookmark to destination (allow backwards)",
                vec![KeyCode::Char('b'), KeyCode::Char('m'), KeyCode::Char('M')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Move bookmark to, allowing backwards",
                "Select destination",
                vec![
                    KeyCode::Char('b'),
                    KeyCode::Char('m'),
                    KeyCode::Char('M'),
                    KeyCode::Enter,
                ],
                CommandTreeNode::new_action(Message::BookmarkMoveAllowBackwards),
            ),
            (
                "Bookmark move",
                "Tug to selection",
                vec![KeyCode::Char('b'), KeyCode::Char('m'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::BookmarkMoveTug),
            ),
            (
                "Bookmark",
                "Rename",
                vec![KeyCode::Char('b'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::BookmarkRename),
            ),
            (
                "Bookmark",
                "Track",
                vec![KeyCode::Char('b'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::BookmarkTrack),
            ),
            (
                "Bookmark",
                "Untrack",
                vec![KeyCode::Char('b'), KeyCode::Char('u')],
                CommandTreeNode::new_action(Message::BookmarkUntrack),
            ),
            (
                "Bookmark",
                "Delete",
                vec![KeyCode::Char('b'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::BookmarkDelete),
            ),
            (
                "Bookmark",
                "Forget",
                vec![KeyCode::Char('b'), KeyCode::Char('f')],
                CommandTreeNode::new_action(Message::BookmarkForget),
            ),
            (
                "Bookmark",
                "Forget, including remotes",
                vec![KeyCode::Char('b'), KeyCode::Char('F')],
                CommandTreeNode::new_action(Message::BookmarkForgetIncludeRemotes),
            ),
            (
                "Bookmark",
                "Set to selection",
                vec![KeyCode::Char('b'), KeyCode::Char('s')],
                CommandTreeNode::new_action(Message::BookmarkSet),
            ),
            (
                "Commands",
                "Commit",
                vec![KeyCode::Char('c')],
                CommandTreeNode::new_children(),
            ),
            (
                "Commit",
                "Selection",
                vec![KeyCode::Char('c'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::Commit),
            ),
            (
                "Commands",
                "Describe",
                vec![KeyCode::Char('d')],
                CommandTreeNode::new_children(),
            ),
            (
                "Describe",
                "Selection",
                vec![KeyCode::Char('d'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::Describe),
            ),
            (
                "Commands",
                "Duplicate",
                vec![KeyCode::Char('D')],
                CommandTreeNode::new_children(),
            ),
            (
                "Duplicate",
                "Selection",
                vec![KeyCode::Char('D'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::Duplicate),
            ),
            (
                "Duplicate",
                "Selection onto destination",
                vec![KeyCode::Char('D'), KeyCode::Char('o')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Duplicate onto",
                "Select destination",
                vec![KeyCode::Char('D'), KeyCode::Char('o'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::DuplicateOnto),
            ),
            (
                "Duplicate",
                "Selection insert after destination",
                vec![KeyCode::Char('D'), KeyCode::Char('a')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Duplicate insert after",
                "Select destination",
                vec![KeyCode::Char('D'), KeyCode::Char('a'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::DuplicateInsertAfter),
            ),
            (
                "Duplicate",
                "Selection insert before destination",
                vec![KeyCode::Char('D'), KeyCode::Char('b')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Duplicate insert before",
                "Select destination",
                vec![KeyCode::Char('D'), KeyCode::Char('b'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::DuplicateInsertBefore),
            ),
            (
                "Commands",
                "Edit",
                vec![KeyCode::Char('e')],
                CommandTreeNode::new_children(),
            ),
            (
                "Edit",
                "Selection",
                vec![KeyCode::Char('e'), KeyCode::Char('e')],
                CommandTreeNode::new_action(Message::Edit),
            ),
            (
                "Commands",
                "Evolog",
                vec![KeyCode::Char('E')],
                CommandTreeNode::new_children(),
            ),
            (
                "Evolog",
                "Selection",
                vec![KeyCode::Char('E'), KeyCode::Char('e')],
                CommandTreeNode::new_action(Message::Evolog),
            ),
            (
                "Evolog",
                "Selection (patch)",
                vec![KeyCode::Char('E'), KeyCode::Char('E')],
                CommandTreeNode::new_action(Message::EvologPatch),
            ),
            (
                "Commands",
                "File",
                vec![KeyCode::Char('f')],
                CommandTreeNode::new_children(),
            ),
            (
                "File",
                "Track (enter filepath)",
                vec![KeyCode::Char('f'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::FileTrack),
            ),
            (
                "File",
                "Untrack selection (must be ignored)",
                vec![KeyCode::Char('f'), KeyCode::Char('u')],
                CommandTreeNode::new_action(Message::FileUntrack),
            ),
            (
                "Commands",
                "Git",
                vec![KeyCode::Char('g')],
                CommandTreeNode::new_children(),
            ),
            (
                "Git",
                "Fetch",
                vec![KeyCode::Char('g'), KeyCode::Char('f')],
                CommandTreeNode::new_children(),
            ),
            (
                "Git fetch",
                "Default",
                vec![KeyCode::Char('g'), KeyCode::Char('f'), KeyCode::Char('f')],
                CommandTreeNode::new_action(Message::GitFetch),
            ),
            (
                "Git fetch",
                "All remotes",
                vec![KeyCode::Char('g'), KeyCode::Char('f'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::GitFetchAllRemotes),
            ),
            (
                "Git fetch",
                "Tracked bookmarks",
                vec![KeyCode::Char('g'), KeyCode::Char('f'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::GitFetchTracked),
            ),
            (
                "Git fetch",
                "Branch by name",
                vec![KeyCode::Char('g'), KeyCode::Char('f'), KeyCode::Char('b')],
                CommandTreeNode::new_action(Message::GitFetchBranch),
            ),
            (
                "Git fetch",
                "Remote by name",
                vec![KeyCode::Char('g'), KeyCode::Char('f'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::GitFetchRemote),
            ),
            (
                "Git",
                "Push",
                vec![KeyCode::Char('g'), KeyCode::Char('p')],
                CommandTreeNode::new_children(),
            ),
            (
                "Git push",
                "Default",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('p')],
                CommandTreeNode::new_action(Message::GitPush),
            ),
            (
                "Git push",
                "All bookmarks",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::GitPushAll),
            ),
            (
                "Git push",
                "Bookmarks at selection",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::GitPushRevision),
            ),
            (
                "Git push",
                "Tracked bookmarks",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::GitPushTracked),
            ),
            (
                "Git push",
                "Deleted bookmarks",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::GitPushDeleted),
            ),
            (
                "Git push",
                "New bookmark for selection",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::GitPushChange),
            ),
            (
                "Git push",
                "New named bookmark for selection",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('n')],
                CommandTreeNode::new_action(Message::GitPushNamed),
            ),
            (
                "Git push",
                "Bookmark by name",
                vec![KeyCode::Char('g'), KeyCode::Char('p'), KeyCode::Char('b')],
                CommandTreeNode::new_action(Message::GitPushBookmark),
            ),
            (
                "Commands",
                "Interdiff",
                vec![KeyCode::Char('i')],
                CommandTreeNode::new_children(),
            ),
            (
                "Interdiff",
                "From @ to selection",
                vec![KeyCode::Char('i'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::InterdiffToSelection),
            ),
            (
                "Interdiff",
                "From selection to @",
                vec![KeyCode::Char('i'), KeyCode::Char('f')],
                CommandTreeNode::new_action(Message::InterdiffFromSelection),
            ),
            (
                "Interdiff",
                "From selection to destination",
                vec![KeyCode::Char('i'), KeyCode::Char('i')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Interdiff to destination",
                "Select destination",
                vec![KeyCode::Char('i'), KeyCode::Char('i'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::InterdiffFromSelectionToDestination),
            ),
            (
                "Commands",
                "Metaedit",
                vec![KeyCode::Char('m')],
                CommandTreeNode::new_children(),
            ),
            (
                "Metaedit",
                "Update change-id",
                vec![KeyCode::Char('m'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::MetaeditUpdateChangeId),
            ),
            (
                "Metaedit",
                "Update author timestamp to now",
                vec![KeyCode::Char('m'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::MetaeditUpdateAuthorTimestamp),
            ),
            (
                "Metaedit",
                "Update author to configured user",
                vec![KeyCode::Char('m'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::MetaeditUpdateAuthor),
            ),
            (
                "Metaedit",
                "Set author",
                vec![KeyCode::Char('m'), KeyCode::Char('A')],
                CommandTreeNode::new_action(Message::MetaeditSetAuthor),
            ),
            (
                "Metaedit",
                "Set author timestamp",
                vec![KeyCode::Char('m'), KeyCode::Char('T')],
                CommandTreeNode::new_action(Message::MetaeditSetAuthorTimestamp),
            ),
            (
                "Metaedit",
                "Force rewrite",
                vec![KeyCode::Char('m'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::MetaeditForceRewrite),
            ),
            (
                "Commands",
                "New",
                vec![KeyCode::Char('n')],
                CommandTreeNode::new_children(),
            ),
            (
                "New",
                "After selection",
                vec![KeyCode::Char('n'), KeyCode::Char('n')],
                CommandTreeNode::new_action(Message::New),
            ),
            (
                "New",
                "After selection (rebase children)",
                vec![KeyCode::Char('n'), KeyCode::Char('a')],
                CommandTreeNode::new_action(Message::NewInsertAfter),
            ),
            (
                "New",
                "Before selection (rebase children)",
                vec![KeyCode::Char('n'), KeyCode::Char('b')],
                CommandTreeNode::new_action(Message::NewBefore),
            ),
            (
                "New",
                "After trunk",
                vec![KeyCode::Char('n'), KeyCode::Char('m')],
                CommandTreeNode::new_action(Message::NewAfterTrunk),
            ),
            (
                "New",
                "After trunk (sync)",
                vec![KeyCode::Char('n'), KeyCode::Char('M')],
                CommandTreeNode::new_action(Message::NewAfterTrunkSync),
            ),
            (
                "Commands",
                "Next",
                vec![KeyCode::Char('N')],
                CommandTreeNode::new_children(),
            ),
            (
                "Commands",
                "Parallelize",
                vec![KeyCode::Char('p')],
                CommandTreeNode::new_children(),
            ),
            (
                "Parallelize",
                "Selection with parent",
                vec![KeyCode::Char('p'), KeyCode::Char('p')],
                CommandTreeNode::new_action(Message::Parallelize),
            ),
            (
                "Parallelize",
                "From selection to destination",
                vec![KeyCode::Char('p'), KeyCode::Char('P')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Parallelize range",
                "Select destination",
                vec![KeyCode::Char('p'), KeyCode::Char('P'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::ParallelizeRange),
            ),
            (
                "Parallelize",
                "Revset",
                vec![KeyCode::Char('p'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::ParallelizeRevset),
            ),
            (
                "Next",
                "Next",
                vec![KeyCode::Char('N'), KeyCode::Char('n')],
                CommandTreeNode::new_action(Message::Next),
            ),
            (
                "Next",
                "Nth next",
                vec![KeyCode::Char('N'), KeyCode::Char('N')],
                CommandTreeNode::new_action(Message::NextOffset),
            ),
            (
                "Next",
                "Next (edit)",
                vec![KeyCode::Char('N'), KeyCode::Char('e')],
                CommandTreeNode::new_action(Message::NextEdit),
            ),
            (
                "Next",
                "Nth next (edit)",
                vec![KeyCode::Char('N'), KeyCode::Char('E')],
                CommandTreeNode::new_action(Message::NextEditOffset),
            ),
            (
                "Next",
                "Next (no-edit)",
                vec![KeyCode::Char('N'), KeyCode::Char('x')],
                CommandTreeNode::new_action(Message::NextNoEdit),
            ),
            (
                "Next",
                "Nth next (no-edit)",
                vec![KeyCode::Char('N'), KeyCode::Char('X')],
                CommandTreeNode::new_action(Message::NextNoEditOffset),
            ),
            (
                "Next",
                "Next conflict",
                vec![KeyCode::Char('N'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::NextConflict),
            ),
            (
                "Commands",
                "Previous",
                vec![KeyCode::Char('P')],
                CommandTreeNode::new_children(),
            ),
            (
                "Previous",
                "Previous",
                vec![KeyCode::Char('P'), KeyCode::Char('p')],
                CommandTreeNode::new_action(Message::Prev),
            ),
            (
                "Previous",
                "Nth previous",
                vec![KeyCode::Char('P'), KeyCode::Char('P')],
                CommandTreeNode::new_action(Message::PrevOffset),
            ),
            (
                "Previous",
                "Previous (edit)",
                vec![KeyCode::Char('P'), KeyCode::Char('e')],
                CommandTreeNode::new_action(Message::PrevEdit),
            ),
            (
                "Previous",
                "Nth previous (edit)",
                vec![KeyCode::Char('P'), KeyCode::Char('E')],
                CommandTreeNode::new_action(Message::PrevEditOffset),
            ),
            (
                "Previous",
                "Previous (no-edit)",
                vec![KeyCode::Char('P'), KeyCode::Char('x')],
                CommandTreeNode::new_action(Message::PrevNoEdit),
            ),
            (
                "Previous",
                "Nth previous (no-edit)",
                vec![KeyCode::Char('P'), KeyCode::Char('X')],
                CommandTreeNode::new_action(Message::PrevNoEditOffset),
            ),
            (
                "Previous",
                "Previous conflict",
                vec![KeyCode::Char('P'), KeyCode::Char('c')],
                CommandTreeNode::new_action(Message::PrevConflict),
            ),
            (
                "Commands",
                "Squash",
                vec![KeyCode::Char('s')],
                CommandTreeNode::new_children(),
            ),
            (
                "Squash",
                "Selection into parent",
                vec![KeyCode::Char('s'), KeyCode::Char('s')],
                CommandTreeNode::new_action(Message::Squash),
            ),
            (
                "Squash",
                "Selection into destination",
                vec![KeyCode::Char('s'), KeyCode::Char('i')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Squash into",
                "Select destination",
                vec![KeyCode::Char('s'), KeyCode::Char('i'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::SquashInto),
            ),
            (
                "Commands",
                "Status",
                vec![KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::Status),
            ),
            (
                "Commands",
                "Sign",
                vec![KeyCode::Char('S')],
                CommandTreeNode::new_children(),
            ),
            (
                "Sign",
                "Selection",
                vec![KeyCode::Char('S'), KeyCode::Char('s')],
                CommandTreeNode::new_action(Message::Sign),
            ),
            (
                "Sign",
                "From selection to destination",
                vec![KeyCode::Char('S'), KeyCode::Char('S')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Sign range",
                "Select destination",
                vec![KeyCode::Char('S'), KeyCode::Char('S'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::SignRange),
            ),
            (
                "Sign",
                "Unsign selection",
                vec![KeyCode::Char('S'), KeyCode::Char('u')],
                CommandTreeNode::new_action(Message::Unsign),
            ),
            (
                "Sign",
                "Unsign from selection to destination",
                vec![KeyCode::Char('S'), KeyCode::Char('U')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Unsign range",
                "Select destination",
                vec![KeyCode::Char('S'), KeyCode::Char('U'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::UnsignRange),
            ),
            (
                "Commands",
                "Simplify parents",
                vec![KeyCode::Char('y')],
                CommandTreeNode::new_children(),
            ),
            (
                "Simplify parents of",
                "Selection",
                vec![KeyCode::Char('y'), KeyCode::Char('y')],
                CommandTreeNode::new_action(Message::SimplifyParents),
            ),
            (
                "Simplify parents of",
                "Selection with descendants",
                vec![KeyCode::Char('y'), KeyCode::Char('Y')],
                CommandTreeNode::new_action(Message::SimplifyParentsSource),
            ),
            (
                "Commands",
                "Rebase",
                vec![KeyCode::Char('r')],
                CommandTreeNode::new_children(),
            ),
            (
                "Rebase",
                "Selection onto trunk",
                vec![KeyCode::Char('r'), KeyCode::Char('m')],
                CommandTreeNode::new_action(Message::RebaseOntoTrunk),
            ),
            (
                "Rebase",
                "Selected branch onto trunk",
                vec![KeyCode::Char('r'), KeyCode::Char('M')],
                CommandTreeNode::new_action(Message::RebaseBranchOntoTrunk),
            ),
            (
                "Rebase",
                "Selection onto destination",
                vec![KeyCode::Char('r'), KeyCode::Char('o')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase onto",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('o'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseOntoDestination),
            ),
            (
                "Rebase",
                "Selected branch onto destination",
                vec![KeyCode::Char('r'), KeyCode::Char('O')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase branch onto",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('O'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseBranchOntoDestination),
            ),
            (
                "Rebase",
                "Selection onto destination (no descendants)",
                vec![KeyCode::Char('r'), KeyCode::Char('r')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase revision onto",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('r'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseOntoDestinationNoDescendants),
            ),
            (
                "Rebase",
                "Selection after destination",
                vec![KeyCode::Char('r'), KeyCode::Char('a')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase after",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('a'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseAfterDestination),
            ),
            (
                "Rebase",
                "Selection after destination (no descendants)",
                vec![KeyCode::Char('r'), KeyCode::Char('A')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase after",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('A'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseAfterDestinationNoDescendants),
            ),
            (
                "Rebase",
                "Selection before destination",
                vec![KeyCode::Char('r'), KeyCode::Char('b')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase before",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('b'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseBeforeDestination),
            ),
            (
                "Rebase",
                "Selection before destination (no descendants)",
                vec![KeyCode::Char('r'), KeyCode::Char('B')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Rebase before",
                "Select destination",
                vec![KeyCode::Char('r'), KeyCode::Char('B'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RebaseBeforeDestinationNoDescendants),
            ),
            (
                "Commands",
                "Restore",
                vec![KeyCode::Char('R')],
                CommandTreeNode::new_children(),
            ),
            (
                "Restore",
                "Changes in selection",
                vec![KeyCode::Char('R'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::Restore),
            ),
            (
                "Restore",
                "Changes in selection (restore descendants)",
                vec![KeyCode::Char('R'), KeyCode::Char('d')],
                CommandTreeNode::new_action(Message::RestoreRestoreDescendants),
            ),
            (
                "Restore",
                "From selection into @",
                vec![KeyCode::Char('R'), KeyCode::Char('f')],
                CommandTreeNode::new_action(Message::RestoreFrom),
            ),
            (
                "Restore",
                "From @ into selection",
                vec![KeyCode::Char('R'), KeyCode::Char('i')],
                CommandTreeNode::new_action(Message::RestoreInto),
            ),
            (
                "Restore",
                "From selection into destination",
                vec![KeyCode::Char('R'), KeyCode::Char('R')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Restore into",
                "Select destination",
                vec![KeyCode::Char('R'), KeyCode::Char('R'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RestoreFromInto),
            ),
            (
                "Commands",
                "View",
                vec![KeyCode::Char('v')],
                CommandTreeNode::new_children(),
            ),
            (
                "View",
                "Selection",
                vec![KeyCode::Char('v'), KeyCode::Char('v')],
                CommandTreeNode::new_action(Message::View),
            ),
            (
                "View",
                "From selection to @",
                vec![KeyCode::Char('v'), KeyCode::Char('f')],
                CommandTreeNode::new_action(Message::ViewFromSelection),
            ),
            (
                "View",
                "From @ to selection",
                vec![KeyCode::Char('v'), KeyCode::Char('t')],
                CommandTreeNode::new_action(Message::ViewToSelection),
            ),
            (
                "View",
                "From selection to destination",
                vec![KeyCode::Char('v'), KeyCode::Char('V')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "View to destination",
                "Select destination",
                vec![KeyCode::Char('v'), KeyCode::Char('V'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::ViewFromSelectionToDestination),
            ),
            (
                "Commands",
                "Revert",
                vec![KeyCode::Char('V')],
                CommandTreeNode::new_children(),
            ),
            (
                "Revert",
                "Selection onto @",
                vec![KeyCode::Char('V'), KeyCode::Char('v')],
                CommandTreeNode::new_action(Message::Revert),
            ),
            (
                "Revert",
                "Selection onto destination",
                vec![KeyCode::Char('V'), KeyCode::Char('o')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Revert onto",
                "Select destination",
                vec![KeyCode::Char('V'), KeyCode::Char('o'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RevertOntoDestination),
            ),
            (
                "Revert",
                "Selection after destination",
                vec![KeyCode::Char('V'), KeyCode::Char('a')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Revert after",
                "Select destination",
                vec![KeyCode::Char('V'), KeyCode::Char('a'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RevertInsertAfter),
            ),
            (
                "Revert",
                "Selection before destination",
                vec![KeyCode::Char('V'), KeyCode::Char('b')],
                CommandTreeNode::new_action_with_children(Message::SaveSelection),
            ),
            (
                "Revert before",
                "Select destination",
                vec![KeyCode::Char('V'), KeyCode::Char('b'), KeyCode::Enter],
                CommandTreeNode::new_action(Message::RevertInsertBefore),
            ),
            (
                "Commands",
                "Undo",
                vec![KeyCode::Char('u')],
                CommandTreeNode::new_children(),
            ),
            (
                "Undo",
                "Undo last operation",
                vec![KeyCode::Char('u'), KeyCode::Char('u')],
                CommandTreeNode::new_action(Message::Undo),
            ),
            (
                "Undo",
                "Redo last operation",
                vec![KeyCode::Char('u'), KeyCode::Char('r')],
                CommandTreeNode::new_action(Message::Redo),
            ),
        ];

        let mut tree = Self(CommandTreeNode::new_children());
        tree.add_children(items);
        tree
    }
}

fn render_help_text(entries: HelpEntries) -> Text<'static> {
    const COL_WIDTH: usize = 26;
    const MAX_ENTRIES_PER_COL: usize = 14;

    // Get lines for each column, splitting if over MAX_ENTRIES_PER_COL
    let columns: Vec<Vec<Line>> = entries
        .into_iter()
        .flat_map(|(group_help_text, help_group)| {
            let chunks: Vec<Vec<(String, String)>> = help_group
                .chunks(MAX_ENTRIES_PER_COL)
                .map(|c| c.to_vec())
                .collect();

            chunks.into_iter().enumerate().map(move |(i, chunk)| {
                let mut col_lines = Vec::new();
                // First chunk gets the header, subsequent chunks get blank header
                let header = if i == 0 {
                    group_help_text.clone()
                } else {
                    String::new()
                };
                col_lines.push(Line::from(vec![Span::styled(
                    format!("{header:COL_WIDTH$}"),
                    Style::default().fg(Color::Blue),
                )]));
                col_lines.extend(chunk.into_iter().map(|(key, help)| {
                    let mut num_cols = key.len() + 1 + help.len();
                    if !key.is_ascii() {
                        num_cols -= 2;
                    }
                    let padding = " ".repeat(COL_WIDTH.saturating_sub(num_cols));
                    Line::from(vec![
                        Span::styled(key, Style::default().fg(Color::Green)),
                        Span::raw(" "),
                        Span::raw(help),
                        Span::raw(padding),
                    ])
                }));
                col_lines
            })
        })
        .collect();

    // Render the columns
    let num_rows = columns.iter().map(|c| c.len()).max().unwrap();
    let lines: Vec<Line> = (0..num_rows)
        .map(|i| {
            let mut spans: Vec<Span> = vec![Span::raw(" ")];

            for col in &columns {
                let empty_line = Line::from(Span::raw(" ".repeat(COL_WIDTH)));
                let col_line = col.get(i).unwrap_or(&empty_line).clone();
                spans.extend(col_line.spans)
            }

            Line::from(spans)
        })
        .collect();

    lines.into()
}

pub fn display_unbound_error_lines(info_list: &mut Option<Text<'static>>, key_code: &KeyCode) {
    let error_line = Line::from(vec![
        Span::styled(" Unbound suffix: ", Style::default().fg(Color::Red)),
        Span::raw("'"),
        Span::styled(format!("{key_code}"), Style::default().fg(Color::Green)),
        Span::raw("'"),
    ]);
    match info_list {
        None => {
            *info_list = Some(error_line.into());
        }
        Some(info_list) => {
            let add_blank_line = info_list.lines.first().unwrap().spans[0] != error_line.spans[0];
            if let Some(last_line) = info_list.lines.last()
                && !last_line.spans.is_empty()
                && last_line.spans[0] == error_line.spans[0]
            {
                info_list.lines.pop();
                info_list.lines.pop();
            }

            if add_blank_line {
                info_list.lines.push(Line::from(vec![]));
            }
            info_list.lines.push(error_line);
        }
    }
}
