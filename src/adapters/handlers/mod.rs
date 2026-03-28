//! Custom HTTP handlers for endpoints beyond basic CRUD.

mod bibkey;
mod expand;
mod export;
mod import;
mod import_file;
mod junction;
mod keyword_tree;
mod render;
mod search;

pub use bibkey::get_by_bibkey;
pub use expand::{EXPANDABLE_FIELDS, expand_bibitem};
pub use export::{
    export_authors, export_bibitems, export_institutions, export_journals, export_keywords,
    export_publishers, export_schools, export_series,
};
pub use import::{
    import_authors, import_bibitems, import_institutions, import_journals, import_keywords,
    import_publishers, import_schools, import_series,
};
pub use junction::{
    add_author_to_bibitem, get_bibitem_authors, get_bibitem_keywords, remove_author_from_bibitem,
    replace_bibitem_authors, set_bibitem_keywords,
};
pub use keyword_tree::get_keyword_tree;
pub use render::render_bibitems;
pub use search::search_bibitems;
