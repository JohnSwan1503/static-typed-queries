use core::marker::PhantomData;

use super::error::fail;
use super::{Offsets, Size};
use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::inject::Inject;
use crate::node::kind::Kind;
use crate::node::name::Name;
use crate::part::Part;
use crate::part::from::From;
use crate::part::from::rule::AliasRule;
use crate::part::param::Param;
use crate::statement::bind::Bind;
use crate::statement::bind::path::Path;
use crate::statement::bind::slot::Slot;

const MAX_CTES: usize = 64;
const MAX_HOOKS: usize = 64;
const MAX_NUMBERED: usize = 1024;

#[derive(Clone, Copy)]
struct Cte {
    node: &'static Node,
    path: Path,
    suffix: u16,
}

#[derive(Clone, Copy)]
struct Hook {
    name: Name,
    node: &'static Node,
    path: Path,
    after: bool,
}

#[derive(Clone, Copy)]
struct Numbered {
    path: Path,
    slot: Slot,
}

pub(super) struct Renderer<'a, D> {
    root: Option<&'static Node>,
    sql: &'a mut [u8],
    binds: &'a mut [Bind],
    offsets: &'a mut [Offsets],
    size: Size,
    rendered: usize,
    first_bind: usize,
    hooks: [Option<Hook>; MAX_HOOKS],
    ctes: [Option<Cte>; MAX_CTES],
    cte_count: usize,
    recursive: bool,
    numbered: [Option<Numbered>; MAX_NUMBERED],
    numbered_count: usize,
    dialect: PhantomData<D>,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn new(
        sql: &'a mut [u8],
        binds: &'a mut [Bind],
        offsets: &'a mut [Offsets],
    ) -> Self {
        Self {
            root: None,
            sql,
            binds,
            offsets,
            size: Size {
                sql: 0,
                binds: 0,
                before: 0,
                after: 0,
            },
            rendered: 0,
            first_bind: 0,
            hooks: [None; MAX_HOOKS],
            ctes: [None; MAX_CTES],
            cte_count: 0,
            recursive: false,
            numbered: [None; MAX_NUMBERED],
            numbered_count: 0,
            dialect: PhantomData,
        }
    }

    pub(super) const fn size(&self) -> Size {
        self.size
    }

    pub(super) const fn statement(&mut self, root: &'static Node) {
        self.root = Some(root);
        let (main, path) = target(root, Path::ROOT);
        self.find_hooks(main, path);
        self.render_hooks(false);
        self.single(root.name, main, path);
        self.render_hooks(true);
    }

    const fn find_hooks(&mut self, node: &'static Node, path: Path) {
        self.add_hooks(node, path, false);
        self.add_hooks(node, path, true);
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            let child = match parts[i] {
                Part::Expr(expr) => Some(expr.as_ref()),
                Part::From(from) => Some(from.node()),
                Part::Lit(_) | Part::Ident(_) | Part::Param(_) => None,
            };
            if let Some(child) = child {
                self.find_hooks(child, self.child_path(path, child));
            }
            i += 1;
        }
    }

    const fn add_hooks(&mut self, owner: &'static Node, path: Path, after: bool) {
        let hooks = if after { owner.after.0 } else { owner.before.0 };
        let mut i = 0;
        while i < hooks.len() {
            let name = hooks[i].name;
            let (node, path) = hook(owner, hooks[i], self.child_path(path, hooks[i]));
            if !self.has_hook(node, path, after) {
                let count = self.size.before + self.size.after;
                if count == MAX_HOOKS {
                    fail(&["a statement can't have more than 64 hooks"]);
                }
                self.hooks[count] = Some(Hook {
                    name,
                    node,
                    path,
                    after,
                });
                if after {
                    self.size.after += 1;
                } else {
                    self.size.before += 1;
                }
            }
            i += 1;
        }
    }

    const fn has_hook(&self, node: &'static Node, path: Path, after: bool) -> bool {
        let path = instance_path(node, path);
        let mut i = 0;
        while i < self.size.before + self.size.after {
            if let Some(hook) = self.hooks[i]
                && hook.after == after
                && same_node(hook.node, node)
                && instance_path(hook.node, hook.path).same(&path)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    const fn render_hooks(&mut self, after: bool) {
        let mut i = 0;
        while i < self.size.before + self.size.after {
            if let Some(hook) = self.hooks[i]
                && hook.after == after
            {
                self.single(hook.name, hook.node, hook.path);
            }
            i += 1;
        }
    }

    const fn single(&mut self, name: Name, node: &'static Node, path: Path) {
        self.ctes = [None; MAX_CTES];
        self.cte_count = 0;
        self.recursive = false;
        self.numbered = [None; MAX_NUMBERED];
        self.numbered_count = 0;
        self.first_bind = self.size.binds;
        self.collect(node, path);
        self.with_clause();
        self.body(node, path);
        if !self.offsets.is_empty() {
            self.offsets[self.rendered] = Offsets {
                name,
                sql: self.size.sql,
                binds: self.size.binds,
            };
        }
        self.rendered += 1;
    }

    const fn collect(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Expr(expr) => {
                    if !matches!(expr.kind(), Kind::Query) {
                        fail(&[
                            "`",
                            expr.name().as_str(),
                            "` isn't a query, so it can't be used as an expression",
                        ]);
                    }
                    self.collect(expr.as_ref(), self.child_path(path, expr.as_ref()));
                }
                Part::From(from) => {
                    let child = from.node();
                    let child_path = self.child_path(path, child);
                    match placement::<D>(from) {
                        Inject::Cte { recursive } => {
                            self.collect(child, child_path);
                            self.add_cte(child, child_path, recursive);
                        }
                        Inject::Ident | Inject::Subquery => self.collect(child, child_path),
                    }
                }
                Part::Lit(_) | Part::Ident(_) | Part::Param(_) => {}
            }
            i += 1;
        }
    }

    const fn add_cte(&mut self, node: &'static Node, path: Path, recursive: bool) {
        self.recursive |= recursive;
        let path = instance_path(node, path);
        if self.cte_suffix(node, path).is_some() {
            return;
        }
        let name = node.name.as_str();
        let mut suffix = 0;
        while self.cte_name_taken(name, suffix) {
            suffix = if suffix == 0 { 2 } else { suffix + 1 };
        }
        if suffix != 0 && recursive {
            if self.has_cte(node) {
                fail(&[
                    "`",
                    name,
                    "` is recursive and has parameters, so it can only be reached through one path",
                ]);
            }
            fail(&[
                "two different CTEs are named `",
                name,
                "`, and the second one is recursive, so it can't be renamed",
            ]);
        }
        if self.cte_count == MAX_CTES {
            fail(&["a statement can't have more than 64 CTEs"]);
        }
        self.ctes[self.cte_count] = Some(Cte { node, path, suffix });
        self.cte_count += 1;
    }

    const fn has_cte(&self, node: &'static Node) -> bool {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && same_node(cte.node, node)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    const fn cte_suffix(&self, node: &'static Node, path: Path) -> Option<u16> {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && same_node(cte.node, node)
                && cte.path.same(&path)
            {
                return Some(cte.suffix);
            }
            i += 1;
        }
        None
    }

    const fn cte_name_taken(&self, name: &str, suffix: u16) -> bool {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && suffixed_eq(cte.node.name.as_str(), cte.suffix, name, suffix)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    const fn with_clause(&mut self) {
        if self.cte_count == 0 {
            return;
        }
        self.push(if self.recursive {
            "WITH RECURSIVE "
        } else {
            "WITH "
        });
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i] {
                if i > 0 {
                    self.push(", ");
                }
                self.quoted(cte.node.name.as_str(), cte.suffix);
                self.push(" AS (");
                self.body(cte.node, cte.path);
                self.push(")");
            }
            i += 1;
        }
        self.push(" ");
    }

    const fn body(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Lit(lit) => self.push(lit.as_str()),
                Part::Ident(ident) => self.quoted(ident.0, 0),
                Part::Param(param) => self.param(node, path, param),
                Part::Expr(expr) => {
                    self.push("(");
                    self.body(expr.as_ref(), self.child_path(path, expr.as_ref()));
                    self.push(")");
                }
                Part::From(from) => self.from(path, from),
            }
            i += 1;
        }
    }

    const fn from(&mut self, path: Path, from: From) {
        let child = from.node();
        let child_path = self.child_path(path, child);
        match placement::<D>(from) {
            Inject::Ident => self.body(child, child_path),
            Inject::Cte { .. } => match self.cte_suffix(child, instance_path(child, child_path)) {
                Some(suffix) => self.quoted(child.name.as_str(), suffix),
                None => panic!("CTE wasn't collected before rendering"),
            },
            Inject::Subquery => {
                self.push("(");
                self.body(child, child_path);
                self.push(")");
                if let AliasRule::NodeName = from.rule() {
                    self.push(" AS ");
                    self.quoted(child.name.as_str(), 0);
                }
            }
        }
    }

    const fn param(&mut self, node: &'static Node, path: Path, param: Param) {
        self.byte(D::PARAMS.prefix());
        if !D::PARAMS.numbered() {
            self.bind(node, path, param);
            return;
        }
        let slot = Slot::from_param(param);
        let number = match self.number_of(path, slot) {
            Some(number) => number,
            None => {
                if self.numbered_count == MAX_NUMBERED {
                    fail(&["a statement can't have more than 1024 distinct parameters"]);
                }
                self.numbered[self.numbered_count] = Some(Numbered { path, slot });
                self.numbered_count += 1;
                self.bind(node, path, param);
                self.numbered_count as u16
            }
        };
        self.number(number);
    }

    const fn number_of(&self, path: Path, slot: Slot) -> Option<u16> {
        let mut i = 0;
        while i < self.numbered_count {
            if let Some(numbered) = self.numbered[i]
                && numbered.slot.inner() == slot.inner()
                && numbered.path.same(&path)
            {
                return Some(i as u16 + 1);
            }
            i += 1;
        }
        None
    }

    const fn bind(&mut self, node: &'static Node, path: Path, param: Param) {
        if self.size.binds - self.first_bind == u16::MAX as usize {
            fail(&["a statement can't have more than 65535 parameters"]);
        }
        if !self.binds.is_empty() {
            self.binds[self.size.binds] = Bind::from_param(node, path, param);
        }
        self.size.binds += 1;
    }

    const fn child_path(&self, path: Path, child: &'static Node) -> Path {
        let root = match self.root {
            Some(root) => root,
            None => panic!("rendering started without a root"),
        };
        let mut depth = path.steps().len();
        loop {
            if let Some(path) = item_path(frame(root, path, depth), path.prefix(depth), child) {
                return path;
            }
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        if has_params(child) {
            fail(&[
                "`",
                child.name.as_str(),
                "` has parameters, but no query it's used in lists it among its items",
            ]);
        }
        path
    }

    const fn number(&mut self, n: u16) {
        let mut digits = [0; 5];
        let mut count = 0;
        let mut rest = n;
        loop {
            digits[count] = b'0' + (rest % 10) as u8;
            count += 1;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        while count > 0 {
            count -= 1;
            self.byte(digits[count]);
        }
    }

    const fn quoted(&mut self, ident: &str, suffix: u16) {
        let [open, close] = D::QUOTE.as_array();
        self.byte(open);
        let bytes = ident.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == close {
                self.byte(close);
            }
            self.byte(bytes[i]);
            i += 1;
        }
        if suffix != 0 {
            self.byte(b'_');
            self.number(suffix);
        }
        self.byte(close);
    }

    const fn push(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            self.byte(bytes[i]);
            i += 1;
        }
    }

    const fn byte(&mut self, byte: u8) {
        if !self.sql.is_empty() {
            self.sql[self.size.sql] = byte;
        }
        self.size.sql += 1;
    }
}

