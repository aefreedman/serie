use std::{
    hash::Hash,
    path::{Path, PathBuf},
};
#[cfg(test)]
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

use chrono::{DateTime, FixedOffset};
use rustc_hash::FxHashMap;

use crate::Result;

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitHash(String);

impl CommitHash {
    pub fn as_short_hash(&self) -> &str {
        if self.0.bytes().all(|b| b.is_ascii_digit()) {
            &self.0
        } else {
            self.0.get(..7).unwrap_or(&self.0)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CommitHash {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Commit {
    pub commit_hash: CommitHash,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<FixedOffset>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<FixedOffset>,
    pub subject: String,
    pub body: String,
    pub parent_commit_hashes: Vec<CommitHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ref {
    Tag {
        name: String,
        target: CommitHash,
    },
    Branch {
        name: String,
        target: CommitHash,
    },
    RemoteBranch {
        name: String,
        target: CommitHash,
    },
    Stash {
        name: String,
        message: String,
        target: CommitHash,
    },
}

impl Ref {
    pub fn name(&self) -> &str {
        match self {
            Ref::Tag { name, .. } => name,
            Ref::Branch { name, .. } => name,
            Ref::RemoteBranch { name, .. } => name,
            Ref::Stash { name, .. } => name,
        }
    }

    pub fn target(&self) -> &CommitHash {
        match self {
            Ref::Tag { target, .. } => target,
            Ref::Branch { target, .. } => target,
            Ref::RemoteBranch { target, .. } => target,
            Ref::Stash { target, .. } => target,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Head {
    Branch { name: String },
    Detached { target: CommitHash },
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum SortCommit {
    Chronological,
    Topological,
}

type CommitMap = FxHashMap<CommitHash, Commit>;
type CommitsMap = FxHashMap<CommitHash, Vec<CommitHash>>;

type RefMap = FxHashMap<CommitHash, Vec<Ref>>;

pub struct Repository {
    #[cfg(test)]
    path: PathBuf,
    plastic: Option<(crate::plastic::Backend, crate::plastic::Snapshot)>,
    commit_map: CommitMap,

    parents_map: CommitsMap,
    children_map: CommitsMap,

    ref_map: RefMap,
    head: Head,
    // to preserve order of the original commits from `git log`, we store the commit hashes
    commit_hashes: Vec<CommitHash>,
}

impl Repository {
    #[cfg(test)]
    pub fn load(
        path: &Path,
        sort: SortCommit,
        max_count: Option<usize>,
        mailmap: bool,
    ) -> Result<Self> {
        check_git_repository(path)?;

        let (mut ref_map, head) = load_refs(path);

        let stashes = load_all_stashes(path, mailmap);
        let commits = load_all_commits(path, sort, &head, &stashes, max_count, mailmap);
        if commits.is_empty() {
            return Err("no commits in the repository".into());
        }

        let commits = merge_stashes_to_commits(commits, stashes);
        let commit_hashes = commits.iter().map(|c| c.commit_hash.clone()).collect();

        let (parents_map, children_map) = build_commits_maps(&commits);
        let commit_map = to_commit_map(commits);

        let stash_ref_map = load_stashes_as_refs(path);
        merge_ref_maps(&mut ref_map, stash_ref_map);

        Ok(Self::new(
            path.to_path_buf(),
            commit_map,
            parents_map,
            children_map,
            ref_map,
            head,
            commit_hashes,
        ))
    }

    pub fn new(
        path: PathBuf,
        commit_map: CommitMap,
        parents_map: CommitsMap,
        children_map: CommitsMap,
        ref_map: RefMap,
        head: Head,
        commit_hashes: Vec<CommitHash>,
    ) -> Self {
        let _ = &path;
        Self {
            #[cfg(test)]
            path,
            plastic: None,
            commit_map,
            parents_map,
            children_map,
            ref_map,
            head,
            commit_hashes,
        }
    }

    /// Adapt the single qualified Plastic repository to the existing graph model.
    /// Primary parents stay first; ordinary merges add deduplicated layout edges.
    /// Typed integrations remain separate evidence; nonordinary links are not ancestry.
    pub fn load_plastic(path: &Path, max_count: Option<usize>) -> Result<Self> {
        let mut limits = crate::plastic::Limits::default();
        if let Some(n) = max_count {
            limits.changesets = n;
        }
        let backend = crate::plastic::Backend::new(path, limits)?;
        let snapshot = backend.load()?;
        Self::from_plastic(backend, snapshot)
    }

    pub fn from_plastic(
        backend: crate::plastic::Backend,
        snapshot: crate::plastic::Snapshot,
    ) -> Result<Self> {
        let positions: FxHashMap<_, _> = snapshot
            .changesets
            .iter()
            .enumerate()
            .map(|(index, cs)| (&cs.key, index))
            .collect();
        for (index, cs) in snapshot.changesets.iter().enumerate() {
            if let Some(parent) = &cs.primary_parent {
                if positions.get(parent).is_some_and(|p| *p <= index) {
                    return Err("unsupported history ordering/cycle: graph requires loaded primary parents after children".into());
                }
            }
        }
        let mut commits = Vec::new();
        for cs in &snapshot.changesets {
            let date = DateTime::parse_from_rfc3339(&cs.date)?;
            let (subject, body) = cs.comment.split_once('\n').unwrap_or((&cs.comment, ""));
            commits.push(Commit {
                commit_hash: cs.key.id.as_str().into(),
                author_name: cs.owner.clone(),
                author_email: String::new(),
                author_date: date,
                committer_name: cs.owner.clone(),
                committer_email: String::new(),
                committer_date: date,
                subject: subject.into(),
                body: format!(
                    "{}

Selector: {}
Branch: {}
Original date: {}
Object ID: {}
GUID: {}
Primary parent: {}",
                    body,
                    cs.key.selector(),
                    cs.branch,
                    cs.date,
                    cs.object_id,
                    cs.guid,
                    cs.primary_parent
                        .as_ref()
                        .map(|p| p.selector())
                        .unwrap_or_else(|| "root (PARENT=-1)".into())
                ),
                parent_commit_hashes: cs
                    .primary_parent
                    .iter()
                    .map(|p| p.id.as_str().into())
                    .collect(),
            });
        }
        let mut ordinary: Vec<_> = snapshot
            .integrations
            .iter()
            .filter(|i| i.kind == crate::plastic::IntegrationKind::Merge)
            .collect();
        ordinary.sort_by_key(|i| {
            (
                i.destination.selector(),
                i.source.selector(),
                i.object_id.clone(),
            )
        });
        for integration in ordinary {
            let (Some(source), Some(destination)) = (
                positions.get(&integration.source),
                positions.get(&integration.destination),
            ) else {
                continue;
            };
            if source <= destination {
                return Err("unsupported ordinary merge ordering/cycle: graph requires source after destination".into());
            }
            let source_hash: CommitHash = integration.source.id.as_str().into();
            let parents = &mut commits[*destination].parent_commit_hashes;
            if !parents.contains(&source_hash) {
                parents.push(source_hash);
            }
        }
        let hashes = commits.iter().map(|c| c.commit_hash.clone()).collect();
        let (parents, children) = build_commits_maps(&commits);
        let mut refs = RefMap::default();
        for r in &snapshot.references {
            if let Some(target) = &r.target {
                // Never equate an unrelated repository's changeset number.
                if target.repository != snapshot.loaded_changeset.repository {
                    continue;
                }
                let target: CommitHash = target.id.as_str().into();
                let annotation = match r.kind {
                    crate::plastic::RefKind::Branch => Ref::Branch {
                        name: r.name.clone(),
                        target: target.clone(),
                    },
                    crate::plastic::RefKind::Label => Ref::Tag {
                        name: r.name.clone(),
                        target: target.clone(),
                    },
                };
                refs.entry(target).or_default().push(annotation);
            }
        }
        refs.values_mut().for_each(|rs| rs.sort());
        let head = Head::Detached {
            target: snapshot.loaded_changeset.id.as_str().into(),
        };
        let mut repository = Self::new(
            PathBuf::new(),
            to_commit_map(commits),
            parents,
            children,
            refs,
            head,
            hashes,
        );
        repository.plastic = Some((backend, snapshot));
        Ok(repository)
    }

    pub fn snapshot(&self) -> Option<&crate::plastic::Snapshot> {
        self.plastic.as_ref().map(|(_, snapshot)| snapshot)
    }

    pub fn warnings(&self) -> Vec<String> {
        let Some(snapshot) = self.snapshot() else {
            return Vec::new();
        };
        let mut warnings = snapshot.warnings.clone();
        if !snapshot
            .changesets
            .iter()
            .any(|cs| cs.key == snapshot.loaded_changeset)
        {
            warnings.push(format!(
                "Loaded workspace changeset {} is outside the history window.",
                snapshot.loaded_changeset.selector()
            ));
        }
        let nonordinary = snapshot
            .integrations
            .iter()
            .filter(|i| i.kind != crate::plastic::IntegrationKind::Merge)
            .count();
        if nonordinary > 0 {
            warnings.push(format!("{nonordinary} nonordinary integration links omitted from drawing (types retained in dump/details)."));
        }
        let outside = snapshot
            .integrations
            .iter()
            .filter(|i| {
                i.kind == crate::plastic::IntegrationKind::Merge
                    && (!snapshot.changesets.iter().any(|c| c.key == i.source)
                        || !snapshot.changesets.iter().any(|c| c.key == i.destination))
            })
            .count();
        if outside > 0 {
            warnings.push(format!("{outside} ordinary merge links have endpoints outside the loaded window; omitted from drawing, retained in dump/details."));
        }
        let unsupported = snapshot
            .references
            .iter()
            .filter(|r| {
                r.target
                    .as_ref()
                    .is_none_or(|t| t.repository != snapshot.loaded_changeset.repository)
            })
            .count();
        if unsupported > 0 {
            warnings.push(format!("{unsupported} references have unsupported/missing targets; retained in dump, not annotated."));
        }
        warnings
    }

    pub fn copy_selector(&self, value: &str) -> String {
        if let Some(snapshot) = self.snapshot() {
            if let Some(cs) = snapshot
                .changesets
                .iter()
                .find(|cs| cs.key.id.as_str() == value)
            {
                return cs.key.selector();
            }
            if let Some(r) = snapshot.references.iter().find(|r| r.name == value) {
                return r.selector();
            }
        }
        value.to_owned()
    }

    pub fn commit(&self, commit_hash: &CommitHash) -> Option<&Commit> {
        self.commit_map.get(commit_hash)
    }

    pub fn all_commits(&self) -> Vec<&Commit> {
        self.commit_hashes
            .iter()
            .filter_map(|hash| self.commit(hash))
            .collect()
    }

    pub fn parents_hash(&self, commit_hash: &CommitHash) -> Vec<&CommitHash> {
        self.parents_map
            .get(commit_hash)
            .map(|hs| hs.iter().collect::<Vec<&CommitHash>>())
            .unwrap_or_default()
    }

    pub fn children_hash(&self, commit_hash: &CommitHash) -> Vec<&CommitHash> {
        self.children_map
            .get(commit_hash)
            .map(|hs| hs.iter().collect::<Vec<&CommitHash>>())
            .unwrap_or_default()
    }

    pub fn refs(&self, commit_hash: &CommitHash) -> Vec<&Ref> {
        self.ref_map
            .get(commit_hash)
            .map(|refs| refs.iter().collect::<Vec<&Ref>>())
            .unwrap_or_default()
    }

    pub fn all_refs(&self) -> Vec<&Ref> {
        self.ref_map.values().flatten().collect()
    }

    pub fn head(&self) -> &Head {
        &self.head
    }

    pub fn plastic_detail(&self, id: &str) -> Result<crate::plastic::Detail> {
        let (backend, snapshot) = self.plastic.as_ref().ok_or("not a Plastic repository")?;
        let id = crate::plastic::ChangesetId::new(id)?;
        let cs = snapshot
            .changesets
            .iter()
            .find(|cs| cs.key.id == id)
            .ok_or("detail changeset is outside loaded window")?;
        Ok(backend.detail(&cs.key)?)
    }

    pub fn commit_detail(&self, commit_hash: &CommitHash) -> (Commit, Vec<FileChange>) {
        let mut commit = self.commit(commit_hash).unwrap().clone();
        if let Some((_, snapshot)) = &self.plastic {
            let cs = snapshot
                .changesets
                .iter()
                .find(|c| c.key.id.as_str() == commit_hash.as_str())
                .unwrap();
            for warning in self.warnings() {
                commit
                    .body
                    .push_str(&format!("{}WARNING: {warning}", char::from(10)));
            }
            for link in snapshot
                .integrations
                .iter()
                .filter(|i| i.destination == cs.key || i.source == cs.key)
            {
                commit.body.push_str(&format!(
                    "
Integration [{}]: {} -> {}; base: {}",
                    link.raw_type,
                    link.source.selector(),
                    link.destination.selector(),
                    link.base
                        .as_ref()
                        .map(|b| b.selector())
                        .unwrap_or_else(|| "none".into())
                ));
            }
            let changes = match self.plastic_detail(cs.key.id.as_str()) {
                Ok(detail) => detail
                    .paths
                    .into_iter()
                    .map(|p| FileChange::Plastic {
                        description: format!(
                            "{} {} -> {} [rev {}, parent rev {}]",
                            p.change_type,
                            p.source_path,
                            p.destination_path,
                            p.revision_id,
                            p.parent_revision_id
                        ),
                    })
                    .collect(),
                Err(error) => {
                    commit.body.push_str(&format!(
                        "
ERROR loading changed paths: {error}"
                    ));
                    Vec::new()
                }
            };
            return (commit, changes);
        }
        #[cfg(test)]
        let changes = if commit.parent_commit_hashes.is_empty() {
            get_initial_commit_additions(&self.path, commit_hash)
        } else {
            get_diff_summary(&self.path, commit_hash)
        };
        #[cfg(not(test))]
        let changes = Vec::new();
        (commit, changes)
    }
}

#[cfg(test)]
fn check_git_repository(path: &Path) -> Result<()> {
    if !is_inside_work_tree(path) && !is_bare_repository(path) {
        let msg = "not a git repository (or any of the parent directories)";
        return Err(msg.into());
    }
    Ok(())
}

#[cfg(test)]
fn is_inside_work_tree(path: &Path) -> bool {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(path)
        .output()
        .unwrap();
    output.status.success() && output.stdout == b"true\n"
}

#[cfg(test)]
fn is_bare_repository(path: &Path) -> bool {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--is-bare-repository")
        .current_dir(path)
        .output()
        .unwrap();
    output.status.success() && output.stdout == b"true\n"
}

#[cfg(test)]
fn load_all_commits(
    path: &Path,
    sort: SortCommit,
    head: &Head,
    stashes: &[Commit],
    max_count: Option<usize>,
    mailmap: bool,
) -> Vec<Commit> {
    let mut cmd = Command::new("git");
    cmd.arg("log");

    cmd.arg(match sort {
        SortCommit::Chronological => "--date-order",
        SortCommit::Topological => "--topo-order",
    })
    .arg(format!("--pretty={}", load_commits_format(mailmap)))
    .arg("--date=iso-strict")
    .arg("-z"); // use NUL as a delimiter

    // exclude stashes and other refs
    cmd.arg("--branches").arg("--remotes").arg("--tags");

    // commits that are reachable from the stashes
    stashes.iter().for_each(|stash| {
        cmd.arg(stash.parent_commit_hashes[0].as_str());
    });

    if !matches!(head, Head::None) {
        cmd.arg("HEAD");
    }

    if let Some(n) = max_count {
        cmd.arg("--max-count").arg(n.to_string());
    }

    cmd.current_dir(path).stdout(Stdio::piped());

    let mut process = cmd.spawn().unwrap();

    let stdout = process.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut commits = Vec::new();

    for bytes in reader.split(b'\0') {
        let bytes = bytes.unwrap();
        let s = String::from_utf8_lossy(&bytes);

        let parts: Vec<&str> = s.split('\x1f').collect();
        if parts.len() != 10 {
            panic!("unexpected number of parts: {} [{}]", parts.len(), s);
        }

        let commit = Commit {
            commit_hash: parts[0].into(),
            author_name: parts[1].into(),
            author_email: parts[2].into(),
            author_date: parse_iso_date(parts[3]),
            committer_name: parts[4].into(),
            committer_email: parts[5].into(),
            committer_date: parse_iso_date(parts[6]),
            subject: parts[7].into(),
            body: parts[8].into(),
            parent_commit_hashes: parse_parent_commit_hashes(parts[9]),
        };

        commits.push(commit);
    }

    process.wait().unwrap();

    commits
}

#[cfg(test)]
fn load_all_stashes(path: &Path, mailmap: bool) -> Vec<Commit> {
    let mut cmd = Command::new("git")
        .arg("stash")
        .arg("list")
        .arg(format!("--pretty={}", load_commits_format(mailmap)))
        .arg("--date=iso-strict")
        .arg("-z") // use NUL as a delimiter
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut commits = Vec::new();

    for bytes in reader.split(b'\0') {
        let bytes = bytes.unwrap();
        let s = String::from_utf8_lossy(&bytes);

        let parts: Vec<&str> = s.split('\x1f').collect();
        if parts.len() != 10 {
            panic!("unexpected number of parts: {} [{}]", parts.len(), s);
        }

        let commit = Commit {
            commit_hash: parts[0].into(),
            author_name: parts[1].into(),
            author_email: parts[2].into(),
            author_date: parse_iso_date(parts[3]),
            committer_name: parts[4].into(),
            committer_email: parts[5].into(),
            committer_date: parse_iso_date(parts[6]),
            subject: parts[7].into(),
            body: parts[8].into(),
            parent_commit_hashes: parse_parent_commit_hashes(parts[9]),
        };

        commits.push(commit);
    }

    cmd.wait().unwrap();

    commits
}

#[cfg(test)]
fn load_commits_format(mailmap: bool) -> String {
    // The uppercase name/email placeholders (`%aN`, `%aE`, `%cN`, `%cE`) resolve
    // identities through the repository's .mailmap, while the lowercase variants
    // use the raw values recorded in each commit.
    let format = if mailmap {
        [
            "%H", "%aN", "%aE", "%ad", "%cN", "%cE", "%cd", "%s", "%b", "%P",
        ]
    } else {
        [
            "%H", "%an", "%ae", "%ad", "%cn", "%ce", "%cd", "%s", "%b", "%P",
        ]
    };
    format.join("%x1f") // use Unit Separator as a delimiter
}

#[cfg(test)]
fn parse_iso_date(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

#[cfg(test)]
fn parse_parent_commit_hashes(s: &str) -> Vec<CommitHash> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(' ').map(|s| s.into()).collect()
}

fn build_commits_maps(commits: &Vec<Commit>) -> (CommitsMap, CommitsMap) {
    let mut parents_map: CommitsMap = FxHashMap::default();
    let mut children_map: CommitsMap = FxHashMap::default();
    for commit in commits {
        let hash = &commit.commit_hash;
        for parent_hash in &commit.parent_commit_hashes {
            parents_map
                .entry(hash.clone())
                .or_default()
                .push(parent_hash.clone());
            children_map
                .entry(parent_hash.clone())
                .or_default()
                .push(hash.clone());
        }
    }

    (parents_map, children_map)
}

fn to_commit_map(commits: Vec<Commit>) -> CommitMap {
    commits
        .into_iter()
        .map(|commit| (commit.commit_hash.clone(), commit))
        .collect()
}

#[cfg(test)]
fn merge_stashes_to_commits(commits: Vec<Commit>, stashes: Vec<Commit>) -> Vec<Commit> {
    // Stash commit has multiple parent commits, but the first parent commit is the commit that the stash was created from.
    // If the first parent commit is not found, the stash commit is ignored.
    let mut ret = Vec::new();
    let mut statsh_map: FxHashMap<CommitHash, Vec<Commit>> =
        stashes
            .into_iter()
            .fold(FxHashMap::default(), |mut acc, commit| {
                let parent = commit.parent_commit_hashes[0].clone();
                acc.entry(parent).or_default().push(commit);
                acc
            });
    for commit in commits {
        if let Some(stashes) = statsh_map.remove(&commit.commit_hash) {
            for stash in stashes {
                ret.push(stash);
            }
        }
        ret.push(commit);
    }
    ret
}

#[cfg(test)]
fn load_refs(path: &Path) -> (RefMap, Head) {
    let mut cmd = Command::new("git")
        .arg("show-ref")
        .arg("--head")
        .arg("--dereference")
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut ref_map = RefMap::default();
    let mut tag_map: FxHashMap<String, Ref> = FxHashMap::default();
    let mut head: Head = Head::None;

    for line in reader.lines() {
        let line = line.unwrap();

        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            panic!("unexpected number of parts: {} [{}]", parts.len(), line);
        }

        let hash = parts[0];
        let refs = parts[1];

        if refs == "HEAD" {
            head = if let Some(branch) = get_current_branch(path) {
                Head::Branch { name: branch }
            } else {
                Head::Detached {
                    target: hash.into(),
                }
            };
        } else if let Some(r) = parse_branch_refs(hash, refs) {
            ref_map.entry(hash.into()).or_default().push(r);
        } else if let Some(r) = parse_tag_refs(hash, refs) {
            // if annotated tag exists, it will be overwritten by the following line of the same tag
            // this will make the tag point to the commit that the annotated tag points to
            tag_map.insert(r.name().into(), r);
        }
    }

    for tag in tag_map.into_values() {
        ref_map.entry(tag.target().clone()).or_default().push(tag);
    }

    ref_map.values_mut().for_each(|refs| refs.sort());

    cmd.wait().unwrap();

    (ref_map, head)
}

#[cfg(test)]
fn load_stashes_as_refs(path: &Path) -> RefMap {
    let format = ["%gd", "%H", "%s"].join("%x1f"); // use Unit Separator as a delimiter
    let mut cmd = Command::new("git")
        .arg("stash")
        .arg("list")
        .arg(format!("--format={format}"))
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut ref_map = RefMap::default();

    for line in reader.lines() {
        let line = line.unwrap();

        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() != 3 {
            panic!("unexpected number of parts: {} [{}]", parts.len(), line);
        }

        let name = parts[0];
        let hash = parts[1];
        let subject = parts[2];

        let r = Ref::Stash {
            name: name.into(),
            message: subject.into(),
            target: hash.into(),
        };

        ref_map.entry(hash.into()).or_default().push(r);
    }

    cmd.wait().unwrap();

    ref_map
}

#[cfg(test)]
fn merge_ref_maps(m1: &mut RefMap, m2: RefMap) {
    for (k, v) in m2 {
        m1.entry(k).or_default().extend(v);
    }
}

#[cfg(test)]
fn parse_branch_refs(hash: &str, refs: &str) -> Option<Ref> {
    if refs.starts_with("refs/heads/") {
        let name = refs.trim_start_matches("refs/heads/");
        Some(Ref::Branch {
            name: name.into(),
            target: hash.into(),
        })
    } else if refs.starts_with("refs/remotes/") {
        let name = refs.trim_start_matches("refs/remotes/");
        Some(Ref::RemoteBranch {
            name: name.into(),
            target: hash.into(),
        })
    } else {
        None
    }
}

#[cfg(test)]
fn parse_tag_refs(hash: &str, refs: &str) -> Option<Ref> {
    if refs.starts_with("refs/tags/") {
        let name = refs.trim_start_matches("refs/tags/");
        let name = name.trim_end_matches("^{}");
        Some(Ref::Tag {
            name: name.into(),
            target: hash.into(),
        })
    } else {
        None
    }
}

#[cfg(test)]
fn get_current_branch(path: &Path) -> Option<String> {
    let mut cmd = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let branch = if let Some(line) = reader.lines().next() {
        line.ok()
    } else {
        None
    };

    cmd.wait().unwrap();

    branch
}

#[derive(Debug)]
pub enum FileChange {
    Plastic { description: String },
    Add { path: String },
    Modify { path: String },
    Delete { path: String },
    Move { from: String, to: String },
}

#[cfg(test)]
pub fn get_diff_summary(path: &Path, commit_hash: &CommitHash) -> Vec<FileChange> {
    let mut cmd = Command::new("git")
        .arg("diff")
        .arg("--name-status")
        .arg(format!("{}^", commit_hash.0))
        .arg(&commit_hash.0)
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut changes = Vec::new();

    for line in reader.lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split('\t').collect();

        match &parts[0][0..1] {
            "A" => changes.push(FileChange::Add {
                path: parts[1].into(),
            }),
            "M" => changes.push(FileChange::Modify {
                path: parts[1].into(),
            }),
            "D" => changes.push(FileChange::Delete {
                path: parts[1].into(),
            }),
            "R" => changes.push(FileChange::Move {
                from: parts[1].into(),
                to: parts[2].into(),
            }),
            _ => {}
        }
    }

    cmd.wait().unwrap();

    changes
}

#[cfg(test)]
pub fn get_initial_commit_additions(path: &Path, commit_hash: &CommitHash) -> Vec<FileChange> {
    let mut cmd = Command::new("git")
        .arg("ls-tree")
        .arg("--name-status")
        .arg("-r") // the empty tree hash
        .arg(&commit_hash.0)
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = cmd.stdout.take().expect("failed to open stdout");

    let reader = BufReader::new(stdout);

    let mut changes = Vec::new();

    for line in reader.lines() {
        let line = line.unwrap();
        changes.push(FileChange::Add { path: line });
    }

    cmd.wait().unwrap();

    changes
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("commit_hashes", &self.commit_hashes)
            .finish()
    }
}
