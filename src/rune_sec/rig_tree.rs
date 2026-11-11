//! Rig tree panel (Roadmap Wave 2, W2-21 shell +
//! W2-22 rows — roadmap P-8, RM19 §3): the Build
//! workspace's home for the selected figure's node
//! tree, mounted under
//! [`crate::rune_sec::workspaces::Section::RigTree`]
//! in the Build workspace only (the mount itself is
//! IA-W2-21-5; `main.rs` is an integrator surface).
//!
//! The rows are the selected figure's `def.nodes` in
//! tree order (a pre-order walk over `parent` links),
//! indent = depth. Each row carries the at-a-glance
//! facts RM19 §3 names: the node name plus badge
//! glyphs for **head**, **IK end**, **joint limits**
//! (either bound set) and **segment media**. One
//! selection is shared by the stage, both inspectors
//! and this tree: a row press writes
//! `core.selected_figure` + `core.selected_node`
//! (shell selection only — no undo step, never marks
//! dirty, the `shell_select_node` discipline), and
//! the selected node's row renders active wherever
//! the selection came from.
//!
//! Collapse state is panel-owned session state in a
//! `thread_local` (the clips/camera panels' pattern),
//! keyed to the figure it was collected on; it is
//! view state only — it never touches the project.
//!
//! Keyboard navigation is implemented as pure helpers
//! over the def ([`rig_nav_up`] / [`rig_nav_down`] /
//! [`rig_nav_left`] / [`rig_nav_right`]) plus one
//! glue entry point, [`rig_tree_key`], that the shell's
//! `on_key` can call; the `on_key` arm itself is
//! IA-W2-22-1 (main.rs + keymap are integrator
//! surfaces, and the arrow scope interacts with
//! W2-02's selection-based nudge rule). The convention
//! is the tree standard: Up/Down move over the VISIBLE
//! rows, Left collapses an expanded parent or steps
//! to the parent, Right expands a collapsed parent or
//! steps to its first child.
//!
//! The structural actions row (Add / Insert /
//! Duplicate / Mirror / Delete subtree / Reparent)
//! lands with the core ops in W2-23…W2-25 — a button
//! whose op does not exist yet would be a fake
//! control. W2-23 landed the first one: the
//! **Reparent** button (counted key `rig-reparent`,
//! RM19 §7's price), riding the core's
//! `cycle_reparent_selected` — each press moves the
//! selected node's parent to the next legal parent in
//! tree order (its own subtree is never a candidate),
//! one "Reparent node" undo step per press. It renders
//! live exactly when a cycle could succeed (a node is
//! selected, is not the root, the figure is unlocked,
//! and another legal parent exists) and as the inert
//! twin otherwise, so a press can never fire a refusal.
//! The remaining buttons join it as W2-24/W2-25 land
//! their ops.
//!
//! W2-24 landed two more: **Insert** (counted key
//! `rig-insert`) splits the selected segment at its
//! midpoint with a new collinear node
//! (`EditorCore::insert_segment_between`, RM19 §4.2 —
//! the rest pose is preserved exactly), and
//! **Duplicate** (counted key `rig-duplicate`)
//! deep-copies the selected subtree as a sibling
//! subtree starting at rest
//! (`EditorCore::duplicate_subtree`, RM19 §4.3). Both
//! follow Reparent's discipline: live exactly when the
//! op would succeed on the current selection (see
//! `structural_available`), inert otherwise, one
//! labelled undo step per press.
//!
//! W2-25 landed the last two ops' buttons: **Mirror**
//! (counted key `rig-mirror`) reflects the selected
//! subtree across the figure's vertical axis
//! (`EditorCore::mirror_subtree`, RM19 §4.5 — rests,
//! bends, names and image flips mirror together), and
//! **Delete subtree** (counted key
//! `rig-delete-subtree`) removes the selected subtree
//! with the full §4.0 index remap
//! (`EditorCore::delete_subtree`, RM19 §4.4). Their
//! refusal sets coincide with Insert/Duplicate's
//! (figure present and unlocked, node selected, not
//! the root), so they ride the same
//! `structural_available` mirror. The row now reads
//! in RM19 §3's order: Insert, Duplicate, Mirror,
//! Delete subtree, Reparent. **Add** is the one
//! priced button still absent: its op
//! (`add_segment`) exists, but no Wave-2 seat owns
//! the button — it is filed as IA-W2-25-2 rather
//! than smuggled into this seat.
//!
//! Element keys: the card root `rig-tree` is counted
//! (W2-21). Row keys `rig-node-{i}` and disclosure
//! keys `rig-disclose-{i}` are variable-carried
//! (format-generated from the node index — outside
//! the gate counter, the AC1/RM12 precedent RM19 §3
//! prescribed).
//!
//! Copy: the title resolves through the i18n wire
//! (key `rig_tree`, filed as IA-W2-21-3 — until the
//! integrator splices it ×28 the wire falls back to
//! the key itself, the shell's adopt-ahead idiom);
//! the body lines are plain literals, the history
//! panel's precedent.

use std::cell::RefCell;
use std::collections::BTreeSet;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El, ScrollSpec};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{FigureDef, SegmentMedia};

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icons;
use crate::State;

