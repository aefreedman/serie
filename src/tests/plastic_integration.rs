use std::path::Path;

use crate::{
    color, config,
    git::{CommitHash, Head, Repository},
    graph::{self, CellWidthType, GraphImageManager, GraphImageWidthMode, GraphStyle},
    plastic::*,
    protocol::ImageProtocol,
};

fn snapshot() -> Snapshot {
    let changesets =
        parse_changesets(include_str!("../../tests/fixtures/plastic/changesets.xml")).unwrap();
    let repository = changesets[0].key.repository.clone();
    let workspace = parse_status(include_str!("../../tests/fixtures/plastic/status.xml")).unwrap();
    Snapshot {
        integrations: parse_integrations(
            include_str!("../../tests/fixtures/plastic/merges.xml"),
            &repository,
        )
        .unwrap(),
        references: parse_references(
            include_str!("../../tests/fixtures/plastic/branches.xml"),
            RefKind::Branch,
        )
        .unwrap(),
        loaded_changeset: QualifiedChangeset {
            repository,
            id: workspace.loaded.id.clone(),
        },
        changesets,
        workspace,
        missing_endpoints: vec![],
        warnings: vec!["fixture warning".into()],
        history_truncated: false,
        integrations_truncated: false,
        references_truncated: false,
    }
}
fn adapt(snapshot: Snapshot) -> Repository {
    // Constructor does not execute cm or Git; workspace is this project, never sandbox.
    Repository::from_plastic(
        Backend::new(Path::new("."), Limits::default()).unwrap(),
        snapshot,
    )
    .unwrap()
}

#[test]
fn numeric_ids_are_complete_and_safe() {
    for id in ["0", "16", "12345678901234567890"] {
        assert_eq!(CommitHash::from(id).as_short_hash(), id);
    }
    assert_eq!(CommitHash::from("abc").as_short_hash(), "abc");
    assert_eq!(CommitHash::from("1234567abcdef").as_short_hash(), "1234567");
}

#[test]
fn adapter_keeps_primary_edges_separate_and_selectors_qualified() {
    let repo = adapt(snapshot());
    assert_eq!(repo.all_commits().len(), 40);
    let cs17 = repo.commit(&CommitHash::from("17")).unwrap();
    assert_eq!(
        cs17.parent_commit_hashes,
        vec![CommitHash::from("13"), CommitHash::from("14")]
    );
    assert!(repo
        .parents_hash(&CommitHash::from("17"))
        .contains(&&CommitHash::from("14")));
    assert_eq!(
        repo.snapshot()
            .unwrap()
            .changesets
            .iter()
            .find(|c| c.key.id.as_str() == "17")
            .unwrap()
            .primary_parent
            .as_ref()
            .unwrap()
            .id
            .as_str(),
        "13"
    );
    assert!(cs17.body.contains("Original date:"));
    assert!(matches!(repo.head(), Head::Detached { target } if target.as_str() == "16"));
    assert!(repo.copy_selector("16").starts_with("cs:16@rep:"));
    assert!(repo.copy_selector("/main").starts_with("br:/main@rep:"));
    assert!(!repo
        .warnings()
        .iter()
        .any(|w| w.contains("integration links omitted")));
    assert!(repo.warnings().iter().any(|w| w == "fixture warning"));
    assert_eq!(repo.all_refs().len(), 48);
}

#[test]
fn text_graph_has_nodes_and_no_image_uploads() {
    let repo = adapt(snapshot());
    let graph = graph::calc_graph(&repo, None);
    let colors = color::GraphColorSet::new(&config::GraphConfig::default().color);
    let mut manager = GraphImageManager::new(
        &graph,
        &colors,
        CellWidthType::Double,
        GraphStyle::Rounded,
        GraphImageWidthMode::Compact,
        ImageProtocol::Text,
    );
    for commit in repo.all_commits() {
        manager.ensure_uploaded(&commit.commit_hash);
        let cells = manager.prepared_image(&commit.commit_hash).cells();
        let (node_x, _) = graph.commit_pos_map[&commit.commit_hash];
        assert_ne!(cells[node_x * 2].symbol(), " ");
        assert!(cells
            .iter()
            .all(|c| !c.skip() && !c.symbol().contains('\u{1b}')));
    }
    assert!(manager.drain_pending_uploads().is_empty());
    assert!(manager.image_ids().is_empty());
}