const fn placement<D: Dialect>(from: From) -> Inject {
    let inject = from.inject();
    let name = from.node().name.as_str();
    match (from.node().kind, inject) {
        (Kind::Table, Inject::Ident) | (Kind::Query, Inject::Cte { .. } | Inject::Subquery) => {}
        (Kind::Table, _) => fail(&["table `", name, "` can only be referenced by name"]),
        (_, Inject::Ident) => fail(&[
            "`",
            name,
            "` isn't a table, so it can't be referenced by name",
        ]),
        (Kind::Dml, Inject::Cte { .. }) if D::DML_IN_CTE => {}
        (Kind::Dml, Inject::Cte { .. }) => fail(&[
            "`",
            name,
            "` modifies data, which ",
            D::NAME.as_str(),
            " doesn't allow in a CTE",
        ]),
        (Kind::Dml, _) => fail(&[
            "`",
            name,
            "` modifies data, so it can only be embedded as a CTE",
        ]),
        (Kind::Ddl, _) => fail(&["`", name, "` is DDL, so it can't be embedded"]),
        (Kind::Scope, _) => fail(&["`", name, "` only holds values, so it can't be embedded"]),
    }
    inject
}

const fn frame(root: &'static Node, path: Path, depth: usize) -> &'static Node {
    let steps = path.steps();
    let mut node = root;
    let mut i = 0;
    while i < depth {
        node = node.items.0[steps[i] as usize];
        i += 1;
    }
    node
}

const fn item_path(frame: &'static Node, prefix: Path, child: &'static Node) -> Option<Path> {
    let items = frame.items.0;
    let mut i = 0;
    while i < items.len() {
        let item = items[i];
        let path = if same_node(item, child) {
            prefix.child(i as u16)
        } else if let Kind::Scope = item.kind
            && same_node(item.items.0[0], child)
        {
            match prefix.child(i as u16) {
                Some(path) => path.child(0),
                None => None,
            }
        } else {
            i += 1;
            continue;
        };
        return match path {
            Some(path) => Some(path),
            None => fail(&["items can't be nested more than 16 deep"]),
        };
    }
    None
}

const fn instance_path(node: &'static Node, path: Path) -> Path {
    if has_params(node) { path } else { Path::ROOT }
}

const fn target(node: &'static Node, path: Path) -> (&'static Node, Path) {
    let (node, path) = match (node.kind, path.child(0)) {
        (Kind::Scope, Some(path)) => (node.items.0[0], path),
        _ => (node, path),
    };
    if let Kind::Table = node.kind {
        fail(&["`", node.name.as_str(), "` is a table, not a statement"]);
    }
    (node, path)
}

const fn hook(owner: &'static Node, hook: &'static Node, path: Path) -> (&'static Node, Path) {
    if let Kind::Table = hook.kind {
        fail(&[
            "`",
            hook.name.as_str(),
            "` is a table, so it can't be a hook of `",
            owner.name.as_str(),
            "`",
        ]);
    }
    let (node, path) = target(hook, path);
    if has_hooks(node) {
        fail(&[
            "`",
            hook.name.as_str(),
            "` is a hook of `",
            owner.name.as_str(),
            "`, so it can't use items with hooks of its own",
        ]);
    }
    (node, path)
}

const fn has_hooks(node: &'static Node) -> bool {
    if !node.before.0.is_empty() || !node.after.0.is_empty() {
        return true;
    }
    let parts = node.parts.0;
    let mut i = 0;
    while i < parts.len() {
        let nested = match parts[i] {
            Part::Expr(expr) => has_hooks(expr.as_ref()),
            Part::From(from) => has_hooks(from.node()),
            Part::Lit(_) | Part::Ident(_) | Part::Param(_) => false,
        };
        if nested {
            return true;
        }
        i += 1;
    }
    false
}

const fn has_params(node: &'static Node) -> bool {
    let parts = node.parts.0;
    let mut i = 0;
    while i < parts.len() {
        let found = match parts[i] {
            Part::Param(_) => true,
            Part::Expr(expr) => has_params(expr.as_ref()),
            Part::From(from) => has_params(from.node()),
            Part::Lit(_) | Part::Ident(_) => false,
        };
        if found {
            return true;
        }
        i += 1;
    }
    false
}

const fn same_node(a: &'static Node, b: &'static Node) -> bool {
    a.fingerprint.0 == b.fingerprint.0 && suffixed_eq(a.name.as_str(), 0, b.name.as_str(), 0)
}

const fn suffixed_eq(a: &str, a_suffix: u16, b: &str, b_suffix: u16) -> bool {
    let len = suffixed_len(a, a_suffix);
    if len != suffixed_len(b, b_suffix) {
        return false;
    }
    let mut i = 0;
    while i < len {
        if suffixed_byte(a, a_suffix, i) != suffixed_byte(b, b_suffix, i) {
            return false;
        }
        i += 1;
    }
    true
}

const fn suffixed_len(name: &str, suffix: u16) -> usize {
    if suffix == 0 {
        name.len()
    } else {
        name.len() + 1 + digit_count(suffix)
    }
}

const fn suffixed_byte(name: &str, suffix: u16, i: usize) -> u8 {
    let bytes = name.as_bytes();
    if i < bytes.len() {
        return bytes[i];
    }
    if i == bytes.len() {
        return b'_';
    }
    let mut rest = suffix;
    let mut skip = digit_count(suffix) - (i - bytes.len());
    while skip > 0 {
        rest /= 10;
        skip -= 1;
    }
    b'0' + (rest % 10) as u8
}

const fn digit_count(n: u16) -> usize {
    let mut count = 1;
    let mut rest = n / 10;
    while rest > 0 {
        count += 1;
        rest /= 10;
    }
    count
}
