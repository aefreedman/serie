use crate::{
    git::{Repository, SortCommit, SortCommit::*},
    graph,
    plastic::*,
    plastic_integration_tests::snapshot,
};

// Synthetic fixtures, independent of query order and rendering.
fn fixture(rows: &[(&str, Option<&str>, u32)], merges: &[(&str, &str)]) -> Snapshot {
    let mut snap = snapshot();
    let template = snap.changesets[0].clone();
    snap.changesets = rows
        .iter()
        .map(|(id, parent, second)| {
            let mut cs = template.clone();
            cs.key.id = ChangesetId::new(id).unwrap();
            cs.primary_parent = parent.map(|id| QualifiedChangeset {
                repository: cs.key.repository.clone(),
                id: ChangesetId::new(id).unwrap(),
            });
            cs.date = format!("2025-01-01T00:00:{second:02}+00:00");
            cs.branch = if id.parse::<u64>().unwrap() % 2 == 0 {
                "/main/b"
            } else {
                "/main/a"
            }
            .into();
            cs
        })
        .collect();
    let link = snap.integrations[0].clone();
    snap.integrations = merges
        .iter()
        .map(|(source, destination)| {
            let mut link = link.clone();
            link.source.id = ChangesetId::new(source).unwrap();
            link.destination.id = ChangesetId::new(destination).unwrap();
            link.kind = IntegrationKind::Merge;
            link
        })
        .collect();
    snap.references.clear();
    snap.loaded_changeset = snap.changesets[0].key.clone();
    snap
}

fn ordered(snap: Snapshot, sort: SortCommit) -> Repository {
    Repository::from_plastic_ordered(Backend::new(".", Limits::default()).unwrap(), snap, sort)
        .unwrap()
}

fn ids(repo: &Repository) -> Vec<&str> {
    repo.all_commits()
        .iter()
        .map(|c| c.commit_hash.as_str())
        .collect()
}

fn assert_dag(repo: &Repository) {
    let commits = repo.all_commits();
    for (i, cs) in commits.iter().enumerate() {
        for p in &cs.parent_commit_hashes {
            if let Some(j) = commits.iter().position(|c| c.commit_hash == *p) {
                assert!(j > i, "parent {p:?} precedes child");
            }
        }
    }
    assert_eq!(graph::calc_graph(repo, None).commits.len(), commits.len());
}

#[test]
fn interleaved_branches_and_dump_follow_selected_order() {
    let snap = fixture(
        &[
            ("5", Some("3"), 50),
            ("4", Some("2"), 40),
            ("3", Some("0"), 30),
            ("2", Some("0"), 20),
            ("0", None, 0),
        ],
        &[],
    );
    for (sort, expected) in [
        (Chronological, ["5", "4", "3", "2", "0"]),
        (Topological, ["5", "3", "4", "2", "0"]),
    ] {
        let repo = ordered(snap.clone(), sort);
        assert_eq!(ids(&repo), expected);
        assert_dag(&repo);
        let mut bytes = Vec::new();
        crate::write_repository_dump(&repo, None, &mut bytes).unwrap();
        let dump = String::from_utf8(bytes).unwrap();
        let rows: Vec<_> = dump
            .lines()
            .filter(|l| l.starts_with("o cs:") || l.starts_with("* cs:"))
            .collect();
        for (row, id) in rows.iter().zip(expected) {
            assert!(row
                .split_whitespace()
                .nth(1)
                .unwrap()
                .starts_with(&format!("cs:{id}@rep:")));
        }
        assert_eq!(rows.len(), expected.len());
        assert_eq!(repo.snapshot().unwrap().changesets[1].key.id.as_str(), "4");
    }
}

#[test]
fn skew_merges_and_ties_are_deterministic_and_dag_safe() {
    let snap = fixture(
        &[
            ("9", Some("3"), 10),
            ("8", Some("2"), 10),
            ("3", Some("0"), 59),
            ("2", Some("0"), 58),
            ("0", None, 57),
        ],
        &[("2", "9"), ("2", "9"), ("3", "9")],
    );
    for sort in [Chronological, Topological] {
        let repo = ordered(snap.clone(), sort);
        assert_eq!(ids(&repo), ["9", "3", "8", "2", "0"]);
        assert_dag(&repo);
        let mut reversed = snap.clone();
        reversed.changesets.reverse();
        reversed.integrations.reverse();
        assert_eq!(ids(&repo), ids(&ordered(reversed, sort)));
        assert_eq!(
            repo.snapshot().unwrap().changesets[0].date,
            "2025-01-01T00:00:10+00:00"
        );
        assert_eq!(repo.snapshot().unwrap().integrations.len(), 3);
    }
    let ties = fixture(&[("2", None, 1), ("10", None, 1), ("9", None, 1)], &[]);
    for sort in [Chronological, Topological] {
        assert_eq!(ids(&ordered(ties.clone(), sort)), ["10", "9", "2"]);
    }
}

#[test]
fn merge_topo_prefers_primary_when_both_parents_unblocked() {
    let snap = fixture(
        &[
            ("5", Some("1"), 5),
            ("1", Some("0"), 1),
            ("2", Some("0"), 2),
            ("0", None, 0),
        ],
        &[("2", "5")],
    );
    assert_eq!(
        ids(&ordered(snap.clone(), Chronological)),
        ["5", "2", "1", "0"]
    );
    assert_eq!(ids(&ordered(snap, Topological)), ["5", "1", "2", "0"]);
}

