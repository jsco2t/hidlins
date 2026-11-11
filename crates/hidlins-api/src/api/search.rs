use std::collections::HashMap;

use hidlins_core::{SearchOptions, SearchScope, Uuid};

use super::session::AppSession;
use crate::dto::{SearchHit, SearchModeDto, SearchOptionsDto, SearchScopeDto};
use crate::error::HidlinsApiError;

fn parse_uuid(s: &str) -> Result<Uuid, HidlinsApiError> {
    s.parse::<Uuid>()
        .map_err(|_| HidlinsApiError::InvalidInput {
            field: "uuid".to_string(),
            reason: "invalid UUID".to_string(),
        })
}

#[allow(clippy::needless_pass_by_value)] // frb FFI passes owned DTOs
impl AppSession {
    pub fn search(&self, opts: SearchOptionsDto) -> Result<Vec<SearchHit>, HidlinsApiError> {
        let state = self.lock_state();
        let vault = state.require_vault()?;

        let mode = match opts.mode {
            SearchModeDto::Substring => hidlins_core::SearchMode::Substring,
            SearchModeDto::Wildcard => hidlins_core::SearchMode::Wildcard,
            SearchModeDto::Fuzzy => hidlins_core::SearchMode::Fuzzy,
        };

        let scope = match &opts.scope {
            SearchScopeDto::All => SearchScope::All,
            SearchScopeDto::GroupSubtree(uuid_str) => {
                SearchScope::GroupSubtree(parse_uuid(uuid_str)?)
            }
            SearchScopeDto::Tag(tag) => SearchScope::Tag(tag.clone()),
        };

        let core_opts = SearchOptions::new(&opts.query)
            .with_mode(mode)
            .with_scope(scope)
            .include_recycled(opts.include_recycled);

        let results = vault.search(core_opts)?;
        let now = chrono::Utc::now();

        // Build the entry→group map in a single tree walk so each hit is
        // an O(1) lookup instead of a full traversal.
        let group_map = build_entry_group_map(vault.database());

        let hits = results
            .iter()
            .filter_map(|result| {
                let view = vault.get_entry(result.uuid).ok()?;
                let group_uuid = group_map
                    .get(&result.uuid)
                    .map_or_else(String::new, Clone::clone);
                Some(crate::dto::search_hit_from_core(
                    result,
                    &view,
                    &group_uuid,
                    opts.mode,
                    &opts.query,
                    now,
                ))
            })
            .collect();

        Ok(hits)
    }
}

/// Build a map from entry UUID to the hyphenated UUID string of its parent
/// group, in one tree traversal. Avoids O(entries * hits) when search
/// results span a large vault (PRD target: 5,000 entries, <50ms).
fn build_entry_group_map(database: &hidlins_core::Database) -> HashMap<Uuid, String> {
    let mut map = HashMap::new();
    collect_entries(&database.root(), &mut map);
    map
}

fn collect_entries(group: &hidlins_core::GroupRef<'_>, map: &mut HashMap<Uuid, String>) {
    let group_uuid = group.id().uuid().hyphenated().to_string();
    for entry in group.entries() {
        map.insert(entry.id().uuid(), group_uuid.clone());
    }
    for child in group.groups() {
        collect_entries(&child, map);
    }
}