#[test]
fn text_graph_colors_nodes_and_keeps_same_lane_connections() {
    let repo = small_topology(
        &[
            ("3", Some("1")),
            ("2", Some("0")),
            ("1", Some("0")),
            ("0", None),
        ],
        &[],
    );
    let graph = graph::calc_graph(&repo, None);
    let colors = color::GraphColorSet::new(&config::GraphConfig::default().color);
    for width in [CellWidthType::Single, CellWidthType::Double] {
        let mut manager = GraphImageManager::new(
            &graph,
            &colors,
            width,
            GraphStyle::Rounded,
            GraphImageWidthMode::Compact,
            ImageProtocol::Text,
        );
        for commit in repo.all_commits() {
            manager.ensure_uploaded(&commit.commit_hash);
            let cells = manager.prepared_image(&commit.commit_hash).cells();
            for cell in cells.iter().filter(|c| c.symbol() != " ") {
                assert!(matches!(
                    cell.style().fg,
                    Some(ratatui::style::Color::Rgb(..))
                ));
            }
        }
        let child = CommitHash::from("3");
        let parent = CommitHash::from("1");
        let (child_x, child_y) = graph.commit_pos_map[&child];
        let (parent_x, parent_y) = graph.commit_pos_map[&parent];
        assert_eq!(child_x, parent_x);
        assert_eq!(parent_y, child_y + 2);
        let scale = if width == CellWidthType::Single { 1 } else { 2 };
        let middle = graph.commits[child_y + 1];
        let cells = manager.prepared_image(&middle.commit_hash).cells();
        assert_eq!(cells[child_x * scale].symbol(), "│");
        assert_eq!(
            cells[child_x * scale].style().fg,
            manager.prepared_image(&child).cells()[child_x * scale]
                .style()
                .fg,
        );
    }
}

fn small_topology(parents: &[(&str, Option<&str>)], merges: &[(&str, &str)]) -> Repository {
    let mut snap = snapshot();
    let template = snap.changesets[0].clone();
    snap.changesets = parents
        .iter()
        .map(|(id, parent)| {
            let mut changeset = template.clone();
            changeset.key.id = ChangesetId::new(*id).unwrap();
            changeset.primary_parent = parent.map(|parent| QualifiedChangeset {
                repository: changeset.key.repository.clone(),
                id: ChangesetId::new(parent).unwrap(),
            });
            changeset
        })
        .collect();
    let template = snap.integrations[0].clone();
    snap.integrations = merges
        .iter()
        .map(|(source, destination)| {
            let mut link = template.clone();
            link.source.id = ChangesetId::new(*source).unwrap();
            link.destination.id = ChangesetId::new(*destination).unwrap();
            link.kind = IntegrationKind::Merge;
            link
        })
        .collect();
    snap.references.clear();
    snap.loaded_changeset = snap.changesets[0].key.clone();
    adapt(snap)
}

fn assert_text_rows(repo: &Repository, width: CellWidthType, expected: &[&str]) {
    let graph = graph::calc_graph(repo, None);
    let colors = color::GraphColorSet::new(&config::GraphConfig::default().color);
    let mut manager = GraphImageManager::new(
        &graph,
        &colors,
        width,
        GraphStyle::Rounded,
        GraphImageWidthMode::Compact,
        ImageProtocol::Text,
    );
    let rows: Vec<String> = graph
        .commits
        .iter()
        .map(|commit| {
            manager.ensure_uploaded(&commit.commit_hash);
            let cells = manager.prepared_image(&commit.commit_hash).cells();
            let scale = if width == CellWidthType::Single { 1 } else { 2 };
            let (node_x, _) = graph.commit_pos_map[&commit.commit_hash];
            assert_eq!(cells[node_x * scale].symbol(), "●");
            assert_eq!(cells.iter().filter(|cell| cell.symbol() == "●").count(), 1);
            cells.iter().map(|cell| cell.symbol()).collect()
        })
        .collect();
    assert_eq!(rows, expected);
    assert!(manager.drain_pending_uploads().is_empty());
}

#[test]
fn text_adjacent_changesets_have_distinct_node_markers() {
    let repo = small_topology(
        &[
            ("3", Some("2")),
            ("2", Some("1")),
            ("1", Some("0")),
            ("0", None),
        ],
        &[],
    );
    assert_text_rows(&repo, CellWidthType::Single, &["●", "●", "●", "●"]);
    assert_text_rows(&repo, CellWidthType::Double, &["● ", "● ", "● ", "● "]);
}

#[test]
fn text_three_child_fork_preserves_every_exact_cell() {
    let repo = small_topology(
        &[
            ("3", Some("0")),
            ("2", Some("0")),
            ("1", Some("0")),
            ("0", None),
        ],
        &[],
    );
    let graph = graph::calc_graph(&repo, None);
    let lanes: Vec<_> = graph
        .commits
        .iter()
        .map(|commit| graph.commit_pos_map[&commit.commit_hash].0)
        .collect();
    assert_eq!(lanes, [0, 1, 2, 0]);
    assert_text_rows(&repo, CellWidthType::Single, &["●  ", "│● ", "││●", "●┴╯"]);
    assert_text_rows(
        &repo,
        CellWidthType::Double,
        &["●     ", "│ ●   ", "│ │ ● ", "●─┴─╯ "],
    );
}