/// How tall the row list may grow before it scrolls —
/// nine rows at the kit's 28 pt row height plus the
/// gaps between them. Past the cap the list scrolls
/// internally (the history panel's pattern), so a
/// big rig never pushes the Build column off the
/// workspace.
const RIG_LIST_MAX_H: f32 = 9.0 * 28.0 + 8.0 * 4.0;

/// Indent per tree depth, in points.
const RIG_INDENT: f32 = 14.0;

/// Badge glyph size, in points.
const BADGE_SIZE: f32 = 11.0;

thread_local! {
    /// Collapsed parent nodes for the figure the tree
    /// is showing, as (figure index, node set). Panel
    /// session state only (see the module docs): when
    /// the selected figure changes, the stored set is
    /// dropped rather than applied to a figure it was
    /// never collected on.
    static COLLAPSED: RefCell<(usize, BTreeSet<usize>)> =
        RefCell::new((usize::MAX, BTreeSet::new()));
}

/// The collapsed set for figure `fi`, resetting the
/// stored set when the figure has changed since it
/// was collected.
fn collapsed_for(fi: usize) -> BTreeSet<usize> {
    COLLAPSED.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.0 != fi {
            *slot = (fi, BTreeSet::new());
        }
        slot.1.clone()
    })
}

/// Toggle node `ni`'s membership of figure `fi`'s
/// collapsed set (the disclosure press).
fn toggle_collapsed(fi: usize, ni: usize) {
    COLLAPSED.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.0 != fi {
            *slot = (fi, BTreeSet::new());
        }
        if !slot.1.remove(&ni) {
            slot.1.insert(ni);
        }
    });
}

/// Write `ni`'s collapsed membership for figure `fi`
/// to an exact value (the keyboard nav's setter —
/// Left/Right SET the state, they do not toggle it).
fn set_collapsed(fi: usize, ni: usize, collapsed: bool) {
    COLLAPSED.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.0 != fi {
            *slot = (fi, BTreeSet::new());
        }
        if collapsed {
            slot.1.insert(ni);
        } else {
            slot.1.remove(&ni);
        }
    });
}

/// The panel's read model, as data: everything the
/// card shows about the selected figure, factored out
/// of the builder so it is unit-testable without a
/// tree (the history panel's `history_rows` pattern).
#[derive(Clone, Debug, PartialEq)]
pub struct RigSummary {
    /// The selected figure instance's name.
    pub figure_name: String,
    /// How many nodes (segments) its rig has.
    pub node_count: usize,
    /// The selected node's name, when a node is
    /// selected and the index still names a node.
    pub selected_node: Option<String>,
}

/// The summary for the core's selected figure, or
/// `None` when the project has no figure at the
/// selected index (an empty project, or a selection
/// left past the end by a delete).
pub fn rig_summary(core: &EditorCore) -> Option<RigSummary> {
    let fig = core.project.figures.get(core.selected_figure)?;
    let selected_node = core
        .selected_node
        .and_then(|ni| fig.def.nodes.get(ni))
        .map(|n| n.name.clone());
    Some(RigSummary {
        figure_name: fig.name.clone(),
        node_count: fig.def.nodes.len(),
        selected_node,
    })
}

/// One rig-tree row, as data: a node, its tree depth,
/// and the badge facts its rendering shows. Pure over
/// the def + collapse set, so the walk is unit-testable
/// without a tree.
#[derive(Clone, Debug, PartialEq)]
pub struct RigRow {
    /// The node's index in `def.nodes`.
    pub node: usize,
    /// Tree depth (root = 0); the row's indent.
    pub depth: usize,
    /// The node's name.
    pub name: String,
    /// Badge: this node draws a head circle.
    pub head: bool,
    /// Badge: this node is an IK end effector.
    pub ik_end: bool,
    /// Badge: at least one joint-limit bound is set.
    pub limited: bool,
    /// Badge: a segment-media kind, when attached —
    /// the badge glyph differs for image vs text.
    pub media: Option<RigMedia>,
    /// The node has children (a disclosure renders).
    pub has_children: bool,
    /// The node is in the collapsed set (its subtree
    /// is hidden below this row).
    pub collapsed: bool,
    /// This row is the shared selection.
    pub selected: bool,
}

/// The segment-media badge kinds (RM19 §3's "segment
/// media present", split by kind so the glyph can say
/// which).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigMedia {
    Image,
    Text,
}

impl RigMedia {
    /// The badge's icon name for this media kind.
    fn icon(self) -> &'static str {
        match self {
            RigMedia::Image => "image",
            RigMedia::Text => "font",
        }
    }
    /// The badge's tooltip for this media kind.
    fn label(self) -> &'static str {
        match self {
            RigMedia::Image => "Segment media (image)",
            RigMedia::Text => "Segment media (text)",
        }
    }
}

/// The children of node `ni`, in node-index order
/// (the order the def stores them — the walk's one
/// ordering rule). Out-of-range `ni` yields none.
pub fn rig_children(def: &FigureDef, ni: usize) -> Vec<usize> {
    if ni >= def.nodes.len() {
        return Vec::new();
    }
    (0..def.nodes.len())
        .filter(|&i| def.nodes[i].parent == Some(ni))
        .collect()
}

