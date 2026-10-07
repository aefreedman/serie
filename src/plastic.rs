//! Read-only Plastic seam. No Git model dependencies and no mutation commands.
//!
//! `Backend::load` returns a bounded newest-first repository snapshot; parents
//! outside the window remain qualified IDs in `missing_endpoints`. Integration
//! types are deliberately NOT folded into primary ancestry. `detail` is lazy.
//! All query/XML failures fail closed, while valid bounded omissions are disclosed.
use std::{
    collections::HashSet,
    fmt,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repository {
    pub name: String,
    pub server: String,
}
impl Repository {
    pub fn selector(&self) -> String {
        format!("rep:{}@{}", self.name, self.server)
    }
    fn validate(&self) -> Result<()> {
        for s in [&self.name, &self.server] {
            if s.is_empty()
                || s.len() > 4096
                || s.chars()
                    .any(|c| c.is_control() || c == '\'' || c == '"' || c == '\\')
            {
                return Err(Error("unsupported repository identity for query".into()));
            }
        }
        Ok(())
    }
}
/// A changeset NUMBER, not an object ID, GUID, revision ID, or Git hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChangesetId(String);
impl ChangesetId {
    pub fn new(value: &str) -> Result<Self> {
        if value.is_empty()
            || value.len() > 20
            || !value.bytes().all(|b| b.is_ascii_digit())
            || value.parse::<u64>().is_err()
        {
            return Err(Error(format!("invalid changeset number: {value:?}")));
        }
        // Reject ambiguous spellings rather than silently changing identity.
        if value.len() > 1 && value.starts_with('0') {
            return Err(Error("noncanonical changeset number".into()));
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    fn number(&self) -> u64 {
        self.0.parse().expect("validated numeric ID")
    }
}
impl fmt::Display for ChangesetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedChangeset {
    pub repository: Repository,
    pub id: ChangesetId,
}
impl QualifiedChangeset {
    pub fn selector(&self) -> String {
        format!("cs:{}@{}", self.id, self.repository.selector())
    }
}
#[derive(Debug, Clone)]
pub struct Changeset {
    pub key: QualifiedChangeset,
    pub object_id: String,
    pub guid: String,
    pub branch: String,
    pub owner: String,
    /// Original ISO timestamp, including producer timezone.
    pub date: String,
    pub comment: String,
    pub primary_parent: Option<QualifiedChangeset>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationKind {
    Merge,
    CherryPick,
    SubtractiveCherryPick,
    Interval,
    IntervalCherryPick,
    SubtractiveIntervalCherryPick,
    Unknown(String),
}
impl IntegrationKind {
    pub fn from_raw(s: &str) -> Self {
        match s {
            "merge" => Self::Merge,
            "cherrypick" => Self::CherryPick,
            "cherrypicksubstractive" => Self::SubtractiveCherryPick,
            "interval" => Self::Interval,
            "intervalcherrypick" => Self::IntervalCherryPick,
            "intervalcherrypicksubstractive" => Self::SubtractiveIntervalCherryPick,
            _ => Self::Unknown(s.into()),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Integration {
    pub object_id: String,
    pub source: QualifiedChangeset,
    pub destination: QualifiedChangeset,
    pub base: Option<QualifiedChangeset>,
    pub kind: IntegrationKind,
    pub raw_type: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub owner: String,
    pub date: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    Branch,
    Label,
}
#[derive(Debug, Clone)]
pub struct Reference {
    pub repository: Repository,
    pub kind: RefKind,
    pub name: String,
    /// Producer's CHANGESET field. Never derived from branch name/hierarchy.
    pub target: Option<QualifiedChangeset>,
    pub owner: String,
    pub date: String,
    pub comment: String,
}
impl Reference {
    pub fn selector(&self) -> String {
        format!(
            "{}:{}@{}",
            if self.kind == RefKind::Branch {
                "br"
            } else {
                "lb"
            },
            self.name,
            self.repository.selector()
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStatus {
    /// Exact identity from status; server aliases are NOT silently equated.
    pub loaded: QualifiedChangeset,
    pub config_type: String,
    pub config_name: String,
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub changesets: Vec<Changeset>,
    pub integrations: Vec<Integration>,
    pub references: Vec<Reference>,
    pub workspace: WorkspaceStatus,
    /// Loaded marker qualified using the identity returned by this scoped query.
    /// Original status identity remains available in `workspace.loaded`.
    pub loaded_changeset: QualifiedChangeset,
    /// Out-of-window primary/integration endpoints, not invented placeholder nodes.
    pub missing_endpoints: Vec<QualifiedChangeset>,
    /// Must be shown by consumers; nonempty means bounded/unsupported evidence.
    pub warnings: Vec<String>,
    pub history_truncated: bool,
    pub integrations_truncated: bool,
    pub references_truncated: bool,
}
#[derive(Debug, Clone)]
pub struct ChangedPath {
    pub source_path: String,
    pub destination_path: String,
    /// Raw Added/Deleted/Moved/Changed (future values retained verbatim).
    pub change_type: String,
    pub revision_id: String,
    pub parent_revision_id: String,
}
#[derive(Debug, Clone)]
pub struct Detail {
    pub key: QualifiedChangeset,
    pub paths: Vec<ChangedPath>,
}
#[derive(Debug)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Limits {
    pub changesets: usize,
    pub integrations: usize,
    pub references_per_kind: usize,
    pub timeout: Duration,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            changesets: 500,
            integrations: 2000,
            references_per_kind: 2000,
            timeout: Duration::from_secs(30),
            stdout_bytes: 8 * 1024 * 1024,
            stderr_bytes: 64 * 1024,
        }
    }
}
impl Limits {
    fn validate(&self) -> Result<()> {
        if [self.changesets, self.integrations, self.references_per_kind]
            .iter()
            .any(|n| *n == 0 || *n > 10_000)
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(120)
            || self.stdout_bytes == 0
            || self.stdout_bytes > 32 * 1024 * 1024
            || self.stderr_bytes == 0
            || self.stderr_bytes > 1024 * 1024
        {
            return Err(Error("limits outside supported bounds".into()));
        }
        Ok(())
    }
}
/// Uses PATH's cm by default; callers may select a trusted absolute cm executable.
/// The workspace must exist. No shell, stdin, user-supplied queries or output files.
pub struct Backend {
    executable: PathBuf,
    workspace: PathBuf,
    limits: Limits,
}
impl Backend {
    pub fn new(workspace: impl AsRef<Path>, limits: Limits) -> Result<Self> {
        Self::with_executable(workspace, "cm", limits)
    }
    pub fn with_executable(
        workspace: impl AsRef<Path>,
        executable: impl AsRef<Path>,
        limits: Limits,
    ) -> Result<Self> {
        limits.validate()?;
        let workspace = workspace.as_ref().canonicalize()?;
        if !workspace.is_dir() {
            return Err(Error("workspace is not a directory".into()));
        }
        Ok(Self {
            executable: executable.as_ref().into(),
            workspace,
            limits,
        })
    }
    fn run(&self, args: &[String]) -> Result<String> {
        checked_run(&self.executable, &self.workspace, args, &self.limits)
    }
    pub fn status(&self) -> Result<WorkspaceStatus> {
        parse_status(&self.run(&strings(&[
            "status",
            "--header",
            "--xml",
            "--encoding=utf-8",
        ]))?)
    }
    fn find(&self, object: &str, clauses: &str, scope: &Repository) -> Result<String> {
        scope.validate()?;
        // Installed cm requires repository clause AFTER order/limit, despite help synopsis.
        self.run(&[
            "find".into(),
            object.into(),
            format!("{clauses} on repository '{}'", scope.selector()),
            "--xml".into(),
            "--encoding=utf-8".into(),
        ])
    }
    pub fn load(&self) -> Result<Snapshot> {
        let workspace = self.status()?;
        let query_repository = workspace.loaded.repository.clone();
        let scope = &query_repository;
        let mut changesets = parse_changesets(&self.find(
            "changeset",
            &format!(
                "where ignorehidden = 'false' order by changesetid desc limit {}",
                self.limits.changesets + 1
            ),
            scope,
        )?)?;
        let history_truncated = truncate(&mut changesets, self.limits.changesets);
        let mut integrations = Vec::new();
        if let (Some(min), Some(max)) = (
            changesets.iter().map(|c| c.key.id.number()).min(),
            changesets.iter().map(|c| c.key.id.number()).max(),
        ) {
            // IDs bound the query only; NEVER imply edges.
            integrations = parse_integrations(
                &self.find(
                    "merge",
                    &format!(
                        "where dstchangeset >= {min} and dstchangeset <= {max} limit {}",
                        self.limits.integrations + 1
                    ),
                    scope,
                )?,
                scope,
            )?;
        }
        let integrations_truncated = truncate(&mut integrations, self.limits.integrations);
        // Merge XML carries no repository fields. It is qualified by the exact query scope.
        // Changeset XML may return a display-server alias: keep both identities as observed.
        let mut references = Vec::new();
        let mut references_truncated = false;
        for (object, kind) in [("branch", RefKind::Branch), ("label", RefKind::Label)] {
            let mut refs = parse_references(
                &self.find(
                    object,
                    &format!("limit {}", self.limits.references_per_kind + 1),
                    scope,
                )?,
                kind,
            )?;
            references_truncated |= truncate(&mut refs, self.limits.references_per_kind);
            references.extend(refs);
        }
        if self.status()? != workspace {
            return Err(Error(
                "workspace selection changed during history read; retry".into(),
            ));
        }
        let mut warnings = Vec::new();
        if history_truncated {
            warnings
                .push("History window truncated; parents outside window are not loaded.".into());
        }
        if integrations_truncated {
            warnings
                .push("Integration query truncated; integration topology is incomplete.".into());
        }
        if references_truncated {
            warnings.push("Branch/label query truncated; annotations are incomplete.".into());
        }
        // Explicit repository-scoped query proves the mapping of returned aliases for THIS read.
        // Requalify integrations and loaded marker to the single returned identity only when uniform.
        let identities: HashSet<_> = changesets
            .iter()
            .map(|c| c.key.repository.clone())
            .collect();
        let mut loaded_changeset = workspace.loaded.clone();
        if identities.len() == 1 {
            let observed = identities.into_iter().next().unwrap();
            if observed != *scope {
                warnings.push(format!("Scoped query returned server alias {}; workspace status used {}. Mapping is local to this scoped read.", observed.selector(), scope.selector()));
                loaded_changeset.repository = observed.clone();
                for link in &mut integrations {
                    link.source.repository = observed.clone();
                    link.destination.repository = observed.clone();
                    if let Some(base) = &mut link.base {
                        base.repository = observed.clone();
                    }
                }
            }
        } else if identities.len() > 1 {
            return Err(Error(
                "single-repository query returned mixed identities".into(),
            ));
        }
        if integrations
            .iter()
            .any(|i| i.kind != IntegrationKind::Merge)
        {
            warnings.push("Nonordinary integrations retained with raw types; do not render as ordinary ancestry.".into());
        }
        let keys: HashSet<_> = changesets.iter().map(|c| c.key.clone()).collect();
        let mut missing_endpoints = Vec::new();
        let mut seen = HashSet::new();
        for key in changesets
            .iter()
            .filter_map(|c| c.primary_parent.as_ref())
            .chain(
                integrations
                    .iter()
                    .flat_map(|i| [&i.source, &i.destination].into_iter().chain(i.base.iter())),
            )
        {
            if !keys.contains(key) && seen.insert(key.clone()) {
                missing_endpoints.push(key.clone());
            }
        }
        if !missing_endpoints.is_empty() {
            warnings.push(
                "Some topology endpoints are outside the loaded window (see missing_endpoints)."
                    .into(),
            );
        }
        Ok(Snapshot {
            changesets,
            integrations,
            references,
            workspace,
            loaded_changeset,
            missing_endpoints,
            warnings,
            history_truncated,
            integrations_truncated,
            references_truncated,
        })
    }
    pub fn detail(&self, key: &QualifiedChangeset) -> Result<Detail> {
        key.repository.validate()?;
        parse_detail(
            &self.run(&[
                "log".into(),
                key.selector(),
                "--xml".into(),
                "--encoding=utf-8".into(),
                "--repositorypaths".into(),
            ])?,
            key,
        )
    }
}
fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| (*s).into()).collect()
}
fn truncate<T>(xs: &mut Vec<T>, limit: usize) -> bool {
    let truncated = xs.len() > limit;
    xs.truncate(limit);
    truncated
}

/// Concurrent bounded pipe reads prevent stdout/stderr deadlocks. Overflow is an
/// error, never a successful partial document. Deadline includes pipe completion.
fn checked_run(executable: &Path, cwd: &Path, args: &[String], limits: &Limits) -> Result<String> {
    let mut child = Command::new(executable)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (tx, rx) = mpsc::channel();
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    for (index, mut pipe, cap) in [
        (
            0,
            Box::new(out) as Box<dyn Read + Send>,
            limits.stdout_bytes,
        ),
        (
            1,
            Box::new(err) as Box<dyn Read + Send>,
            limits.stderr_bytes,
        ),
    ] {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe
                .by_ref()
                .take(cap as u64 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send((index, result));
        });
    }
    drop(tx);
    let started = Instant::now();
    let mut outputs: [Option<Vec<u8>>; 2] = [None, None];
    let mut status = None;
    let result = (|| loop {
        while let Ok((index, result)) = rx.try_recv() {
            let bytes = result?;
            let cap = if index == 0 {
                limits.stdout_bytes
            } else {
                limits.stderr_bytes
            };
            if bytes.len() > cap {
                return Err(Error(format!(
                    "cm {} output exceeded {} bytes",
                    if index == 0 { "stdout" } else { "stderr" },
                    cap
                )));
            }
            outputs[index] = Some(bytes);
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if let Some(status) = status {
            if let [Some(stdout), Some(stderr)] = &outputs {
                if !status.success() {
                    return Err(Error(format!(
                        "cm exited {status}: {}",
                        String::from_utf8_lossy(stderr)
                    )));
                }
                return String::from_utf8(stdout.clone())
                    .map_err(|e| Error(format!("cm returned non-UTF-8 output: {e}")));
            }
        }
        if started.elapsed() >= limits.timeout {
            return Err(Error("cm read deadline exceeded".into()));
        }
        thread::sleep(Duration::from_millis(5));
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn document<'a>(xml: &'a str, root: &str) -> Result<roxmltree::Document<'a>> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| Error(format!("invalid cm XML: {e}")))?;
    if doc.root_element().tag_name().name() != root {
        return Err(Error(format!("expected XML root {root}")));
    }
    Ok(doc)
}
fn field<'a>(n: roxmltree::Node<'a, '_>, name: &str) -> Result<&'a str> {
    let mut children = n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == name);
    let child = children
        .next()
        .ok_or_else(|| Error(format!("missing XML field {name}")))?;
    if children.next().is_some() || child.children().any(|c| c.is_element()) {
        return Err(Error(format!("invalid scalar XML field {name}")));
    }
    Ok(child.text().unwrap_or(""))
}
fn child<'a, 'b>(n: roxmltree::Node<'a, 'b>, name: &str) -> Result<roxmltree::Node<'a, 'b>> {
    let mut nodes = n.children().filter(|n| n.has_tag_name(name));
    let node = nodes
        .next()
        .ok_or_else(|| Error(format!("missing XML node {name}")))?;
    if nodes.next().is_some() {
        return Err(Error(format!("duplicate XML node {name}")));
    }
    Ok(node)
}
fn repository(n: roxmltree::Node<'_, '_>) -> Result<Repository> {
    let r = Repository {
        name: field(n, "REPNAME")?.into(),
        server: field(n, "REPSERVER")?.into(),
    };
    r.validate()?;
    Ok(r)
}
fn key(repo: &Repository, id: &str) -> Result<QualifiedChangeset> {
    Ok(QualifiedChangeset {
        repository: repo.clone(),
        id: ChangesetId::new(id)?,
    })
}
fn optional_key(repo: &Repository, id: &str) -> Result<Option<QualifiedChangeset>> {
    if id.is_empty() || id == "-1" {
        Ok(None)
    } else {
        Ok(Some(key(repo, id)?))
    }
}
fn rows<'a, 'b>(
    doc: &'b roxmltree::Document<'a>,
    name: &str,
) -> Result<Vec<roxmltree::Node<'a, 'b>>> {
    let nodes: Vec<_> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element())
        .collect();
    if nodes.iter().any(|n| n.tag_name().name() != name) {
        return Err(Error(format!("unexpected record in {name} query")));
    }
    Ok(nodes)
}
pub fn parse_changesets(xml: &str) -> Result<Vec<Changeset>> {
    let doc = document(xml, "PLASTICQUERY")?;
    let mut seen = HashSet::new();
    rows(&doc, "CHANGESET")?
        .into_iter()
        .map(|n| {
            let repo = repository(n)?;
            let key = key(&repo, field(n, "CHANGESETID")?)?;
            if !seen.insert(key.clone()) {
                return Err(Error("duplicate changeset identity".into()));
            }
            let parent_number = field(n, "PARENT")?;
            if parent_number.is_empty() {
                return Err(Error(
                    "changeset parent evidence is empty (root must be -1)".into(),
                ));
            }
            let parent = optional_key(&repo, parent_number)?;
            if parent.as_ref() == Some(&key) {
                return Err(Error("self-parent changeset".into()));
            }
            Ok(Changeset {
                key,
                primary_parent: parent,
                object_id: field(n, "ID")?.into(),
                guid: field(n, "GUID")?.into(),
                branch: field(n, "BRANCH")?.into(),
                owner: field(n, "OWNER")?.into(),
                date: field(n, "DATE")?.into(),
                comment: field(n, "COMMENT")?.into(),
            })
        })
        .collect()
}
pub fn parse_integrations(xml: &str, scope: &Repository) -> Result<Vec<Integration>> {
    scope.validate()?;
    let doc = document(xml, "PLASTICQUERY")?;
    rows(&doc, "MERGE")?
        .into_iter()
        .map(|n| {
            let raw_type = field(n, "TYPE")?.to_owned();
            Ok(Integration {
                object_id: field(n, "ID")?.into(),
                source: key(scope, field(n, "SRCCHANGESET")?)?,
                destination: key(scope, field(n, "DSTCHANGESET")?)?,
                base: optional_key(scope, field(n, "BASECHANGESET")?)?,
                kind: IntegrationKind::from_raw(&raw_type),
                raw_type,
                source_branch: field(n, "SRCBRANCH")?.into(),
                destination_branch: field(n, "DSTBRANCH")?.into(),
                owner: field(n, "OWNER")?.into(),
                date: field(n, "DATE")?.into(),
            })
        })
        .collect()
}
/// cm find label emits MARKER records, with CHANGESET identifying the target.
/// Missing/unrecognized fields fail explicitly rather than inventing a label target.
pub fn parse_references(xml: &str, kind: RefKind) -> Result<Vec<Reference>> {
    let doc = document(xml, "PLASTICQUERY")?;
    rows(
        &doc,
        if kind == RefKind::Branch {
            "BRANCH"
        } else {
            "MARKER"
        },
    )?
    .into_iter()
    .map(|n| {
        let repo = repository(n)?;
        Ok(Reference {
            target: optional_key(&repo, field(n, "CHANGESET")?)?,
            repository: repo,
            kind: kind.clone(),
            name: field(n, "NAME")?.into(),
            owner: field(n, "OWNER")?.into(),
            date: field(n, "DATE")?.into(),
            comment: field(n, "COMMENT")?.into(),
        })
    })
    .collect()
}
pub fn parse_status(xml: &str) -> Result<WorkspaceStatus> {
    let doc = document(xml, "StatusOutput")?;
    let root = doc.root_element();
    let status = child(child(root, "WorkspaceStatus")?, "Status")?;
    let rep = child(status, "RepSpec")?;
    let repo = Repository {
        name: field(rep, "Name")?.into(),
        server: field(rep, "Server")?.into(),
    };
    repo.validate()?;
    Ok(WorkspaceStatus {
        loaded: key(&repo, field(status, "Changeset")?)?,
        config_type: field(root, "WkConfigType")?.into(),
        config_name: field(root, "WkConfigName")?.into(),
    })
}
pub fn parse_detail(xml: &str, requested: &QualifiedChangeset) -> Result<Detail> {
    let doc = document(xml, "LogList")?;
    let changesets = rows(&doc, "Changeset")?;
    if changesets.len() != 1 || field(changesets[0], "ChangesetId")? != requested.id.as_str() {
        return Err(Error(
            "log result does not match requested changeset".into(),
        ));
    }
    let changes = changesets[0]
        .children()
        .find(|n| n.has_tag_name("Changes"))
        .ok_or_else(|| Error("log omitted Changes".into()))?;
    let paths = changes
        .children()
        .filter(|n| n.is_element())
        .map(|n| {
            if !n.has_tag_name("Item") {
                return Err(Error("unexpected log change record".into()));
            }
            Ok(ChangedPath {
                source_path: field(n, "SrcCmPath")?.into(),
                destination_path: field(n, "DstCmPath")?.into(),
                change_type: field(n, "Type")?.into(),
                revision_id: field(n, "RevId")?.into(),
                parent_revision_id: field(n, "ParentRevId")?.into(),
            })
        })
        .collect::<Result<_>>()?;
    Ok(Detail {
        key: requested.clone(),
        paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const CSETS: &str = include_str!("../tests/fixtures/plastic/changesets.xml");
    const MERGES: &str = include_str!("../tests/fixtures/plastic/merges.xml");
    const BRANCHES: &str = include_str!("../tests/fixtures/plastic/branches.xml");
    const STATUS: &str = include_str!("../tests/fixtures/plastic/status.xml");
    const DETAIL: &str = include_str!("../tests/fixtures/plastic/detail-16.xml");
    fn repo() -> Repository {
        Repository {
            name: "Example Repositories/test-repo".into(),
            server: "example-org@unity".into(),
        }
    }

    #[test]
    fn actual_metadata_and_primary_topology() {
        let cs = parse_changesets(CSETS).unwrap();
        assert_eq!(cs.len(), 40);
        let get = |id: &str| cs.iter().find(|c| c.key.id.as_str() == id).unwrap();
        assert_eq!(get("41").object_id, "622");
        assert_eq!(get("41").primary_parent.as_ref().unwrap().id.as_str(), "16");
        assert_eq!(get("17").primary_parent.as_ref().unwrap().id.as_str(), "13");
        assert!(get("0").primary_parent.is_none());
        assert_eq!(get("16").owner, "developer@example.com");
        assert_eq!(get("16").date, "2026-09-14T12:28:15-04:00");
        assert!(get("19").comment.contains("résumé 日本語 😀"));
        assert_eq!(
            get("16").key.selector(),
            "cs:16@rep:Example Repositories/test-repo@example-org@unity"
        );
    }
    #[test]
    fn actual_integrations_are_not_primary_parents() {
        let links = parse_integrations(MERGES, &repo()).unwrap();
        assert_eq!(links.len(), 9);
        let link = &links[0];
        assert_eq!(link.source.id.as_str(), "14");
        assert_eq!(link.destination.id.as_str(), "17");
        assert_eq!(link.object_id, "439");
        assert_eq!(link.kind, IntegrationKind::Merge);
        assert!(link.base.is_none());
        assert_eq!(link.source.repository, repo());
    }
    #[test]
    fn every_nonordinary_and_unknown_type_is_retained() {
        for (raw, expected) in [
            ("cherrypick", IntegrationKind::CherryPick),
            (
                "cherrypicksubstractive",
                IntegrationKind::SubtractiveCherryPick,
            ),
            ("interval", IntegrationKind::Interval),
            ("intervalcherrypick", IntegrationKind::IntervalCherryPick),
            (
                "intervalcherrypicksubstractive",
                IntegrationKind::SubtractiveIntervalCherryPick,
            ),
            (
                "future-type",
                IntegrationKind::Unknown("future-type".into()),
            ),
        ] {
            let xml = MERGES
                .replace("<TYPE>merge</TYPE>", &format!("<TYPE>{raw}</TYPE>"))
                .replace(
                    "<BASECHANGESET></BASECHANGESET>",
                    "<BASECHANGESET>13</BASECHANGESET>",
                );
            let links = parse_integrations(&xml, &repo()).unwrap();
            assert_eq!(links[0].kind, expected);
            assert_eq!(links[0].raw_type, raw);
            assert_eq!(links[0].base.as_ref().unwrap().id.as_str(), "13");
        }
    }
    #[test]
    fn actual_refs_and_status_preserve_qualified_context() {
        let refs = parse_references(BRANCHES, RefKind::Branch).unwrap();
        assert_eq!(refs.len(), 48);
        assert_eq!(refs[0].name, "/main");
        assert_eq!(refs[0].target.as_ref().unwrap().id.as_str(), "2");
        assert_eq!(
            refs[0].selector(),
            "br:/main@rep:Example Repositories/test-repo@example-org@unity"
        );
        let status = parse_status(STATUS).unwrap();
        assert_eq!(status.loaded.id.as_str(), "16");
        assert_eq!(status.loaded.repository.server, "1234567890123@cloud");
        assert_ne!(status.loaded.repository, repo());
    }
    #[test]
    fn synthetic_label_target_and_empty_live_labels() {
        let live = parse_references(
            include_str!("../tests/fixtures/plastic/labels-marker.xml"),
            RefKind::Label,
        )
        .unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].name, "serie-plastic-test-label");
        assert_eq!(live[0].target.as_ref().unwrap().id.as_str(), "16");
        assert_eq!(live[0].repository.server, "example-org@unity");
        assert!(
            parse_references("<PLASTICQUERY><BRANCH /></PLASTICQUERY>", RefKind::Label).is_err()
        );
        assert!(parse_references(
            include_str!("../tests/fixtures/plastic/labels-empty.xml"),
            RefKind::Label
        )
        .unwrap()
        .is_empty());
        let xml = "<PLASTICQUERY><MARKER><NAME>release &amp; café</NAME><REPNAME>repo</REPNAME><REPSERVER>server:8087</REPSERVER><CHANGESET>7</CHANGESET><OWNER>owner</OWNER><DATE>date</DATE><COMMENT>comment</COMMENT></MARKER></PLASTICQUERY>";
        let refs = parse_references(xml, RefKind::Label).unwrap();
        assert_eq!(refs[0].name, "release & café");
        assert_eq!(refs[0].target.as_ref().unwrap().id.as_str(), "7");
    }
    #[test]
    fn lazy_detail_uses_changeset_number_not_objid_or_revid() {
        let requested = key(&repo(), "16").unwrap();
        let detail = parse_detail(DETAIL, &requested).unwrap();
        assert_eq!(detail.paths.len(), 1);
        assert_eq!(detail.paths[0].destination_path, "/p3-conflict.txt");
        assert_eq!(detail.paths[0].source_path, "/p3-conflict.txt");
        assert_eq!(detail.paths[0].revision_id, "538");
        assert_eq!(detail.paths[0].parent_revision_id, "530");
        assert_eq!(detail.paths[0].change_type, "Changed");
        assert_eq!(detail.key, requested);
        assert!(parse_detail(DETAIL, &key(&repo(), "437").unwrap()).is_err());
    }
    #[test]
    fn malformed_truncated_duplicate_missing_and_wrong_roots_fail_closed() {
        for xml in [
            "garbage",
            "<PLASTICQUERY>",
            "<wrong/>",
            "<PLASTICQUERY><CHANGESET/></PLASTICQUERY>",
        ] {
            assert!(parse_changesets(xml).is_err());
        }
        assert!(parse_changesets(&CSETS.replace("<PARENT>16</PARENT>", "")).is_err());
        assert!(parse_changesets(&CSETS.replace(
            "<CHANGESETID>40</CHANGESETID>",
            "<CHANGESETID>41</CHANGESETID>"
        ))
        .is_err());
        assert!(
            parse_changesets(&CSETS.replace("<PARENT>16</PARENT>", "<PARENT>bad</PARENT>"))
                .is_err()
        );
        assert!(parse_changesets(&CSETS.replace(
            "<PARENT>16</PARENT>",
            "<PARENT>16</PARENT><PARENT>16</PARENT>"
        ))
        .is_err());
        assert!(parse_status(&STATUS.replace(
            "<Changeset>16</Changeset>",
            "<Changeset>16</Changeset><Changeset>17</Changeset>"
        ))
        .is_err());
        assert!(parse_integrations(
            &MERGES.replace(
                "<SRCCHANGESET>14</SRCCHANGESET>",
                "<SRCCHANGESET>revid:433</SRCCHANGESET>"
            ),
            &repo()
        )
        .is_err());
    }
    #[test]
    fn empty_history_and_bounded_missing_parents_are_not_invented() {
        assert!(parse_changesets("<PLASTICQUERY/>").unwrap().is_empty());
        assert!(parse_integrations("<PLASTICQUERY/>", &repo())
            .unwrap()
            .is_empty());
        let mut cs = parse_changesets(CSETS).unwrap();
        assert!(truncate(&mut cs, 2));
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].primary_parent.as_ref().unwrap().id.as_str(), "16");
        assert!(!cs.iter().any(|c| c.key.id.as_str() == "16"));
        assert!(!truncate(&mut cs, 2));
    }
    #[test]
    fn safe_ids_context_and_limits() {
        for id in [
            "",
            "-1",
            "cs:7",
            "7;evil",
            "01",
            "18446744073709551616",
            "日本",
        ] {
            assert!(ChangesetId::new(id).is_err());
        }
        assert_eq!(ChangesetId::new("0").unwrap().to_string(), "0");
        assert_eq!(ChangesetId::new("7").unwrap().to_string(), "7");
        let mut unsafe_repo = repo();
        unsafe_repo.name = "repo' or 1=1".into();
        assert!(unsafe_repo.validate().is_err());
        let mut limits = Limits::default();
        limits.changesets = 0;
        assert!(limits.validate().is_err());
        assert!(Limits::default().validate().is_ok());
    }
    // Re-execute this test binary as a fake producer: no cm, shell, authentication,
    // or real workspace is needed on any platform.
    fn producer_args() -> Vec<String> {
        strings(&["--exact", "plastic::tests::fake_cm_process", "--nocapture"])
    }

    #[test]
    fn fake_cm_process() {
        let Ok(mode) = std::env::var("SERIE_TEST_PRODUCER") else {
            return;
        };
        use std::io::Write;
        match mode.as_str() {
            "success" => print!("fake cm output"),
            "failure" => std::process::exit(7),
            "timeout" => thread::sleep(Duration::from_secs(5)),
            "stdout" => std::io::stdout().write_all(&vec![b'x'; 8192]).unwrap(),
            "stderr" => std::io::stderr().write_all(&vec![b'x'; 8192]).unwrap(),
            "invalid-utf8" => std::io::stdout().write_all(&[0xff]).unwrap(),
            "path" => {
                if std::env::current_exe().unwrap().file_stem().unwrap() == "cm" {
                    print!("fake cm found on PATH");
                } else {
                    let backend =
                        Backend::new(std::env::current_dir().unwrap(), Limits::default()).unwrap();
                    assert_eq!(backend.executable, Path::new("cm"));
                    assert!(backend
                        .run(&producer_args())
                        .unwrap()
                        .contains("fake cm found on PATH"));
                    print!("PATH lookup verified");
                }
            }
            _ => panic!("unknown fake producer mode"),
        }
        std::io::stdout().flush().unwrap();
        std::io::stderr().flush().unwrap();
        std::process::exit(0);
    }

    #[test]
    fn checked_process_success_failure_timeout_and_both_pipe_caps() {
        let cwd = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        // Child-only environment avoids races with other tests and user PATH.
        // checked_run has no environment override in production; a wrapper test
        // process supplies each mode before exercising that exact function.
        for mode in [
            "success",
            "failure",
            "timeout",
            "stdout",
            "stderr",
            "invalid-utf8",
        ] {
            let output = Command::new(&executable)
                .args(strings(&[
                    "--exact",
                    "plastic::tests::check_fake_producer",
                    "--nocapture",
                ]))
                .env("SERIE_TEST_CHECK", mode)
                .current_dir(cwd.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
    }

    #[test]
    fn check_fake_producer() {
        let Ok(mode) = std::env::var("SERIE_TEST_CHECK") else {
            return;
        };
        // Isolated child process: no parallel tests are running here.
        std::env::set_var("SERIE_TEST_PRODUCER", &mode);
        let mut limits = Limits::default();
        if mode == "timeout" {
            limits.timeout = Duration::from_millis(100);
        }
        if mode == "stdout" || mode == "stderr" {
            limits.stdout_bytes = 4096;
            limits.stderr_bytes = 4096;
        }
        let started = Instant::now();
        let result = checked_run(
            &std::env::current_exe().unwrap(),
            &std::env::current_dir().unwrap(),
            &producer_args(),
            &limits,
        );
        match mode.as_str() {
            "success" => assert!(result.unwrap().contains("fake cm output")),
            "failure" => assert!(result.unwrap_err().0.contains("exited")),
            "timeout" => {
                assert!(result.unwrap_err().0.contains("deadline"));
                assert!(started.elapsed() < Duration::from_secs(3));
            }
            "stdout" | "stderr" => assert!(result
                .unwrap_err()
                .0
                .contains(&format!("{mode} output exceeded"))),
            "invalid-utf8" => assert!(result.unwrap_err().0.contains("non-UTF-8")),
            _ => panic!("unknown fake producer mode"),
        }
    }

    #[test]
    fn default_cm_is_resolved_from_path_in_workspace_with_spaces() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("fake tools");
        let workspace = root.path().join("workspace with spaces");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&workspace).unwrap();
        let executable = std::env::current_exe().unwrap();
        let fake_cm = bin.join(if cfg!(windows) { "cm.exe" } else { "cm" });
        std::fs::copy(&executable, fake_cm).unwrap();
        let output = Command::new(executable)
            .args(producer_args())
            .env("SERIE_TEST_PRODUCER", "path")
            .env("PATH", &bin)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("PATH lookup verified"));
    }
}