#[test]
fn text_ordinary_merge_preserves_every_exact_cell() {
    let repo = small_topology(
        &[
            ("3", Some("2")),
            ("2", Some("0")),
            ("1", Some("0")),
            ("0", None),
        ],
        &[("1", "3")],
    );
    assert_text_rows(&repo, CellWidthType::Single, &["●╮", "●│", "│●", "●╯"]);
    assert_text_rows(
        &repo,
        CellWidthType::Double,
        &["●─╮ ", "● │ ", "│ ● ", "●─╯ "],
    );
}

#[test]
fn window_and_empty_history_do_not_fabricate_nodes() {
    let mut snap = snapshot();
    snap.changesets.truncate(2);
    snap.history_truncated = true;
    snap.missing_endpoints = vec![snap.changesets[0].primary_parent.clone().unwrap()];
    let repo = adapt(snap);
    let graph = graph::calc_graph(&repo, None);
    assert_eq!(graph.commits.len(), 2);
    assert!(repo.commit(&CommitHash::from("16")).is_none());
    assert!(repo
        .warnings()
        .iter()
        .any(|w| w.contains("outside the history window")));
    let mut snap = snapshot();
    snap.changesets.clear();
    let repo = adapt(snap);
    assert!(graph::calc_graph(&repo, None).commits.is_empty());
    assert!(repo.plastic_detail("16").is_err());
    assert!(repo.plastic_detail("unsafe").is_err());
}

#[test]
fn nonordinary_and_unsupported_refs_are_not_ancestry() {
    let mut snap = snapshot();
    snap.integrations[0].kind = IntegrationKind::SubtractiveCherryPick;
    snap.integrations[0].raw_type = "cherrypicksubstractive".into();
    snap.references[0].target = None;
    let repo = adapt(snap);
    assert_eq!(
        repo.commit(&CommitHash::from("17"))
            .unwrap()
            .parent_commit_hashes,
        vec![CommitHash::from("13")]
    );
    assert!(repo
        .warnings()
        .iter()
        .any(|w| w.contains("unsupported/missing targets")));
    assert!(repo
        .warnings()
        .iter()
        .any(|w| w.contains("1 nonordinary integration links omitted")));
    assert_eq!(
        repo.snapshot().unwrap().integrations[0].kind,
        IntegrationKind::SubtractiveCherryPick
    );
}

#[test]
fn ordinary_merge_edges_deduplicate_and_respect_window() {
    let mut snap = snapshot();
    snap.integrations.push(snap.integrations[0].clone());
    let mut duplicate_primary = snap.integrations[0].clone();
    duplicate_primary.source.id = ChangesetId::new("13").unwrap();
    snap.integrations.push(duplicate_primary);
    let repo = adapt(snap);
    assert_eq!(
        repo.commit(&CommitHash::from("17"))
            .unwrap()
            .parent_commit_hashes,
        vec![CommitHash::from("13"), CommitHash::from("14")]
    );
    assert!(repo
        .children_hash(&CommitHash::from("14"))
        .contains(&&CommitHash::from("17")));
    let graph = graph::calc_graph(&repo, None);
    assert_eq!(graph.commits.len(), 40);
    assert!(graph
        .edges
        .iter()
        .any(|row| row.iter().any(|e| !e.edge_type.is_vertically_related())));

    let mut snap = snapshot();
    snap.changesets.retain(|c| c.key.id.as_str() != "14");
    let repo = adapt(snap);
    assert_eq!(
        repo.commit(&CommitHash::from("17"))
            .unwrap()
            .parent_commit_hashes,
        vec![CommitHash::from("13")]
    );
    assert_eq!(graph::calc_graph(&repo, None).commits.len(), 39);
    assert!(repo
        .warnings()
        .iter()
        .any(|w| w.contains("ordinary merge links have endpoints outside")));
}

#[test]
fn unsupported_parent_and_merge_order_fails_without_graph_panic() {
    let mut snap = snapshot();
    snap.changesets.reverse();
    assert!(Repository::from_plastic(Backend::new(".", Limits::default()).unwrap(), snap).is_err());
    let mut snap = snapshot();
    let link = &mut snap.integrations[0];
    std::mem::swap(&mut link.source, &mut link.destination);
    assert!(Repository::from_plastic(Backend::new(".", Limits::default()).unwrap(), snap).is_err());
}
