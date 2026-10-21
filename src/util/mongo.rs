use mongodb::{
    bson::{ doc, Document },
    Collection,
    Cursor,
};

pub async fn find_with_pagination<T>(
    collection: &Collection<T>,
    filter: Document,
    ascending: Option<bool>,
    limit: Option<u32>,
    page: Option<u32>,
) -> mongodb::error::Result<Cursor<T>>
where
    T: Unpin + Send + Sync,
{
    let limit = limit.unwrap_or(10) as i64;

    let sort_order = match ascending {
        Some(true) => 1,
        _ => -1,
    };

    let mut find = collection
        .find(filter)
        .sort(doc! { "created_at": sort_order })
        .limit(limit);

    if let Some(page) = page {
        let skip = limit * (page.saturating_sub(1) as i64);
        find = find.skip(skip as u64);
    }

    find.await
}