/// The visible rows of the tree: a pre-order walk
/// from the roots (nodes with no parent), descending
/// into a node's children unless the node is in
/// `collapsed`. Malformed parent links (out of range,
/// self-parented, cycles) cannot hang or panic the
/// walk: a visited set bounds it, and a node whose
/// parent names nothing real is treated as a root so
/// it still shows.
pub fn rig_rows(
    def: &FigureDef,
    collapsed: &BTreeSet<usize>,
    selected: Option<usize>,
) -> Vec<RigRow> {
    fn is_root(def: &FigureDef, ni: usize) -> bool {
        match def.nodes[ni].parent {
            None => true,
            Some(p) => p >= def.nodes.len() || p == ni,
        }
    }
    fn walk(
        def: &FigureDef,
        collapsed: &BTreeSet<usize>,
        selected: Option<usize>,
        ni: usize,
        depth: usize,
        visited: &mut BTreeSet<usize>,
        rows: &mut Vec<RigRow>,
    ) {
        if !visited.insert(ni) {
            return;
        }
        let node = &def.nodes[ni];
        let children = rig_children(def, ni);
        let is_collapsed = collapsed.contains(&ni);
        rows.push(RigRow {
            node: ni,
            depth,
            name: node.name.clone(),
            head: node.head,
            ik_end: node.ik_end,
            limited: node.has_limits(),
            media: node.media.as_ref().map(|m| match m {
                SegmentMedia::Image { .. } => RigMedia::Image,
                SegmentMedia::Text { .. } => RigMedia::Text,
            }),
            has_children: !children.is_empty(),
            collapsed: is_collapsed,
            selected: selected == Some(ni),
        });
        if !is_collapsed {
            for child in children {
                walk(def, collapsed, selected, child, depth + 1, visited, rows);
            }
        }
    }
    let mut rows = Vec::new();
    let mut visited = BTreeSet::new();
    for ni in 0..def.nodes.len() {
        if is_root(def, ni) {
            walk(def, collapsed, selected, ni, 0, &mut visited, &mut rows);
        }
    }
    // A node hidden inside a collapsed subtree is no
    // root's descendant either — but it is hidden by
    // the collapse, not orphaned: mark the collapsed
    // subtrees visited here, so the rescue below only
    // revives nodes no root reaches because their
    // parent links are malformed. Gated on the
    // collapsed node itself having a row, so a
    // collapsed node caught in a parent cycle is
    // still rescued as its own root.
    fn mark_hidden(
        def: &FigureDef,
        ni: usize,
        visited: &mut BTreeSet<usize>,
    ) {
        if !visited.insert(ni) {
            return;
        }
        for child in rig_children(def, ni) {
            mark_hidden(def, child, visited);
        }
    }
    for &c in collapsed {
        if visited.contains(&c) {
            for child in rig_children(def, c) {
                mark_hidden(def, child, &mut visited);
            }
        }
    }
    // A node caught in a parent cycle is no root's
    // descendant; show it as its own root rather than
    // dropping it from the tree entirely.
    for ni in 0..def.nodes.len() {
        if !visited.contains(&ni) {
            walk(def, collapsed, selected, ni, 0, &mut visited, &mut rows);
        }
    }
    rows
}

/// The node a row press selects: figure and node move
/// together (one shared selection with the stage and
/// the inspectors). Shell selection only — no undo
/// step, never marks dirty. Returns false when either
/// index names nothing (a stale press), leaving the
/// selection untouched.
pub fn select_rig_node(core: &mut EditorCore, fi: usize, ni: usize) -> bool {
    let Some(fig) = core.project.figures.get(fi) else {
        return false;
    };
    if ni >= fig.def.nodes.len() {
        return false;
    }
    core.selected_figure = fi;
    core.selected_node = Some(ni);
    true
}

/// Up: the visible row above `ni`'s, or `None` at the
/// top (or when `ni` is not visible — a selection
/// inside a collapsed subtree has no row to move
/// from).
pub fn rig_nav_up(rows: &[RigRow], ni: usize) -> Option<usize> {
    let pos = rows.iter().position(|r| r.node == ni)?;
    pos.checked_sub(1).map(|p| rows[p].node)
}

/// Down: the visible row below `ni`'s, or `None` at
/// the bottom (or when `ni` is not visible).
pub fn rig_nav_down(rows: &[RigRow], ni: usize) -> Option<usize> {
    let pos = rows.iter().position(|r| r.node == ni)?;
    rows.get(pos + 1).map(|r| r.node)
}

/// What one Left/Right press means, as data, so the
/// rule is pinnable without a shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigNav {
    /// Move the shared selection to this node.
    Select(usize),
    /// Set this node's collapsed membership (the
    /// selection stays where it is).
    SetCollapsed(usize, bool),
}

