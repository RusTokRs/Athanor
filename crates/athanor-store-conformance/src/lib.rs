include!("query_lifecycle.rs");

mod atomic_publication;

pub use atomic_publication::verify_atomic_publication_contract;
