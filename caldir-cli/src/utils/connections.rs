use caldir_core::{Caldir, CaldirError, Connection, EventChange};

/// Return the caldir's connections, optionally narrowed to a single calendar slug.
pub fn connections(caldir: &Caldir, slug: Option<&str>) -> Vec<Result<Connection, CaldirError>> {
    let all = caldir.connections();
    match slug {
        Some(s) => all
            .into_iter()
            .filter(|conn| conn.as_ref().ok().and_then(|c| c.local().slug()) == Some(s))
            .collect(),
        None => all,
    }
}

/// Count `(created, updated, deleted)` over a sequence of event changes.
pub fn count_changes<'a, I>(changes: I) -> (usize, usize, usize)
where
    I: IntoIterator<Item = &'a EventChange>,
{
    changes
        .into_iter()
        .fold((0, 0, 0), |(c, u, d), change| match change {
            EventChange::Create(_) => (c + 1, u, d),
            EventChange::Update { .. } => (c, u + 1, d),
            EventChange::Delete(_) => (c, u, d + 1),
        })
}