/// Left: collapse an expanded parent (selection
/// stays); otherwise step to the parent; at a root,
/// nothing. Per the tree convention in the module
/// docs.
pub fn rig_nav_left(def: &FigureDef, collapsed: &BTreeSet<usize>, ni: usize) -> Option<RigNav> {
    if ni >= def.nodes.len() {
        return None;
    }
    // A root is a node with no parent, or one whose parent link names nothing
    // real (the walk's is_root rule) — Left stops there either way.
    let is_root = match def.nodes[ni].parent {
        None => true,
        Some(p) => p >= def.nodes.len() || p == ni,
    };
    if is_root {
        return None;
    }
    if !rig_children(def, ni).is_empty() && !collapsed.contains(&ni) {
        return Some(RigNav::SetCollapsed(ni, true));
    }
    match def.nodes[ni].parent {
        Some(p) if p < def.nodes.len() && p != ni => Some(RigNav::Select(p)),
        _ => None,
    }
}

/// Right: expand a collapsed parent (selection
/// stays); otherwise step to the first child; at a
/// leaf, nothing.
pub fn rig_nav_right(def: &FigureDef, collapsed: &BTreeSet<usize>, ni: usize) -> Option<RigNav> {
    if ni >= def.nodes.len() {
        return None;
    }
    let children = rig_children(def, ni);
    if children.is_empty() {
        return None;
    }
    if collapsed.contains(&ni) {
        return Some(RigNav::SetCollapsed(ni, false));
    }
    Some(RigNav::Select(children[0]))
}

/// The four tree-navigation keys, named for the
/// `on_key` glue (IA-W2-22-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigKey {
    Up,
    Down,
    Left,
    Right,
}

/// One tree-navigation key press against the core's
/// selected figure, riding the panel's own collapse
/// set. Returns true when the press was consumed (a
/// nav step or a collapse change happened). With no
/// node selected, Down/Up enter the tree at its
/// first/last visible row; Left/Right do nothing.
/// With no figure, nothing is consumed.
pub fn rig_tree_key(core: &mut EditorCore, key: RigKey) -> bool {
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get(fi) else {
        return false;
    };
    let def = fig.def.clone();
    let collapsed = collapsed_for(fi);
    let rows = rig_rows(&def, &collapsed, core.selected_node);
    let Some(ni) = core.selected_node.filter(|n| *n < def.nodes.len()) else {
        return match key {
            RigKey::Down => rows
                .first()
                .map(|r| select_rig_node(core, fi, r.node))
                .unwrap_or(false),
            RigKey::Up => rows
                .last()
                .map(|r| select_rig_node(core, fi, r.node))
                .unwrap_or(false),
            RigKey::Left | RigKey::Right => false,
        };
    };
    let nav = match key {
        RigKey::Up => rig_nav_up(&rows, ni).map(RigNav::Select),
        RigKey::Down => rig_nav_down(&rows, ni).map(RigNav::Select),
        RigKey::Left => rig_nav_left(&def, &collapsed, ni),
        RigKey::Right => rig_nav_right(&def, &collapsed, ni),
    };
    match nav {
        Some(RigNav::Select(target)) => select_rig_node(core, fi, target),
        Some(RigNav::SetCollapsed(node, state)) => {
            set_collapsed(fi, node, state);
            true
        }
        None => false,
    }
}

/// One badge glyph with its tooltip, or nothing when
/// the fact does not hold for this node.
fn badge(on: bool, icon: &str, label: &str, t: &Theme) -> Option<El> {
    on.then(|| icons::icon_el(icon, BADGE_SIZE, t.text_secondary).help(label))
}

/// One tree row: disclosure, indent, name, badges.
/// The row press selects (figure + node); the
/// disclosure press toggles collapse and nothing
/// else.
fn rig_row_el(fi: usize, r: &RigRow, t: &Theme) -> El {
    let ni = r.node;
    let disclosure = if r.has_children {
        let glyph = if r.collapsed {
            "chevron-right"
        } else {
            "chevron-down"
        };
        icons::icon_el(glyph, 10.0, t.text_secondary)
            .key(format!("rig-disclose-{ni}"))
            .help(if r.collapsed { "Expand" } else { "Collapse" })
            .on_press(move |_: &mut State| toggle_collapsed(fi, ni))
    } else {
        el().w(10.0)
    };
    let mut line = design::list_row(&format!("rig-node-{ni}"), r.selected, t)
        .w_full()
        .child(el().w(r.depth as f32 * RIG_INDENT))
        .child(disclosure)
        .child(
            text(r.name.clone())
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0),
        );
    for b in [
        badge(r.head, "circle", "Head", t),
        badge(r.ik_end, "hand", "IK end", t),
        badge(r.limited, "lock", "Joint limits set", t),
        badge(
            r.media.is_some(),
            r.media.map(|m| m.icon()).unwrap_or("image"),
            r.media.map(|m| m.label()).unwrap_or("Segment media"),
            t,
        ),
    ]
    .into_iter()
    .flatten()
    {
        line = line.child(b);
    }
    line.on_press(move |s: &mut State| {
        select_rig_node(&mut s.core, fi, ni);
    })
}

/// Whether the Reparent button can act on the core's
/// current selection: a node is selected on an
/// unlocked figure, it is not the root, and at least
/// one other legal parent exists. This is exactly the
/// success condition of
/// `EditorCore::cycle_reparent_selected`, mirrored
/// read-side so the button renders live only when a
/// press will work (the pose panel's live/inert
/// discipline).
pub fn reparent_available(core: &EditorCore) -> bool {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return false;
    };
    if fig.locked {
        return false;
    }
    let Some(ni) = core.selected_node.filter(|n| *n < fig.def.nodes.len()) else {
        return false;
    };
    if fig.def.nodes[ni].parent.is_none() {
        return false;
    }
    !EditorCore::legal_parents(&fig.def, ni).is_empty()
}