#[test]
fn window_and_qualified_missing_endpoints_remain_evidence() {
    let mut snap = fixture(&[("5", Some("3"), 5), ("4", Some("2"), 4)], &[("1", "5")]);
    snap.history_truncated = true;
    snap.integrations_truncated = true;
    snap.references_truncated = true;
    snap.missing_endpoints = vec![snap.changesets[0].primary_parent.clone().unwrap()];
    // Foreign endpoint with SAME numeric ID must not constrain local ordering.
    let mut foreign = snap.changesets[1].key.clone();
    foreign.repository.name = "other-synthetic-repo".into();
    snap.integrations[0].source = foreign;
    for sort in [Chronological, Topological] {
        let repo = ordered(snap.clone(), sort);
        assert_dag(&repo);
        assert_eq!(ids(&repo), ["5", "4"]);
        let raw = repo.snapshot().unwrap();
        assert!(raw.history_truncated && raw.integrations_truncated && raw.references_truncated);
        assert_eq!(raw.missing_endpoints, snap.missing_endpoints);
        assert_eq!(raw.integrations[0].source, snap.integrations[0].source);
        assert!(repo
            .warnings()
            .iter()
            .any(|w| w.contains("outside the loaded window")));
    }
}

#[test]
fn primary_and_merge_cycles_fail_closed_in_both_modes() {
    for snap in [
        fixture(&[("1", Some("1"), 1)], &[]),
        fixture(&[("2", Some("1"), 2), ("1", Some("2"), 1)], &[]),
        fixture(&[("2", Some("1"), 2), ("1", None, 1)], &[("2", "1")]),
    ] {
        for sort in [Chronological, Topological] {
            let error = Repository::from_plastic_ordered(
                Backend::new(".", Limits::default()).unwrap(),
                snap.clone(),
                sort,
            )
            .unwrap_err();
            assert!(error.to_string().contains("cycle"));
        }
    }
}

#[test]
fn timezone_dates_compare_instants_not_original_strings() {
    let mut snap = fixture(&[("1", None, 0), ("2", None, 0)], &[]);
    snap.changesets[0].date = "2025-01-01T01:00:00+01:00".into();
    snap.changesets[1].date = "2025-01-01T00:00:00+00:00".into();
    for sort in [Chronological, Topological] {
        assert_eq!(ids(&ordered(snap.clone(), sort)), ["2", "1"]);
    }
}

#[test]
fn maximum_bounded_chain_is_iterative_and_missing_root_stays_outside() {
    let mut snap = fixture(&[("1", Some("0"), 1)], &[]);
    let template = snap.changesets[0].clone();
    snap.changesets = (1..=10000)
        .map(|n| {
            let mut cs = template.clone();
            cs.key.id = ChangesetId::new(&n.to_string()).unwrap();
            cs.primary_parent.as_mut().unwrap().id =
                ChangesetId::new(&(n - 1).to_string()).unwrap();
            cs
        })
        .collect();
    snap.history_truncated = true;
    for sort in [Chronological, Topological] {
        let indices = crate::ordering::indices(&snap, sort).unwrap();
        assert_eq!(indices.len(), 10000);
        assert_eq!(indices, (0..10000).rev().collect::<Vec<_>>());
        assert_eq!(snap.changesets.len(), 10000);
    }
}

#[test]
fn nonordinary_links_do_not_add_constraints_and_reload_recomputes_order() {
    let mut snap = fixture(&[("2", None, 2), ("1", None, 1)], &[("2", "1"), ("1", "2")]);
    for kind in [
        IntegrationKind::CherryPick,
        IntegrationKind::SubtractiveCherryPick,
        IntegrationKind::Interval,
        IntegrationKind::IntervalCherryPick,
        IntegrationKind::SubtractiveIntervalCherryPick,
        IntegrationKind::Unknown("synthetic".into()),
    ] {
        for link in &mut snap.integrations {
            link.kind = kind.clone();
        }
        for sort in [Chronological, Topological] {
            let repo = ordered(snap.clone(), sort);
            assert_eq!(ids(&repo), ["2", "1"]);
            assert_dag(&repo);
        }
    }
    snap.changesets[1].date = "2025-01-01T00:00:03+00:00".into();
    for sort in [Chronological, Topological] {
        assert_eq!(ids(&ordered(snap.clone(), sort)), ["1", "2"]);
    }
}

#[test]
fn qualified_primary_parent_is_not_an_unrelated_numeric_layout_edge() {
    let mut snap = fixture(&[("2", Some("1"), 2), ("1", None, 1)], &[]);
    snap.changesets[0]
        .primary_parent
        .as_mut()
        .unwrap()
        .repository
        .name = "other-synthetic-repo".into();
    for sort in [Chronological, Topological] {
        let repo = ordered(snap.clone(), sort);
        assert!(repo.all_commits()[0].parent_commit_hashes.is_empty());
        assert_eq!(
            repo.snapshot().unwrap().changesets[0].primary_parent,
            snap.changesets[0].primary_parent
        );
        assert!(repo.all_commits()[0].body.contains("other-synthetic-repo"));
    }
    snap.changesets.push(snap.changesets[0].clone());
    for sort in [Chronological, Topological] {
        assert!(crate::ordering::indices(&snap, sort)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
    }
}
