mod app;
mod check;
mod color;
mod config;
mod event;
mod external;
mod git;
mod graph;
mod keybind;
mod ordering;
mod plastic;
mod protocol;
mod search;
mod view;
mod widget;

#[cfg(test)]
#[path = "tests/graph.rs"]
mod graph_tests;

#[cfg(test)]
#[path = "tests/plastic_integration.rs"]
mod plastic_integration_tests;

#[cfg(test)]
#[path = "tests/ordering.rs"]
mod ordering_tests;

#[cfg(test)]
#[path = "tests/mailmap.rs"]
mod mailmap_tests;

#[cfg(test)]
#[path = "tests/git.rs"]
mod test_git;

use std::rc::Rc;

use app::{App, Ret};
use clap::{Parser, ValueEnum};
use graph::GraphImageManager;
use serde::Deserialize;

/// Serie Plastic - A rich Unity Version Control changeset graph in your terminal
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Plastic workspace used as cwd for history queries and configured commands
    #[arg(long, default_value = ".")]
    workspace: std::path::PathBuf,

    /// Print deterministic qualified history, primary edges, integrations and refs; no terminal required
    #[arg(long)]
    dump: bool,

    /// Include changed paths for this numeric changeset in --dump
    #[arg(long, requires = "dump")]
    detail: Option<String>,

    /// Maximum changesets to load (1..=10000; default 500)
    #[arg(short = 'n', long, value_name = "NUMBER")]
    max_count: Option<usize>,

    /// Image protocol to render graph [default: auto]
    #[arg(short, long, value_name = "TYPE")]
    protocol: Option<ImageProtocolType>,

    /// Changeset ordering algorithm [default: chrono]
    #[arg(short, long, value_name = "TYPE")]
    order: Option<CommitOrderType>,

    /// Changeset graph cell width [default: auto]
    #[arg(short, long, value_name = "TYPE")]
    graph_width: Option<GraphWidthType>,

    /// Changeset graph edge style [default: rounded]
    #[arg(short = 's', long, value_name = "TYPE")]
    graph_style: Option<GraphStyle>,

    /// Initial selection of changeset [default: latest]
    #[arg(short, long, value_name = "TYPE")]
    initial_selection: Option<InitialSelection>,

    /// Primary branch to keep on the leftmost column
    #[arg(short = 'b', long, value_name = "BRANCH")]
    primary_branch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ImageProtocolType {
    Auto,
    Text,
    Iterm,
    Kitty,
    KittyUnicode,
}