/// The actions row's last button (W2-23): "Reparent"
/// cycles the selected node's parent through the
/// legal parents in tree order (see the module docs).
/// Keyed `rig-reparent` (counted; RM19 §7 priced it
/// in the rig-tree key set). The remaining structural
/// buttons landed with W2-24/W2-25's ops.
fn reparent_button(core: &EditorCore, t: &Theme) -> El {
    if reparent_available(core) {
        btn("rig-reparent", "Reparent", false, t, |s| {
            s.core.cycle_reparent_selected();
        })
    } else {
        btn_inert("rig-reparent", "Reparent", t)
    }
}

/// Whether Insert or Duplicate can succeed on the
/// current selection (W2-24): a figure is present and
/// unlocked, a node is selected and in range, and it
/// is not the root. The two ops' refusal sets coincide
/// exactly today — insert refuses the root because it
/// has no segment to split, duplicate because the
/// clone root would have no parent to anchor on — so
/// one read-side mirror serves both buttons, exactly
/// the success condition of
/// `EditorCore::insert_segment_between` /
/// `EditorCore::duplicate_subtree` (the pose panel's
/// live/inert discipline, as with
/// [`reparent_available`]).
fn structural_available(core: &EditorCore) -> bool {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return false;
    };
    if fig.locked {
        return false;
    }
    let Some(ni) = core.selected_node.filter(|n| *n < fig.def.nodes.len()) else {
        return false;
    };
    fig.def.nodes[ni].parent.is_some()
}

/// See [`structural_available`].
pub fn insert_available(core: &EditorCore) -> bool {
    structural_available(core)
}

/// See [`structural_available`].
pub fn duplicate_available(core: &EditorCore) -> bool {
    structural_available(core)
}

/// See [`structural_available`]. W2-25:
/// `EditorCore::mirror_subtree`'s refusal set is the
/// same four (no figure, stale node, locked, root —
/// the root is the figure's mirror axis).
pub fn mirror_available(core: &EditorCore) -> bool {
    structural_available(core)
}

/// See [`structural_available`]. W2-25:
/// `EditorCore::delete_subtree`'s refusal set is the
/// same four (no figure, stale node, locked, root —
/// deleting the root would clear the whole figure).
pub fn delete_subtree_available(core: &EditorCore) -> bool {
    structural_available(core)
}

/// "Insert" (W2-24): split the selected segment at
/// its midpoint with a new collinear node
/// (`EditorCore::insert_segment_between`, RM19 §4.2).
/// Keyed `rig-insert` (counted; RM19 §7 priced it in
/// the rig-tree key set). The press resolves the
/// selection at press time and calls the op; the
/// live/inert rendering above means the op's refusals
/// can never fire from here.
fn insert_button(core: &EditorCore, t: &Theme) -> El {
    if insert_available(core) {
        btn("rig-insert", "Insert", false, t, |s| {
            if let Some(ni) = s.core.selected_node {
                s.core.insert_segment_between(ni);
            }
        })
    } else {
        btn_inert("rig-insert", "Insert", t)
    }
}

/// "Duplicate" (W2-24): deep-copy the selected node's
/// subtree as a sibling subtree
/// (`EditorCore::duplicate_subtree`, RM19 §4.3).
/// Keyed `rig-duplicate` (counted; RM19 §7 priced it
/// in the rig-tree key set). Same press discipline as
/// [`insert_button`].
fn duplicate_button(core: &EditorCore, t: &Theme) -> El {
    if duplicate_available(core) {
        btn("rig-duplicate", "Duplicate", false, t, |s| {
            if let Some(ni) = s.core.selected_node {
                s.core.duplicate_subtree(ni);
            }
        })
    } else {
        btn_inert("rig-duplicate", "Duplicate", t)
    }
}

/// "Mirror" (W2-25): reflect the selected node's
/// subtree across the figure's vertical axis
/// (`EditorCore::mirror_subtree`, RM19 §4.5). Keyed
/// `rig-mirror` (counted; RM19 §7 priced it in the
/// rig-tree key set). Same press discipline as
/// [`insert_button`].
fn mirror_button(core: &EditorCore, t: &Theme) -> El {
    if mirror_available(core) {
        btn("rig-mirror", "Mirror", false, t, |s| {
            if let Some(ni) = s.core.selected_node {
                s.core.mirror_subtree(ni);
            }
        })
    } else {
        btn_inert("rig-mirror", "Mirror", t)
    }
}

