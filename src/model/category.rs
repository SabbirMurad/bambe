use serde::{Deserialize, Serialize};

/*
  category — a self-referencing tree via `parent_id`.

  The admin table shows one nesting level (a parent row with its children
  collapsed under it), but nothing here caps the depth: a fixed
  parent/child pair would have to be migrated the first time someone wants
  Men > Footwear > Boots.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub uuid: String,
    pub name: String,
    // URL key, unique among siblings.
    pub slug: String,
    pub description: Option<String>,
    // None for a top-level category.
    pub parent_id: Option<String>,

    pub image_id: Option<String>,
    // Hidden categories stay in the tree and keep their products; they just
    // don't render in storefront navigation.
    pub visible: bool,
    // Manual ordering within a parent — menus are not alphabetical.
    pub position: u16,

    /*
      Denormalised count for the admin list, over products in this category
      *and* its descendants. Recompute on product writes and on re-parenting;
      it is a display figure, never the basis of a stock or billing decision.
    */
    pub product_count: u32,

    pub created_at: i64,
    pub modified_at: i64,
    pub modified_by: Option<String>,
}
