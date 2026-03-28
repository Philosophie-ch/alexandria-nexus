//! Custom HTTP handlers for endpoints beyond basic CRUD.

mod bibkey;
mod expand;
mod export;
mod import;
mod import_file;
mod junction;
mod keyword_tree;
mod search;

pub use bibkey::get_by_bibkey;
pub use expand::{EXPANDABLE_FIELDS, expand_bibitem};
pub use export::export_bibitems;
pub use import::import_bibitems;
pub use import_file::import_file;
pub use junction::{
    add_author_to_bibitem, get_bibitem_authors, get_bibitem_keywords, remove_author_from_bibitem,
    replace_bibitem_authors, set_bibitem_keywords,
};
pub use keyword_tree::get_keyword_tree;
pub use search::search_bibitems;