/// "Delete subtree" (W2-25): remove the selected
/// node's whole subtree, remapping every
/// node-indexed store (`EditorCore::delete_subtree`,
/// RM19 §4.4). Keyed `rig-delete-subtree` (counted;
/// RM19 §7 priced it in the rig-tree key set). Same
/// press discipline as [`insert_button`] — the
/// live/inert mirror means the destructive op's
/// refusals can never fire from here.
fn delete_subtree_button(core: &EditorCore, t: &Theme) -> El {
    if delete_subtree_available(core) {
        btn("rig-delete-subtree", "Delete subtree", false, t, |s| {
            if let Some(ni) = s.core.selected_node {
                s.core.delete_subtree(ni);
            }
        })
    } else {
        btn_inert("rig-delete-subtree", "Delete subtree", t)
    }
}

/// The Rig tree card: header (title + segment count),
/// then either the empty state (no figure to rig) or
/// the selected figure's name line and its node rows.
pub fn rig_tree_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let summary = rig_summary(core);
    let meta = summary
        .as_ref()
        .map(|sum| format!("{} segments", sum.node_count));
    let mut card = design::panel_col("rig-tree", t)
        .child(design::panel_title(
            Some("bone"),
            &i18n_wire::tr(core, "rig_tree"),
            meta.as_deref(),
            t,
        ))
        .child(design::divider(t));
    let Some(sum) = summary else {
        return card.child(design::empty_state(
            "bone",
            "No figure to rig — add one from the Figures panel.",
            t,
        ));
    };
    let fi = core.selected_figure;
    let fig = &core.project.figures[fi];
    let collapsed = collapsed_for(fi);
    let rows = rig_rows(&fig.def, &collapsed, core.selected_node);
    let mut list = col().gap(4.0);
    for r in &rows {
        list = list.child(rig_row_el(fi, r, t));
    }
    card = card.child(
        text(sum.figure_name.clone())
            .font(type_::LABEL)
            .color(t.text),
    );
    card = card.child(
        list.scroll(ScrollSpec {
            stick_bottom: false,
        })
        .min_h(0.0)
        .max_h(RIG_LIST_MAX_H),
    );
    card.child(
        row()
            .gap(4.0)
            .child(insert_button(core, t))
            .child(duplicate_button(core, t))
            .child(mirror_button(core, t))
            .child(delete_subtree_button(core, t))
            .child(reparent_button(core, t)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::FigureDef;

    #[test]
    fn summary_reads_the_selected_figure() {
        // The demo project: one humanoid figure "Styk"
        // with 11 nodes, nothing selected.
        let core = EditorCore::new();
        let sum = rig_summary(&core).expect("demo project has a figure");
        assert_eq!(sum.figure_name, "Styk");
        assert_eq!(sum.node_count, 11);
        assert_eq!(sum.selected_node, None);
    }

    #[test]
    fn summary_names_the_selected_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(2);
        let sum = rig_summary(&core).expect("demo project has a figure");
        assert_eq!(sum.selected_node.as_deref(), Some("head"));
        // A stale index past the end reads as no
        // selection, never a panic or a wrong name.
        core.selected_node = Some(999);
        let sum = rig_summary(&core).expect("demo project has a figure");
        assert_eq!(sum.selected_node, None);
    }

    #[test]
    fn summary_is_none_without_a_figure() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(rig_summary(&core), None);
        // A selection index past the end is likewise
        // figure-less.
        let mut core = EditorCore::new();
        core.selected_figure = 7;
        assert_eq!(rig_summary(&core), None);
    }

    /// The humanoid def with one flag of each badge
    /// kind planted: node 4 becomes an IK end, node 3
    /// gains image media.
    fn badged_def() -> FigureDef {
        let mut def = FigureDef::humanoid();
        def.nodes[4].ik_end = true;
        def.nodes[3].media = Some(SegmentMedia::Image {
            asset: 0,
            offset: [0.0, 0.0],
            scale: 1.0,
            angle_offset: 0.0,
            flip: false,
            opacity: 1.0,
        });
        def
    }

    #[test]
    fn rows_walk_preorder_with_depths() {
        // RM19's named pin: rig_tree_rows_follow_parent_depth.
        let def = FigureDef::humanoid();
        let rows = rig_rows(&def, &BTreeSet::new(), None);
        let order: Vec<usize> = rows.iter().map(|r| r.node).collect();
        assert_eq!(order, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let depths: Vec<usize> = rows.iter().map(|r| r.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 2, 3, 2, 3, 1, 2, 1, 2]);
        // The root and chest are parents; the head is not.
        assert!(rows[0].has_children && rows[1].has_children);
        assert!(!rows[2].has_children);
        assert_eq!(rows[1].name, "chest");
    }

    #[test]
    fn rows_carry_the_badges() {
        let def = badged_def();
        let rows = rig_rows(&def, &BTreeSet::new(), Some(4));
        // Head badge on node 2 only.
        assert!(rows[2].head);
        assert!(rows.iter().filter(|r| r.head).count() == 1);
        // IK-end badge where planted.
        assert!(rows[4].ik_end);
        assert!(!rows[3].ik_end);
        // Limit badges: the humanoid constructor sets
        // elbow (4, 6) and knee (8, 10) limits.
        for ni in [4, 6, 8, 10] {
            assert!(rows[ni].limited, "node {ni} carries limits");
        }
        assert!(!rows[0].limited && !rows[2].limited);
        // Media badge where planted, kind-split.
        assert_eq!(rows[3].media, Some(RigMedia::Image));
        assert_eq!(rows[0].media, None);
        // The selection flag rides the shared selection.
        assert!(rows[4].selected);
        assert!(!rows[3].selected);
    }

    #[test]
    fn scene_build_workspace_row_census_is_pinned() {
        // The row census W2-28's `build-workspace`
        // headless scene (scenes.rs fact 38) asserts of
        // the rig card, pinned here at the row level:
        // the demo humanoid exactly as the scene leaves
        // it — its native badges (head on node 2; IK
        // ends and joint limits on nodes 4, 6, 8, 10,
        // all from the constructor) plus ONE planted
        // image media on the head node, node 4 selected.
        let mut def = FigureDef::humanoid();
        def.nodes[2].media = Some(SegmentMedia::Image {
            asset: 0,
            offset: [0.0, 0.0],
            scale: 1.0,
            angle_offset: 0.0,
            flip: false,
            opacity: 1.0,
        });
        let rows = rig_rows(&def, &BTreeSet::new(), Some(4));
        assert_eq!(rows.len(), 11, "every node renders a row");
        let parents: Vec<usize> = rows
            .iter()
            .filter(|r| r.has_children)
            .map(|r| r.node)
            .collect();
        assert_eq!(parents, vec![0, 1, 3, 5, 7, 9], "six disclosures");
        assert_eq!(rows.iter().filter(|r| r.head).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.ik_end).count(), 4);
        assert_eq!(rows.iter().filter(|r| r.limited).count(), 4);
        assert_eq!(rows.iter().filter(|r| r.media.is_some()).count(), 1);
        assert_eq!(rows[2].media, Some(RigMedia::Image));
        assert!(rows[4].selected);
        assert_eq!(rows.iter().filter(|r| r.selected).count(), 1);
    }

    #[test]
    fn collapsed_rows_hide_the_subtree() {
        let def = FigureDef::humanoid();
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        let rows = rig_rows(&def, &collapsed, None);
        let order: Vec<usize> = rows.iter().map(|r| r.node).collect();
        // Chest (1) shows, marked collapsed; its whole
        // subtree (2..6) is hidden; the legs remain.
        assert_eq!(order, vec![0, 1, 7, 8, 9, 10]);
        assert!(rows[1].collapsed && rows[1].has_children);
    }

    #[test]
    fn nav_moves_over_visible_rows_only() {
        let def = FigureDef::humanoid();
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        let rows = rig_rows(&def, &collapsed, None);
        // Down from the root lands on the chest; down
        // from the chest skips its hidden subtree.
        assert_eq!(rig_nav_down(&rows, 0), Some(1));
        assert_eq!(rig_nav_down(&rows, 1), Some(7));
        assert_eq!(rig_nav_up(&rows, 7), Some(1));
        // Ends of the list stop.
        assert_eq!(rig_nav_up(&rows, 0), None);
        assert_eq!(rig_nav_down(&rows, 10), None);
        // A selection hidden inside the collapsed
        // subtree has no row to move from.
        assert_eq!(rig_nav_down(&rows, 4), None);
    }

    #[test]
    fn nav_left_collapses_then_steps_to_parent() {
        let def = FigureDef::humanoid();
        let open = BTreeSet::new();
        // Expanded parent: Left collapses, selection stays.
        assert_eq!(
            rig_nav_left(&def, &open, 1),
            Some(RigNav::SetCollapsed(1, true))
        );
        // Collapsed parent: Left steps to the parent.
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        assert_eq!(
            rig_nav_left(&def, &collapsed, 1),
            Some(RigNav::Select(0))
        );
        // A leaf steps to its parent; the root stops.
        assert_eq!(rig_nav_left(&def, &open, 4), Some(RigNav::Select(3)));
        assert_eq!(rig_nav_left(&def, &open, 0), None);
    }

    #[test]
    fn nav_right_expands_then_descends() {
        let def = FigureDef::humanoid();
        let open = BTreeSet::new();
        // Expanded parent: Right steps to first child.
        assert_eq!(rig_nav_right(&def, &open, 1), Some(RigNav::Select(2)));
        // Collapsed parent: Right expands, selection stays.
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        assert_eq!(
            rig_nav_right(&def, &collapsed, 1),
            Some(RigNav::SetCollapsed(1, false))
        );
        // A leaf stops.
        assert_eq!(rig_nav_right(&def, &open, 2), None);
    }

    #[test]
    fn select_rig_node_writes_figure_and_node_together() {
        let mut core = EditorCore::new();
        assert!(select_rig_node(&mut core, 0, 5));
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, Some(5));
        // Stale indices refuse and leave the selection.
        assert!(!select_rig_node(&mut core, 0, 999));
        assert!(!select_rig_node(&mut core, 9, 0));
        assert_eq!(core.selected_node, Some(5));
        // Selection only: no undo step, no dirty mark —
        // the undo depth is still the fresh core's.
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn tree_key_enters_moves_and_collapses() {
        let mut core = EditorCore::new();
        // No selection: Down enters at the first row.
        assert!(rig_tree_key(&mut core, RigKey::Down));
        assert_eq!(core.selected_node, Some(0));
        assert!(rig_tree_key(&mut core, RigKey::Down));
        assert_eq!(core.selected_node, Some(1));
        // Left on the expanded chest collapses it; the
        // selection stays on the chest…
        assert!(rig_tree_key(&mut core, RigKey::Left));
        assert_eq!(core.selected_node, Some(1));
        // …and Down now skips the hidden subtree.
        assert!(rig_tree_key(&mut core, RigKey::Down));
        assert_eq!(core.selected_node, Some(7));
        // Right descends thigh → shin; Right on the
        // leaf shin does nothing.
        assert!(rig_tree_key(&mut core, RigKey::Right));
        assert_eq!(core.selected_node, Some(8));
        assert!(!rig_tree_key(&mut core, RigKey::Right));
        // Up twice returns to the chest; Right there
        // re-expands it, selection unmoved.
        assert!(rig_tree_key(&mut core, RigKey::Up));
        assert!(rig_tree_key(&mut core, RigKey::Up));
        assert_eq!(core.selected_node, Some(1));
        assert!(rig_tree_key(&mut core, RigKey::Right));
        assert_eq!(core.selected_node, Some(1));
        // A figure-less core consumes nothing.
        core.project.figures.clear();
        assert!(!rig_tree_key(&mut core, RigKey::Down));
    }

    #[test]
    fn reparent_button_availability_mirrors_the_op() {
        // W2-23: the button is live exactly when a
        // cycle press would succeed.
        let mut core = EditorCore::new();
        // Nothing selected: inert.
        assert!(!reparent_available(&core));
        // The root can never reparent: inert.
        core.selected_node = Some(0);
        assert!(!reparent_available(&core));
        // A parented node on an unlocked figure: live.
        core.selected_node = Some(4);
        assert!(reparent_available(&core));
        // A locked figure: inert.
        core.project.figures[0].locked = true;
        assert!(!reparent_available(&core));
        core.project.figures[0].locked = false;
        // A stale selection: inert, never a panic.
        core.selected_node = Some(999);
        assert!(!reparent_available(&core));
        // No figure at all: inert.
        core.project.figures.clear();
        assert!(!reparent_available(&core));
    }

    #[test]
    fn insert_duplicate_button_availability_mirrors_the_ops() {
        // W2-24: both buttons are live exactly when
        // their op would succeed — the two refusal sets
        // coincide (see `structural_available`).
        let mut core = EditorCore::new();
        // Nothing selected: inert.
        assert!(!insert_available(&core));
        assert!(!duplicate_available(&core));
        // The root can neither split nor duplicate:
        // inert.
        core.selected_node = Some(0);
        assert!(!insert_available(&core));
        assert!(!duplicate_available(&core));
        // A parented node on an unlocked figure: live.
        core.selected_node = Some(4);
        assert!(insert_available(&core));
        assert!(duplicate_available(&core));
        // A locked figure: inert.
        core.project.figures[0].locked = true;
        assert!(!insert_available(&core));
        assert!(!duplicate_available(&core));
        core.project.figures[0].locked = false;
        // A stale selection: inert, never a panic.
        core.selected_node = Some(999);
        assert!(!insert_available(&core));
        assert!(!duplicate_available(&core));
        // No figure at all: inert.
        core.project.figures.clear();
        assert!(!insert_available(&core));
        assert!(!duplicate_available(&core));
    }

    #[test]
    fn mirror_delete_button_availability_mirrors_the_ops() {
        // W2-25: both buttons are live exactly when
        // their op would succeed — the refusal sets
        // coincide with Insert/Duplicate's (see
        // `structural_available`).
        let mut core = EditorCore::new();
        // Nothing selected: inert.
        assert!(!mirror_available(&core));
        assert!(!delete_subtree_available(&core));
        // The root can neither mirror (it is the
        // figure's axis) nor delete (it would clear
        // the figure): inert.
        core.selected_node = Some(0);
        assert!(!mirror_available(&core));
        assert!(!delete_subtree_available(&core));
        // A parented node on an unlocked figure: live.
        core.selected_node = Some(4);
        assert!(mirror_available(&core));
        assert!(delete_subtree_available(&core));
        // A locked figure: inert.
        core.project.figures[0].locked = true;
        assert!(!mirror_available(&core));
        assert!(!delete_subtree_available(&core));
        core.project.figures[0].locked = false;
        // A stale selection: inert, never a panic.
        core.selected_node = Some(999);
        assert!(!mirror_available(&core));
        assert!(!delete_subtree_available(&core));
        // No figure at all: inert.
        core.project.figures.clear();
        assert!(!mirror_available(&core));
        assert!(!delete_subtree_available(&core));
    }

    #[test]
    fn panel_thread_local_round_trips() {
        // The collapse set is keyed to its figure: a
        // toggle collected on figure 0 must not leak
        // onto figure 1. The store holds ONE figure's
        // set, so reading figure 1's set drops figure
        // 0's (see COLLAPSED) — re-collect it after
        // the no-leak read before completing the
        // round trip.
        toggle_collapsed(0, 1);
        assert!(collapsed_for(0).contains(&1));
        assert!(collapsed_for(1).is_empty());
        toggle_collapsed(0, 1);
        toggle_collapsed(0, 1);
        assert!(collapsed_for(0).is_empty());
    }
}