impl From<Option<ImageProtocolType>> for protocol::ImageProtocol {
    fn from(protocol: Option<ImageProtocolType>) -> Self {
        match protocol {
            Some(ImageProtocolType::Text) => protocol::ImageProtocol::Text,
            Some(ImageProtocolType::Auto) => protocol::auto_detect(),
            Some(ImageProtocolType::Iterm) => protocol::ImageProtocol::Iterm2,
            Some(ImageProtocolType::Kitty) => protocol::ImageProtocol::Kitty,
            Some(ImageProtocolType::KittyUnicode) => protocol::ImageProtocol::KittyUnicode {
                tmux: protocol::detect_tmux(),
            },
            None => protocol::auto_detect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CommitOrderType {
    Chrono,
    Topo,
}

impl From<Option<CommitOrderType>> for git::SortCommit {
    fn from(order: Option<CommitOrderType>) -> Self {
        match order {
            Some(CommitOrderType::Chrono) => git::SortCommit::Chronological,
            Some(CommitOrderType::Topo) => git::SortCommit::Topological,
            None => git::SortCommit::Chronological,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GraphWidthType {
    Auto,
    Double,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GraphStyle {
    Rounded,
    Angular,
}

impl From<Option<GraphStyle>> for graph::GraphStyle {
    fn from(style: Option<GraphStyle>) -> Self {
        match style {
            Some(GraphStyle::Rounded) => graph::GraphStyle::Rounded,
            Some(GraphStyle::Angular) => graph::GraphStyle::Angular,
            None => graph::GraphStyle::Rounded,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum InitialSelection {
    Latest,
    Head,
}

impl From<Option<InitialSelection>> for app::InitialSelection {
    fn from(selection: Option<InitialSelection>) -> Self {
        match selection {
            Some(InitialSelection::Latest) => app::InitialSelection::Latest,
            Some(InitialSelection::Head) => app::InitialSelection::Head,
            None => app::InitialSelection::Latest,
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args = Args::parse();
    let (core_config, ui_config, graph_config, color_theme, keybind_patch) = config::load()?;
    let order = resolve_order(args.order, core_config.option.order);
    if args.dump {
        let repository = git::Repository::load_plastic(&args.workspace, order, args.max_count)?;
        return dump_repository(&repository, args.detail.as_deref());
    }
    let keybind = keybind::KeyBind::new(keybind_patch);

    let max_count = args.max_count;
    let image_protocol = args.protocol.or(core_config.option.protocol).into();
    let graph_width = args.graph_width.or(core_config.option.graph_width);
    let graph_style = args.graph_style.or(core_config.option.graph_style).into();
    let graph_image_width_mode = graph_config.row_image_width;
    let initial_selection = args
        .initial_selection
        .or(core_config.option.initial_selection)
        .into();
    let primary_branch = args.primary_branch.filter(|s| !s.is_empty());

    let graph_color_set = color::GraphColorSet::new(&graph_config.color);

    let ctx = Rc::new(app::AppContext {
        workspace: args.workspace.canonicalize()?,
        keybind,
        core_config,
        ui_config,
        color_theme,
        image_protocol,
    });

    let ec = event::EventController::init();
    let mut refresh_view_context = None;
    let mut terminal = None;

    let ret = (|| -> Result<()> {
        let ret = loop {
            let repository = git::Repository::load_plastic(&args.workspace, order, max_count)?;

            if repository.all_commits().is_empty() {
                break Err(std::io::Error::other(
                    "no changesets in the bounded history",
                ));
            }
            let graph = graph::calc_graph(&repository, primary_branch.as_deref());

            let cell_width_type = check::decide_cell_width_type(&graph, graph_width)?;

            let graph_image_manager = GraphImageManager::new(
                &graph,
                &graph_color_set,
                cell_width_type,
                graph_style,
                graph_image_width_mode,
                image_protocol,
            );

            if terminal.is_none() {
                terminal = Some(ratatui::init());
            }

            let mut app = App::new(
                &repository,
                graph_image_manager,
                &graph_color_set,
                initial_selection,
                ctx.clone(),
                &ec,
                refresh_view_context,
            );

            match app.run(terminal.as_mut().unwrap()) {
                Ok(Ret::Quit) => {
                    break Ok(());
                }
                Ok(Ret::Refresh(request)) => {
                    refresh_view_context = Some(request.context);
                    continue;
                }
                Err(e) => {
                    break Err(e);
                }
            }
        };

        ret.map_err(Into::into)
    })();
    if terminal.is_some() {
        ratatui::restore();
    }
    ret
}

fn dump_repository(repository: &git::Repository, detail_id: Option<&str>) -> Result<()> {
    use std::io::{self, Write};
    let mut out = io::BufWriter::new(io::stdout().lock());
    write_repository_dump(repository, detail_id, &mut out)?;
    out.flush()?;
    Ok(())
}

fn write_repository_dump(
    repository: &git::Repository,
    detail_id: Option<&str>,
    out: &mut impl std::io::Write,
) -> Result<()> {
    let snapshot = repository.snapshot().expect("Plastic snapshot");
    let graph = graph::calc_graph(repository, None);
    writeln!(
        out,
        "Serie Plastic read-only; primary + ordinary merge graph; typed integrations listed separately"
    )?;
    writeln!(
        out,
        "workspace {} config {:?} {:?}",
        snapshot.workspace.loaded.selector(),
        snapshot.workspace.config_type,
        snapshot.workspace.config_name
    )?;
    writeln!(out, "loaded {}", snapshot.loaded_changeset.selector())?;
    writeln!(out, "counts changesets={} integrations={} references={}; truncated history={} integrations={} references={}", snapshot.changesets.len(), snapshot.integrations.len(), snapshot.references.len(), snapshot.history_truncated, snapshot.integrations_truncated, snapshot.references_truncated)?;
    for warning in repository.warnings() {
        writeln!(out, "WARNING {warning}")?;
    }
    let mut changesets: Vec<_> = snapshot.changesets.iter().collect();
    changesets.sort_by_key(|cs| graph.commit_pos_map[&git::CommitHash::from(cs.key.id.as_str())].1);
    for cs in changesets {
        let hash = git::CommitHash::from(cs.key.id.as_str());
        let (x, y) = graph.commit_pos_map[&hash];
        writeln!(
            out,
            "{} {} lane={} row={} branch={:?} owner={:?} date={} object={} guid={} comment={:?}",
            if cs.key == snapshot.loaded_changeset {
                '*'
            } else {
                'o'
            },
            cs.key.selector(),
            x,
            y,
            cs.branch,
            cs.owner,
            cs.date,
            cs.object_id,
            cs.guid,
            cs.comment
        )?;
        if let Some(parent) = &cs.primary_parent {
            writeln!(
                out,
                "  primary {} -> {}{}",
                cs.key.selector(),
                parent.selector(),
                if snapshot.changesets.iter().any(|c| c.key == *parent) {
                    ""
                } else {
                    " [outside window]"
                }
            )?;
        }
    }
    let mut integrations: Vec<_> = snapshot.integrations.iter().collect();
    integrations.sort_by_key(|i| {
        (
            i.destination.selector(),
            i.source.selector(),
            i.raw_type.clone(),
            i.object_id.clone(),
        )
    });
    for i in integrations {
        if i.kind == plastic::IntegrationKind::Merge
            && snapshot.changesets.iter().any(|c| c.key == i.source)
            && snapshot.changesets.iter().any(|c| c.key == i.destination)
        {
            writeln!(
                out,
                "  layout-merge {} -> {}",
                i.destination.selector(),
                i.source.selector()
            )?;
        }
        writeln!(out, "integration type={:?} {} -> {} base={} object={} owner={:?} date={:?} branches={:?}->{:?}", i.raw_type, i.source.selector(), i.destination.selector(), i.base.as_ref().map(|b| b.selector()).unwrap_or_else(|| "none".into()), i.object_id, i.owner, i.date, i.source_branch, i.destination_branch)?;
    }
    let mut refs: Vec<_> = snapshot.references.iter().collect();
    refs.sort_by_key(|r| r.selector());
    for r in refs {
        writeln!(
            out,
            "reference {} annotation={} owner={:?} date={:?} comment={:?}",
            r.selector(),
            r.target
                .as_ref()
                .map(|t| t.selector())
                .unwrap_or_else(|| "unsupported/missing".into()),
            r.owner,
            r.date,
            r.comment
        )?;
    }
    let mut missing: Vec<_> = snapshot.missing_endpoints.iter().collect();
    missing.sort_by_key(|e| e.selector());
    for endpoint in missing {
        writeln!(out, "missing {}", endpoint.selector())?;
    }
    if let Some(id) = detail_id {
        let id = plastic::ChangesetId::new(id)?;
        let cs = snapshot
            .changesets
            .iter()
            .find(|c| c.key.id == id)
            .ok_or("detail changeset is outside loaded window")?;
        let detail = repository.plastic_detail(cs.key.id.as_str())?;
        for p in detail.paths {
            writeln!(
                out,
                "path {} status={:?} source={:?} destination={:?} revision={} parent_revision={}",
                detail.key.selector(),
                p.change_type,
                p.source_path,
                p.destination_path,
                p.revision_id,
                p.parent_revision_id
            )?;
        }
    }
    Ok(())
}

fn resolve_order(cli: Option<CommitOrderType>, config: Option<CommitOrderType>) -> git::SortCommit {
    cli.or(config).into()
}

#[cfg(test)]
mod ordering_cli_tests {
    use super::*;

    #[test]
    fn order_cli_config_default_precedence() {
        assert!(matches!(
            resolve_order(None, None),
            git::SortCommit::Chronological
        ));
        assert!(matches!(
            resolve_order(None, Some(CommitOrderType::Topo)),
            git::SortCommit::Topological
        ));
        let args = Args::try_parse_from(["serie", "--dump", "--order", "chrono"]).unwrap();
        assert!(matches!(
            resolve_order(args.order, Some(CommitOrderType::Topo)),
            git::SortCommit::Chronological
        ));
        let args = Args::try_parse_from(["serie", "-o", "topo"]).unwrap();
        assert!(matches!(
            resolve_order(args.order, Some(CommitOrderType::Chrono)),
            git::SortCommit::Topological
        ));
        assert!(Args::try_parse_from(["serie", "--order", "invalid"]).is_err());
    }
}
