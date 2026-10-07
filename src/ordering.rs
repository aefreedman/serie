//! Child-before-parent ordering of the loaded Plastic DAG only.
use std::collections::BinaryHeap;

use chrono::DateTime;
use rustc_hash::FxHashMap;

use crate::{git::SortCommit, plastic::Snapshot, Result};

/// Return snapshot indices without rewriting producer evidence or extending its window.
/// Chrono is a date-prioritized Kahn traversal (Serie's `--date-order` intent).
/// Topo uses a ready stack to follow newly unblocked ancestry before other tips
/// (Serie's `--topo-order` intent), visiting primary before ordinary merge parents.
pub fn indices(snapshot: &Snapshot, sort: SortCommit) -> Result<Vec<usize>> {
    let mut positions = FxHashMap::default();
    for (i, cs) in snapshot.changesets.iter().enumerate() {
        if cs.key.repository != snapshot.loaded_changeset.repository {
            return Err("history contains changesets from multiple repositories".into());
        }
        if positions.insert(&cs.key, i).is_some() {
            return Err("duplicate qualified changeset in history".into());
        }
    }
    let mut parents = vec![Vec::new(); positions.len()];
    for (i, cs) in snapshot.changesets.iter().enumerate() {
        if let Some(parent) = cs.primary_parent.as_ref().and_then(|p| positions.get(p)) {
            parents[i].push(*parent);
        }
    }
    let mut merges: Vec<_> = snapshot
        .integrations
        .iter()
        .filter(|i| i.kind == crate::plastic::IntegrationKind::Merge)
        .collect();
    merges.sort_by_key(|i| {
        (
            i.destination.selector(),
            i.source.selector(),
            i.object_id.clone(),
        )
    });
    for link in merges {
        if let (Some(&child), Some(&parent)) = (
            positions.get(&link.destination),
            positions.get(&link.source),
        ) {
            if !parents[child].contains(&parent) {
                parents[child].push(parent);
            }
        }
    }
    let mut children = vec![0usize; parents.len()];
    for ps in &parents {
        for &p in ps {
            children[p] += 1;
        }
    }
    // Numeric IDs descending break timestamp ties independent of query/input order.
    // Qualified selectors are the final identity tie-breaker; dates compare instants.
    let priorities: Vec<_> = snapshot
        .changesets
        .iter()
        .map(|cs| {
            Ok((
                DateTime::parse_from_rfc3339(&cs.date)?,
                cs.key.id.as_str().parse::<u64>()?,
                cs.key.selector(),
            ))
        })
        .collect::<Result<_>>()?;
    let mut ready = BinaryHeap::new();
    for (i, &count) in children.iter().enumerate() {
        if count == 0 {
            ready.push((priorities[i].clone(), i));
        }
    }
    let mut stack = Vec::new();
    let mut output = Vec::with_capacity(parents.len());
    while let Some(i) = match sort {
        SortCommit::Chronological => ready.pop().map(|(_, i)| i),
        SortCommit::Topological => stack.pop().or_else(|| ready.pop().map(|(_, i)| i)),
    } {
        output.push(i);
        let mut unblocked = Vec::new();
        for &parent in &parents[i] {
            children[parent] -= 1;
            if children[parent] == 0 {
                match sort {
                    SortCommit::Chronological => ready.push((priorities[parent].clone(), parent)),
                    SortCommit::Topological => unblocked.push(parent),
                }
            }
        }
        // Reverse push means the primary parent is visited first when both are ready.
        stack.extend(unblocked.into_iter().rev());
    }
    if output.len() != parents.len() {
        return Err("cycle in loaded primary/ordinary merge ancestry".into());
    }
    Ok(output)
}
